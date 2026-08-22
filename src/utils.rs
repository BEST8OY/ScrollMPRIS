use std::collections::HashMap;

use crate::config::{Config, PositionMode, ScrollMode as ConfigScrollMode};
use crate::player::PlayerState;
use crate::scroll::{ScrollMode, ScrollState, scroll_frame};

pub fn format_metadata(format: &str, title: &str, artist: &str, album: &str) -> String {
    format
        .replace("{title}", title.trim())
        .replace("{artist}", artist.trim())
        .replace("{album}", album.trim())
        .trim()
        .to_string()
}

/// Extract clean player name from an MPRIS D-Bus service name (e.g. "org.mpris.MediaPlayer2.spotify" -> "spotify").
pub fn extract_player_name(service: &str) -> Option<String> {
    let trimmed = service.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed
        .strip_prefix("org.mpris.MediaPlayer2.")
        .unwrap_or(trimmed);
    let name = stripped.split('.').next().unwrap_or(stripped).to_lowercase();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Computes CSS classes for Waybar module tagging (e.g. ["playing", "spotify"]).
pub fn get_css_classes(player_state: &PlayerState) -> Vec<String> {
    let mut classes = Vec::new();

    let status = if player_state.status.trim().is_empty() {
        if player_state.playing {
            "playing".to_string()
        } else {
            "stopped".to_string()
        }
    } else {
        player_state.status.trim().to_lowercase()
    };

    classes.push(status);

    if let Some(service) = player_state.get_service()
        && let Some(player_name) = extract_player_name(service)
        && !classes.contains(&player_name)
    {
        classes.push(player_name);
    }

    classes
}

fn get_icon(
    player_state: &PlayerState,
    icon_format: &HashMap<String, String>,
    no_play_icon: bool,
    switch_icons: bool,
) -> String {
    let service = player_state.get_service().unwrap_or("").to_lowercase();

    let service_icon = icon_format
        .iter()
        .find(|(key, _)| service.contains(*key))
        .map(|(_, icon)| icon.as_str())
        .unwrap_or_else(|| icon_format.get("404").map(|s| s.as_str()).unwrap_or(""));

    let play_icon = if no_play_icon {
        ""
    } else if switch_icons {
        match player_state.playing {
            true => "",
            false => "",
        }
    } else {
        match player_state.playing {
            true => "",
            false => "",
        }
    };

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
    advance: bool,
) -> String {
    if config.freeze_on_pause && !player_state.playing {
        scroll_state.offset = 0;
        scroll_state.hold = 0;
        formatted_metadata.chars().take(config.width).collect()
    } else {
        scroll_frame(
            formatted_metadata,
            scroll_state,
            config.width,
            match config.scroll_mode {
                ConfigScrollMode::Wrapping => ScrollMode::Wrapping,
                ConfigScrollMode::Reset => ScrollMode::Reset,
            },
            advance,
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
/// If `advance` is true, the scrolling offset advances for the next frame.
pub fn print_status(
    config: &Config,
    player_state: &mut PlayerState,
    scroll_state: &mut ScrollState,
    last_output: &mut String,
    advance: bool,
) {
    let classes = get_css_classes(player_state);

    // If there's no metadata, output a stopped status.
    if player_state.title.is_empty()
        && player_state.artist.is_empty()
        && player_state.album.is_empty()
    {
        let json_output = serde_json::json!({
            "text": "",
            "class": classes,
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

    let scrolled_text = get_scrolled_text(config, player_state, scroll_state, &formatted, advance);
    let tooltip = format_metadata(
        &config.tooltip_format,
        &player_state.title,
        &player_state.artist,
        &player_state.album,
    );

    // If formatted metadata produces empty scrolled text, output valid JSON with empty text.
    if scrolled_text.trim().is_empty() {
        let json_output = serde_json::json!({
            "text": "",
            "class": classes,
            "tooltip": tooltip,
        })
        .to_string();

        if *last_output != json_output {
            println!("{}", json_output);
            *last_output = json_output;
        }
        return;
    }

    let is_stopped = classes.iter().any(|c| c == "stopped");
    let position_text = get_position_text(config, player_state);

    let output = if is_stopped {
        String::new()
    } else if config.no_icon {
        format!("{}{}", scrolled_text, position_text)
    } else {
        let icon = get_icon(player_state, &config.icon_format, config.no_status_icon, config.switch_icons);
        format!("{} {}{}", icon, scrolled_text, position_text)
    };

    let json_output: String = serde_json::json!({
        "text": output,
        "class": classes,
        "tooltip": tooltip
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_extract_player_name() {
        assert_eq!(
            extract_player_name("org.mpris.MediaPlayer2.spotify"),
            Some("spotify".to_string())
        );
        assert_eq!(
            extract_player_name("org.mpris.MediaPlayer2.spotify.instance_1"),
            Some("spotify".to_string())
        );
        assert_eq!(
            extract_player_name("org.mpris.MediaPlayer2.firefox.instance_1_42"),
            Some("firefox".to_string())
        );
        assert_eq!(
            extract_player_name("org.mpris.MediaPlayer2.vlc"),
            Some("vlc".to_string())
        );
        assert_eq!(
            extract_player_name("mpv"),
            Some("mpv".to_string())
        );
        assert_eq!(extract_player_name(""), None);
        assert_eq!(extract_player_name("   "), None);
    }

    #[test]
    fn test_get_css_classes() {
        let mut state = PlayerState::default();
        assert_eq!(get_css_classes(&state), vec!["stopped".to_string()]);

        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        state.set_service("org.mpris.MediaPlayer2.spotify.instance_1");
        assert_eq!(
            get_css_classes(&state),
            vec!["playing".to_string(), "spotify".to_string()]
        );

        state.update_playback_dbus("Paused".to_string(), 0.0, 1.0);
        state.set_service("org.mpris.MediaPlayer2.firefox");
        assert_eq!(
            get_css_classes(&state),
            vec!["paused".to_string(), "firefox".to_string()]
        );
    }

    #[test]
    fn test_format_position_under_hour() {
        assert_eq!(format_position(0.0), "00:00");
        assert_eq!(format_position(65.0), "01:05");
        assert_eq!(format_position(599.0), "09:59");
    }

    #[test]
    fn test_format_position_over_hour() {
        assert_eq!(format_position(3600.0), "01:00:00");
        assert_eq!(format_position(3665.0), "01:01:05");
        assert_eq!(format_position(7322.0), "02:02:02");
    }

    #[test]
    fn test_format_metadata() {
        let result = format_metadata("{title} - {artist}", "Song Title", "Artist Name", "Album Name");
        assert_eq!(result, "Song Title - Artist Name");
    }

    #[test]
    fn test_format_metadata_tooltip() {
        let result = format_metadata(
            "{title} - {artist} | {album}",
            "Song Title",
            "Artist Name",
            "Album Name",
        );
        assert_eq!(result, "Song Title - Artist Name | Album Name");
    }

    #[test]
    fn test_player_state_rate_estimation() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 2.0);
        // Simulate 2 seconds of real time elapsed
        state.last_update = Some(Instant::now() - Duration::from_secs(2));

        let est = state.estimate_position();
        // 10.0 + (2.0 elapsed * 2.0 rate) = ~14.0
        assert!((13.9..=14.2).contains(&est));
    }

    #[test]
    fn test_json_validity_with_special_characters() {
        let config = Config::default();
        let mut state = PlayerState {
            title: "Song \"With Quotes\" & <Tags>".to_string(),
            artist: "Artist \\ Name".to_string(),
            album: "Album 'Name'".to_string(),
            ..PlayerState::default()
        };
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        state.set_service("org.mpris.MediaPlayer2.spotify");

        let mut scroll_state = ScrollState::new();
        let mut last_output = String::new();

        print_status(&config, &mut state, &mut scroll_state, &mut last_output, false);

        // Verify that the produced output is strictly valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&last_output).expect("Must be valid JSON");
        assert_eq!(parsed["class"], serde_json::json!(["playing", "spotify"]));
        assert!(parsed["text"].as_str().unwrap().contains("Song \"With Quotes\""));
    }
}
