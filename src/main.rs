use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod config;
mod mpris;
mod scroll;

use anyhow::Result;
use config::Config;
use mpris::{active_players, MprisPlayer};
use scroll::{reset, wrapping, ResetState, WrappingState};

use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged};
use dbus::message::MatchRule;

/// Print status for the current player, only if output changes.
fn print_status(
    config: &Config,
    player: &MprisPlayer,
    reset_state: &mut ResetState,
    wrapping_state: &mut WrappingState,
    last_output: &Arc<Mutex<String>>,
) {
    let (icon, normalized_status) = player.icon_and_status();
    let class = if normalized_status == "stopped" {
        "stopped"
    } else {
        normalized_status.as_str()
    };

    let static_text = player.formatted_metadata(&config.format);
    let scrolled_text = match config.scroll_mode {
        config::ScrollMode::Wrapping => wrapping(&static_text, wrapping_state, config.width),
        config::ScrollMode::Reset => reset(&static_text, reset_state, config.width),
    };

    let position_text = if config.position_enabled {
        let pos_text = player.get_position(config.position_mode);
        if !pos_text.is_empty() {
            format!(" {}", pos_text)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let output = if config.no_icon {
        format!("{}{}", scrolled_text, position_text)
    } else if !icon.is_empty() && !scrolled_text.is_empty() {
        format!("{} {}{}", icon, scrolled_text, position_text)
    } else {
        format!("{}{}", icon, scrolled_text)
    };
    let json_output = serde_json::json!({"text": output, "class": class}).to_string();
    let mut last = last_output.lock().unwrap();
    if *last != json_output {
        println!("{}", json_output);
        *last = json_output;
    }
}

/// Select the first non-blocked player.
fn select_player(config: &Config) -> Option<MprisPlayer> {
    active_players().into_iter().find(|p| {
        !config
            .blocked
            .iter()
            .any(|b| p.service.to_lowercase().contains(b))
    })
}

fn main() -> Result<()> {
    let config = Config::parse();
    let reset_state = Arc::new(Mutex::new(ResetState::new()));
    let wrapping_state = Arc::new(Mutex::new(WrappingState::new()));
    let last_output = Arc::new(Mutex::new(String::new()));

    let conn = Connection::new_session()?;

    // Shared state for the currently active player
    let current_player = Arc::new(Mutex::new(select_player(&config)));
    let config_arc = Arc::new(config);
    let last_output_arc = Arc::clone(&last_output);

    // Print initial status
    {
        let player_guard = current_player.lock().unwrap();
        if let Some(ref player) = *player_guard {
            print_status(
                &config_arc,
                player,
                &mut reset_state.lock().unwrap(),
                &mut wrapping_state.lock().unwrap(),
                &last_output_arc,
            );
        } else {
            let mut last = last_output_arc.lock().unwrap();
            let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
            if *last != json_output {
                println!("{}", json_output);
                *last = json_output;
            }
        }
    }

    // Listen for PropertiesChanged signals for all MPRIS players
    let match_rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged")
        .with_path("/org/mpris/MediaPlayer2");
    let current_player_signal = Arc::clone(&current_player);
    let config_signal = Arc::clone(&config_arc);
    let reset_state_signal = Arc::clone(&reset_state);
    let wrapping_state_signal = Arc::clone(&wrapping_state);
    let last_output_signal = Arc::clone(&last_output);

    conn.add_match(match_rule, move |_signal: PropertiesPropertiesChanged, _conn, _msg_info| {
        // On any property change, refresh player list and select the active one
        let mut player_guard = current_player_signal.lock().unwrap();
        let new_player = select_player(&config_signal);
        let changed = match (&*player_guard, &new_player) {
            (Some(old), Some(new)) => {
                old.service != new.service ||
                old.playback_status != new.playback_status ||
                old.title != new.title ||
                old.artist != new.artist ||
                old.album != new.album ||
                old.position != new.position ||
                old.length != new.length
            },
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };
        if changed {
            *player_guard = new_player;
        }
        if let Some(ref player) = *player_guard {
            print_status(
                &config_signal,
                player,
                &mut reset_state_signal.lock().unwrap(),
                &mut wrapping_state_signal.lock().unwrap(),
                &last_output_signal,
            );
        } else {
            let mut last = last_output_signal.lock().unwrap();
            let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
            if *last != json_output {
                println!("{}", json_output);
                *last = json_output;
            }
        }
        true
    })?;

    // Timer thread for position updates while playing
    let current_player_timer = Arc::clone(&current_player);
    let config_timer = Arc::clone(&config_arc);
    let reset_state_timer = Arc::clone(&reset_state);
    let wrapping_state_timer = Arc::clone(&wrapping_state);
    let last_output_timer = Arc::clone(&last_output);

    thread::spawn(move || {
        let mut last_position: Option<i64> = None;
        let mut last_status: Option<String> = None;
        loop {
            thread::sleep(Duration::from_millis(config_timer.delay));
            let mut player_guard = current_player_timer.lock().unwrap();
            if let Some(ref player) = *player_guard {
                if player.playback_status.to_lowercase() == "playing" && config_timer.position_enabled {
                    // Re-fetch the player to get updated position
                    if let Some(updated_player) = mpris::get_player_by_service(&player.service) {
                        let position_changed = updated_player.position != last_position;
                        let status_changed = updated_player.playback_status != last_status.clone().unwrap_or_default();
                        if position_changed || status_changed {
                            last_position = updated_player.position;
                            last_status = Some(updated_player.playback_status.clone());
                            *player_guard = Some(updated_player.clone());
                            print_status(
                                &config_timer,
                                &updated_player,
                                &mut reset_state_timer.lock().unwrap(),
                                &mut wrapping_state_timer.lock().unwrap(),
                                &last_output_timer,
                            );
                        }
                    }
                }
            }
        }
    });

    // Main loop: process D-Bus events
    loop {
        conn.process(Duration::from_millis(1000))?;
    }
}
