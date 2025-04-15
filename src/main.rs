use std::thread;
use std::time::Duration;

mod config;
mod mpris;
mod scroll;

use anyhow::Result;
use config::Config;
use mpris::{active_players, MprisPlayer};
use scroll::{scroll, ScrollMode, ScrollState};

use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged};
use dbus::message::MatchRule;

/// Print status for the current player, only if output changes.
fn print_status(
    config: &Config,
    player: &MprisPlayer,
    scroll_state: &mut ScrollState,
    last_output: &mut String,
) {
    let (icon, normalized_status) = player.icon_and_status();
    let class = if normalized_status == "stopped" {
        "stopped"
    } else {
        normalized_status.as_str()
    };

    let static_text = player.formatted_metadata(&config.format);
    let scrolled_text = scroll(
        &static_text,
        scroll_state,
        config.width,
        match config.scroll_mode {
            config::ScrollMode::Wrapping => ScrollMode::Wrapping,
            config::ScrollMode::Reset => ScrollMode::Reset,
        },
    );

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
    if *last_output != json_output {
        println!("{}", json_output);
        *last_output = json_output;
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
    let mut scroll_state = ScrollState::new();
    let mut last_output = String::new();
    let conn = Connection::new_session()?;
    let current_player = select_player(&config);

    // Print initial status
    if let Some(ref player) = current_player {
        print_status(
            &config,
            player,
            &mut scroll_state,
            &mut last_output,
        );
    } else {
        let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
        if last_output != json_output {
            println!("{}", json_output);
        }
    }

    // Listen for PropertiesChanged signals for all MPRIS players
    let match_rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged")
        .with_path("/org/mpris/MediaPlayer2");
    let config_signal = config.clone();
    let mut scroll_state_signal = ScrollState::new();
    let mut last_output_signal = String::new();
    let mut current_player_signal = current_player.clone();

    conn.add_match(match_rule, move |_signal: PropertiesPropertiesChanged, _conn, _msg_info| {
        let new_player = select_player(&config_signal);
        let changed = match (&current_player_signal, &new_player) {
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
            current_player_signal = new_player.clone();
        }
        if let Some(ref player) = current_player_signal {
            print_status(
                &config_signal,
                player,
                &mut scroll_state_signal,
                &mut last_output_signal,
            );
        } else {
            let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
            if last_output_signal != json_output {
                println!("{}", json_output);
                last_output_signal = json_output;
            }
        }
        true
    })?;

    // Timer thread for position updates while playing
    let config_timer = config.clone();
    let mut scroll_state_timer = ScrollState::new();
    let mut last_output_timer = String::new();
    let mut current_player_timer = current_player.clone();
    thread::spawn(move || {
        let mut last_position: Option<i64> = None;
        let mut last_status: Option<String> = None;
        loop {
            thread::sleep(Duration::from_millis(config_timer.delay));
            if let Some(ref player) = current_player_timer {
                if player.playback_status.to_lowercase() == "playing" {
                    let updated_player = if config_timer.position_enabled {
                        mpris::get_player_by_service(&player.service)
                    } else {
                        None
                    };
                    let (position_changed, status_changed) = if let Some(ref updated) = updated_player {
                        (
                            updated.position != last_position,
                            updated.playback_status != last_status.clone().unwrap_or_default(),
                        )
                    } else {
                        (false, player.playback_status != last_status.clone().unwrap_or_default())
                    };
                    if config_timer.position_enabled {
                        if let Some(updated_player) = updated_player {
                            if position_changed || status_changed {
                                last_position = updated_player.position;
                                last_status = Some(updated_player.playback_status.clone());
                                current_player_timer = Some(updated_player.clone());
                                print_status(
                                    &config_timer,
                                    &updated_player,
                                    &mut scroll_state_timer,
                                    &mut last_output_timer,
                                );
                            }
                        }
                    } else {
                        // Always call print_status for scrolling, regardless of changes
                        print_status(
                            &config_timer,
                            player,
                            &mut scroll_state_timer,
                            &mut last_output_timer,
                        );
                        last_status = Some(player.playback_status.clone());
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
