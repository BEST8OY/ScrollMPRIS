use crate::config::{Config, PositionMode, ScrollMode as ConfigScrollMode};
use crate::player::PlayerState;
use crate::scroll::{scroll, ScrollMode, ScrollState};

/// Picks an icon that represents the service based on its name.
pub fn icon_for(service: &str) -> &'static str {
    let service = service.to_lowercase();
    match service {
        s if s.contains("spotify") => "",
        s if s.contains("vlc") => "󰕼",
        s if s.contains("edge") => "󰇩",
        s if s.contains("firefox") => "󰈹",
        s if s.contains("mpv") => "",
        s if s.contains("chrome") => "",
        s if s.contains("telegramdesktop") => "",
        s if s.contains("tauon") => "",
        _ => "",
    }
}

fn format_metadata(format: &str, title: &str, artist: &str, album: &str) -> String {
    format
        .replace("{title}", title.trim())
        .replace("{artist}", artist.trim())
        .replace("{album}", album.trim())
        .trim()
        .to_string()
}

fn get_icon(player_state: &PlayerState) -> String {
    let service_icon = player_state.get_service().map(icon_for).unwrap_or("");
    let play_icon = if player_state.playing { "" } else { "" };

    if !service_icon.is_empty() {
        format!("{} {}", service_icon, play_icon)
    } else {
        play_icon.to_string()
    }
}

fn get_scrolled_text(
    config: &Config,
    player_state: &PlayerState,
    scroll_state: &mut ScrollState,
    formatted_metadata: &str,
) -> String {
    if config.freeze_on_pause && !player_state.playing {
        scroll_state.offset = 0;
        scroll_state.hold = 0;
        formatted_metadata.chars().take(config.width).collect()
    } else {
        scroll(
            formatted_metadata,
            scroll_state,
            config.width,
            match config.scroll_mode {
                ConfigScrollMode::Wrapping => ScrollMode::Wrapping,
                ConfigScrollMode::Reset => ScrollMode::Reset,
            },
        )
    }
}

fn get_position_text(config: &Config, player_state: &PlayerState) -> String {
    if !config.position_enabled {
        return String::new();
    }

    let seconds = match config.position_mode {
        PositionMode::Increasing => player_state.estimate_position(),
        PositionMode::Remaining => player_state
            .length
            .map_or(player_state.estimate_position(), |length| {
                (length - player_state.estimate_position()).max(0.0)
            }),
    };

    let pos_text = format_position(seconds);
    if !pos_text.is_empty() {
        format!(" {}", pos_text)
    } else {
        String::new()
    }
}

/// Print status for the current player, only if output changes.
pub fn print_status(
    config: &Config,
    player_state: &mut PlayerState,
    scroll_state: &mut ScrollState,
    last_output: &mut String,
) {
    // If there's no metadata, output a stopped status.
    if player_state.title.is_empty() && player_state.artist.is_empty() && player_state.album.is_empty() {
        let json_output = serde_json::json!({
            "text": "",
            "class": "stopped",
        })
        .to_string();

        if *last_output != json_output {
            println!("{}", json_output);
            *last_output = json_output;
        }
        return;
    }

    let formatted = format_metadata(
        &config.format,
        &player_state.title,
        &player_state.artist,
        &player_state.album,
    );

    let scrolled_text = get_scrolled_text(config, player_state, scroll_state, &formatted);

    // This check is still useful if formatted metadata results in an empty scrolled_text
    // even if title/artist/album are not all empty (e.g., format string is empty).
    if scrolled_text.trim().is_empty() {
        if !last_output.is_empty() {
            println!();
            *last_output = String::new();
        }
        return;
    }

    let class = &player_state.status.to_lowercase();
    let position_text = get_position_text(config, player_state);

    let output = if class == &"stopped".to_string() {
        String::new()
    } else if config.no_icon {
        format!("{}{}", scrolled_text, position_text)
    } else {
        let icon = get_icon(player_state);
        format!("{} {}{}", icon, scrolled_text, position_text)
    };

    let json_output = serde_json::json!({
        "text": output,
        "class": class,
    })
    .to_string();

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