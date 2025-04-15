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

/// Print status for the current player.
fn print_status(
    config: &Config,
    player: &MprisPlayer,
    reset_state: &mut ResetState,
    wrapping_state: &mut WrappingState,
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
    println!("{}", serde_json::json!({"text": output, "class": class}));
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

    let conn = Connection::new_session()?;

    // Shared state for the currently active player
    let current_player = Arc::new(Mutex::new(select_player(&config)));
    let config_arc = Arc::new(config);

    // Print initial status
    {
        let player_guard = current_player.lock().unwrap();
        if let Some(ref player) = *player_guard {
            print_status(
                &config_arc,
                player,
                &mut reset_state.lock().unwrap(),
                &mut wrapping_state.lock().unwrap(),
            );
        } else {
            println!("{}", serde_json::json!({"text": "", "class": "none"}));
        }
    }

    // Listen for PropertiesChanged signals for all MPRIS players
    let match_rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged")
        .with_path("/org/mpris/MediaPlayer2");
    let current_player_signal = Arc::clone(&current_player);
    let config_signal = Arc::clone(&config_arc);
    let reset_state_signal = Arc::clone(&reset_state);
    let wrapping_state_signal = Arc::clone(&wrapping_state);

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
            );
        } else {
            println!("{}", serde_json::json!({"text": "", "class": "none"}));
        }
        true
    })?;

    // Timer thread for position updates while playing
    let current_player_timer = Arc::clone(&current_player);
    let config_timer = Arc::clone(&config_arc);
    let reset_state_timer = Arc::clone(&reset_state);
    let wrapping_state_timer = Arc::clone(&wrapping_state);

    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(config_timer.delay));
            let mut player_guard = current_player_timer.lock().unwrap();
            if let Some(ref player) = *player_guard {
                if player.playback_status.to_lowercase() == "playing" && config_timer.position_enabled {
                    // Re-fetch the player to get updated position
                    if let Some(updated_player) = mpris::get_player_by_service(&player.service) {
                        *player_guard = Some(updated_player.clone());
                        print_status(
                            &config_timer,
                            &updated_player,
                            &mut reset_state_timer.lock().unwrap(),
                            &mut wrapping_state_timer.lock().unwrap(),
                        );
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
