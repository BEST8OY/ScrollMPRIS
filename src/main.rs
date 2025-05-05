use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use config::Config;
use mpris::{active_players, MprisPlayer};
use scroll::{scroll, ScrollMode, ScrollState};

use dbus::blocking::{Connection};

mod config;
mod mpris;
mod scroll;

/// Print status for the current player, only if output changes.
///
/// # Arguments
/// * `config` - The configuration settings.
/// * `player` - The current MPRIS player.
/// * `scroll_state` - The scroll state for the text.
/// * `last_output` - The last output string to avoid redundant prints.
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
    let scrolled_text = if config.freeze_on_pause && normalized_status == "paused" {
        // Reset scroll state and show static text if freeze flag is set and player is paused
        scroll_state.offset = 0;
        scroll_state.hold = 0;
        static_text.chars().take(config.width).collect::<String>()
    } else {
        // Otherwise, scroll normally (even if paused, if freeze flag is not set)
        scroll(
            &static_text,
            scroll_state,
            config.width,
            match config.scroll_mode {
                config::ScrollMode::Wrapping => ScrollMode::Wrapping,
                config::ScrollMode::Reset => ScrollMode::Reset,
            },
        )
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
    if *last_output != json_output {
        println!("{}", json_output);
        *last_output = json_output;
    }
}

/// Select the first non-blocked player from the list of active players.
///
/// # Arguments
/// * `config` - The configuration settings.
///
/// # Returns
/// An Option containing the first non-blocked MprisPlayer, or None if none are available.
fn select_player(config: &Config) -> Option<MprisPlayer> {
    active_players().into_iter().find(|p| {
        !config
            .blocked
            .iter()
            .any(|b| p.service.to_lowercase().contains(b))
    })
}

/// Handles D-Bus PropertiesChanged signals for all MPRIS players.
///
/// Registers a signal handler that updates the current player and output when properties change.
fn handle_dbus_signals(
    conn: &Connection,
    config: Arc<Config>,
    scroll_state: Arc<Mutex<ScrollState>>,
    last_output: Arc<Mutex<String>>,
    current_player: Arc<Mutex<Option<MprisPlayer>>>,
) -> anyhow::Result<()> {
    use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged;
    use dbus::message::MatchRule;
    let match_rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged")
        .with_path("/org/mpris/MediaPlayer2");
    let config_signal = config.clone();
    let scroll_state_signal = scroll_state.clone();
    let last_output_signal = last_output.clone();
    let current_player_signal = current_player.clone();
    conn.add_match(match_rule, move |_signal: PropertiesPropertiesChanged, _conn, _msg_info| {
        let new_player = select_player(&config_signal);
        let mut current_player_signal = current_player_signal.lock().unwrap();
        let changed = match (&*current_player_signal, &new_player) {
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
            *current_player_signal = new_player.clone();
        }
        drop(current_player_signal);
        let current_player_signal = current_player.lock().unwrap();
        let mut scroll_state_signal = scroll_state_signal.lock().unwrap();
        let mut last_output_signal = last_output_signal.lock().unwrap();
        if let Some(ref player) = *current_player_signal {
            print_status(
                &config_signal,
                player,
                &mut scroll_state_signal,
                &mut last_output_signal,
            );
        } else {
            let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
            if *last_output_signal != json_output {
                println!("{}", json_output);
                *last_output_signal = json_output;
            }
        }
        true
    })?;
    Ok(())
}

/// Spawns a timer thread for position updates while playing.
///
/// This thread periodically updates the output if the player is playing and position is enabled.
fn spawn_timer_thread(
    config: Arc<Config>,
    scroll_state: Arc<Mutex<ScrollState>>,
    last_output: Arc<Mutex<String>>,
    current_player: Arc<Mutex<Option<MprisPlayer>>>,
) {
    std::thread::spawn(move || {
        let mut last_position: Option<i64> = None;
        let mut last_status: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_millis(config.delay));
            let mut current_player_timer = current_player.lock().unwrap();
            if let Some(ref player) = *current_player_timer {
                let status = player.playback_status.to_lowercase();
                if status == "playing" || status == "paused" {
                    let updated_player = if config.position_enabled && status == "playing" {
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
                    if config.position_enabled && status == "playing" {
                        if let Some(updated_player) = updated_player {
                            if position_changed || status_changed {
                                last_position = updated_player.position;
                                last_status = Some(updated_player.playback_status.clone());
                                *current_player_timer = Some(updated_player.clone());
                                let mut scroll_state_timer = scroll_state.lock().unwrap();
                                let mut last_output_timer = last_output.lock().unwrap();
                                print_status(
                                    &config,
                                    &updated_player,
                                    &mut scroll_state_timer,
                                    &mut last_output_timer,
                                );
                            }
                        }
                    } else {
                        let mut scroll_state_timer = scroll_state.lock().unwrap();
                        let mut last_output_timer = last_output.lock().unwrap();
                        print_status(
                            &config,
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
}

/// Main entry point for ScrollMPRIS.
fn main() -> Result<()> {
    let config = Arc::new(Config::parse());
    let scroll_state = Arc::new(Mutex::new(ScrollState::new()));
    let last_output = Arc::new(Mutex::new(String::new()));
    let conn = Connection::new_session()?;
    let current_player = Arc::new(Mutex::new(select_player(&config)));

    // Print initial status
    {
        let current_player_guard = current_player.lock().unwrap();
        let mut scroll_state_guard = scroll_state.lock().unwrap();
        let mut last_output_guard = last_output.lock().unwrap();
        if let Some(ref player) = *current_player_guard {
            print_status(
                &config,
                player,
                &mut scroll_state_guard,
                &mut last_output_guard,
            );
        } else {
            let json_output = serde_json::json!({"text": "", "class": "none"}).to_string();
            if *last_output_guard != json_output {
                println!("{}", json_output);
            }
        }
    }

    // Listen for PropertiesChanged signals for all MPRIS players
    handle_dbus_signals(&conn, config.clone(), scroll_state.clone(), last_output.clone(), current_player.clone())?;

    // Timer thread for position updates while playing
    spawn_timer_thread(config.clone(), scroll_state.clone(), last_output.clone(), current_player.clone());

    // Note on ScrollState usage:
    //
    // There are multiple ScrollState instances in this program (main, D-Bus signal handler, timer thread).
    // This is intentional: each context manages its own scroll state to avoid race conditions and to allow
    // independent scrolling behavior for initial output, D-Bus events, and timer-based updates.
    //
    // If you want perfectly synchronized scrolling across all outputs, you could refactor to use a single
    // Arc<Mutex<ScrollState>> shared by all threads/handlers. However, this may introduce contention and
    // is not always desirable for user experience.

    // Main loop: process D-Bus events
    loop {
        conn.process(Duration::from_millis(1000))?;
    }
}
