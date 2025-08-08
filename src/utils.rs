/// Picks an icon that represents the service based on its name.
pub fn icon_for(service: &str) -> &'static str {
    let service = service.to_lowercase();
    if service.contains("spotify") {
        ""
    } else if service.contains("vlc") {
        "󰕼"
    } else if service.contains("edge") {
        "󰇩"
    } else if service.contains("firefox") {
        "󰈹"
    } else if service.contains("mpv") {
        ""
    } else if service.contains("chrome") {
        ""
    } else if service.contains("telegramdesktop") {
        ""
    } else if service.contains("tauon") {
        ""
    } else {
        ""
    }
}
use crate::config::{Config, ScrollMode as ConfigScrollMode, PositionMode};
use crate::scroll::{scroll, ScrollMode, ScrollState};
use crate::player::PlayerState;

/// Print status for the current player, only if output changes.
pub fn print_status(
    config: &Config,
    player_state: &mut PlayerState,
    scroll_state: &mut ScrollState,
    last_output: &mut String,
) {
    // Combine service icon and play/pause icon
    let service_icon = player_state.get_service().map(icon_for).unwrap_or("");
    let play_icon = if player_state.playing { "" } else { "" };
    let icon = if !service_icon.is_empty() {
        format!("{} {}", service_icon, play_icon)
    } else {
        play_icon.to_string()
    };
    let class = if player_state.playing { "playing" } else { "paused" };
    let static_text = format!("{} - {}", player_state.title, player_state.artist);
    let scrolled_text = if config.freeze_on_pause && !player_state.playing {
        scroll_state.offset = 0;
        scroll_state.hold = 0;
        static_text.chars().take(config.width).collect::<String>()
    } else {
        scroll(
            &static_text,
            scroll_state,
            config.width,
            match config.scroll_mode {
                ConfigScrollMode::Wrapping => ScrollMode::Wrapping,
                ConfigScrollMode::Reset => ScrollMode::Reset,
            },
        )
    };
    let position_text = if config.position_enabled {
        let seconds = match config.position_mode {
            PositionMode::Increasing => player_state.estimate_position(),
            PositionMode::Remaining => {
                if let Some(length) = player_state.length {
                    let rem = length - player_state.estimate_position();
                    if rem > 0.0 { rem } else { 0.0 }
                } else {
                    player_state.estimate_position()
                }
            }
        };
        let pos_text = format_position(seconds);
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

/// Formats time (in seconds) to a mm:ss or hh:mm:ss string.
pub fn format_position(seconds: f64) -> String {
    let total_seconds = seconds as i64;
    if total_seconds >= 3600 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }
}
