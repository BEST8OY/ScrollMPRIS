use std::collections::HashMap;

use crate::config::{Config, StatusIcons};
use crate::player::PlayerState;
use crate::scroll::{ScrollDirection, ScrollMode, ScrollState, scroll_frame};

/// Map of independent scroll states identified by key (field name, block id, or "__full__").
pub type ScrollStateMap = HashMap<String, ScrollState>;

/// Static regex instances for format parsing.
static BLOCK_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)\[scroll(?::([^\]]+))?\](.*?)\[/scroll\]").unwrap()
});

static FIELD_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\{([a-zA-Z_]+)(?::([^\}]+))?\}").unwrap()
});

static FIELD_SPEC_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\{[a-zA-Z_]+:[^\}]+\}").unwrap()
});

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

/// Retrieve the player brand icon glyph.
pub fn get_player_icon(
    player_state: &PlayerState,
    icon_format: &HashMap<String, String>,
) -> String {
    let service = player_state.get_service().unwrap_or("").to_lowercase();
    icon_format
        .iter()
        .find(|(key, _)| service.contains(*key))
        .map(|(_, icon)| icon.as_str())
        .unwrap_or_else(|| icon_format.get("404").map(|s| s.as_str()).unwrap_or(""))
        .to_string()
}

/// Retrieve the playback state icon glyph.
pub fn get_status_icon(player_state: &PlayerState, status_icons: &StatusIcons) -> String {
    let is_stopped =
        !player_state.playing && player_state.title.is_empty() && player_state.artist.is_empty();
    if is_stopped {
        return status_icons.stopped.clone();
    }

    if player_state.playing {
        status_icons.playing.clone()
    } else {
        status_icons.paused.clone()
    }
}

/// Retrieve the string value for any format token (metadata, icon, or timer).
pub fn get_field_value(field: &str, player_state: &PlayerState, config: &Config) -> String {
    match field.to_lowercase().as_str() {
        "title" => player_state.title.clone(),
        "artist" => player_state.artist.clone(),
        "album" => player_state.album.clone(),
        "player" => player_state
            .get_service()
            .and_then(extract_player_name)
            .unwrap_or_default(),
        "status" => player_state.status.clone(),
        "player_icon" | "app_icon" => get_player_icon(player_state, &config.icon_format),
        "status_icon" | "play_icon" | "state_icon" => {
            get_status_icon(player_state, &config.status_icons)
        }
        "icon" => {
            let player_icon = get_player_icon(player_state, &config.icon_format);
            let status_icon = get_status_icon(player_state, &config.status_icons);
            if !player_icon.is_empty() && !status_icon.is_empty() {
                format!("{player_icon} {status_icon}")
            } else {
                format!("{player_icon}{status_icon}")
            }
        }
        "position" | "elapsed" => {
            let pos = player_state.estimate_position();
            format_position(pos)
        }
        "remaining" | "countdown" => {
            let pos = player_state.estimate_position();
            let remaining = player_state.length.map_or(0.0, |len| (len - pos).max(0.0));
            format_position(remaining)
        }
        "length" | "duration" => player_state.length.map(format_position).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Parses format options string like "20", "20:bounce", "marquee:15", "scroll", etc.
pub fn parse_scroll_options(
    options: Option<&str>,
    default_width: usize,
    default_mode: ScrollMode,
) -> (bool, usize, ScrollMode) {
    let mut is_scroll = false;
    let mut width = default_width;
    let mut mode = default_mode;

    if let Some(opts) = options {
        for part in opts.split(':').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if part.eq_ignore_ascii_case("scroll") {
                is_scroll = true;
            } else if let Ok(w) = part.parse::<usize>() {
                is_scroll = true;
                width = w;
            } else if let Ok(m) = part.parse::<ScrollMode>() {
                is_scroll = true;
                mode = m;
            }
        }
    }

    (is_scroll, width, mode)
}

/// Check if the format string requests field-aware scrolling.
pub fn has_field_scroll_directives(format: &str) -> bool {
    if format.contains("[scroll") {
        return true;
    }
    FIELD_SPEC_RE.is_match(format)
}

/// Formats metadata for tooltip without truncating or scrolling, resolving all fields to full text.
pub fn format_tooltip(format: &str, player_state: &PlayerState, config: &Config) -> String {
    let unwrapped = BLOCK_RE.replace_all(format, "$2");
    FIELD_RE
        .replace_all(&unwrapped, |caps: &regex::Captures| {
            let field = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            get_field_value(field, player_state, config)
        })
        .trim()
        .to_string()
}

/// Legacy format_metadata helper.
pub fn format_metadata(format: &str, title: &str, artist: &str, album: &str) -> String {
    format
        .replace("{title}", title.trim())
        .replace("{artist}", artist.trim())
        .replace("{album}", album.trim())
        .trim()
        .to_string()
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

/// Render scrolled text according to configuration, format directives, and player state.
pub fn render_scrolled_format(
    config: &Config,
    player_state: &PlayerState,
    scroll_states: &mut ScrollStateMap,
    advance: bool,
) -> String {
    let is_frozen = config.freeze_on_pause && !player_state.playing;

    if !has_field_scroll_directives(&config.format) {
        let full_text = format_tooltip(&config.format, player_state, config);
        let state = scroll_states.entry("__full__".to_string()).or_default();
        if is_frozen {
            state.offset = 0;
            state.hold = 0;
            state.direction = ScrollDirection::Forward;
            return full_text.chars().take(config.width).collect();
        } else {
            return scroll_frame(&full_text, state, config.width, config.scroll_mode, advance);
        }
    }

    // Process block tags [scroll:options]...[/scroll]
    let mut block_idx = 0;
    let with_blocks = BLOCK_RE.replace_all(&config.format, |caps: &regex::Captures| {
        let options = caps.get(1).map(|m| m.as_str());
        let inner = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let (_, width, mode) = parse_scroll_options(options, config.width, config.scroll_mode);
        let resolved_inner = format_tooltip(inner, player_state, config);
        let key = format!("block_{block_idx}");
        block_idx += 1;

        let state = scroll_states.entry(key).or_default();
        if is_frozen {
            state.offset = 0;
            state.hold = 0;
            state.direction = ScrollDirection::Forward;
            resolved_inner.chars().take(width).collect::<String>()
        } else {
            scroll_frame(&resolved_inner, state, width, mode, advance)
        }
    });

    // Process field placeholders {field:options} or {field}
    let mut field_idx = 0;
    let rendered = FIELD_RE.replace_all(&with_blocks, |caps: &regex::Captures| {
        let field = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let options = caps.get(2).map(|m| m.as_str());
        let field_lower = field.to_lowercase();
        let (is_explicit_scroll, width, mode) =
            parse_scroll_options(options, config.width, config.scroll_mode);
        let val = get_field_value(&field_lower, player_state, config);
        let key = format!("{field_lower}_{field_idx}");
        field_idx += 1;

        if is_explicit_scroll {
            let state = scroll_states.entry(key).or_default();
            if is_frozen {
                state.offset = 0;
                state.hold = 0;
                state.direction = ScrollDirection::Forward;
                val.chars().take(width).collect::<String>()
            } else {
                scroll_frame(&val, state, width, mode, advance)
            }
        } else {
            val
        }
    });

    rendered.into_owned()
}

/// Print status for the current player, only if output changes.
/// If `advance` is true, the scrolling offset advances for the next frame.
pub fn print_status(
    config: &Config,
    player_state: &mut PlayerState,
    scroll_states: &mut ScrollStateMap,
    last_output: &mut String,
    advance: bool,
) {
    let classes = get_css_classes(player_state);
    let is_empty_metadata = player_state.title.is_empty()
        && player_state.artist.is_empty()
        && player_state.album.is_empty();
    let is_stopped = classes.iter().any(|c| c == "stopped") || is_empty_metadata;

    if is_stopped {
        let output = if config.format_stopped.is_empty() {
            String::new()
        } else {
            format_tooltip(&config.format_stopped, player_state, config)
        };

        let tooltip = if output.is_empty() {
            String::new()
        } else {
            format_tooltip(&config.tooltip_format, player_state, config)
        };

        let json_output = serde_json::json!({
            "text": output,
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

    let scrolled_text = render_scrolled_format(config, player_state, scroll_states, advance);
    let tooltip = format_tooltip(&config.tooltip_format, player_state, config);

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

    let json_output: String = serde_json::json!({
        "text": scrolled_text,
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
    let total_seconds = seconds.round().max(0.0) as i64;
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
        assert_eq!(extract_player_name("mpv"), Some("mpv".to_string()));
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
        let result =
            format_metadata("{title} - {artist}", "Song Title", "Artist Name", "Album Name");
        assert_eq!(result, "Song Title - Artist Name");
    }

    #[test]
    fn test_format_tooltip_with_tokens() {
        let config = Config::default();
        let mut state = PlayerState {
            title: "Super Long Song Title".to_string(),
            artist: "Famous Artist".to_string(),
            album: "Hit Album".to_string(),
            playing: true,
            length: Some(200.0),
            ..PlayerState::default()
        };
        state.set_service("org.mpris.MediaPlayer2.spotify");

        let result = format_tooltip(
            "{player_icon} {status_icon} {title:10} - {artist} | {album} [{position}/{length}]",
            &state,
            &config,
        );
        assert_eq!(
            result,
            "  Super Long Song Title - Famous Artist | Hit Album [00:00/03:20]"
        );
    }

    #[test]
    fn test_icon_tokens_separate_and_combined() {
        let config = Config::default();
        let mut state = PlayerState {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            playing: true,
            ..PlayerState::default()
        };
        state.set_service("org.mpris.MediaPlayer2.spotify");

        assert_eq!(
            get_field_value("player_icon", &state, &config),
            ""
        );
        assert_eq!(
            get_field_value("status_icon", &state, &config),
            ""
        );
        assert_eq!(
            get_field_value("icon", &state, &config),
            " "
        );
    }

    #[test]
    fn test_custom_status_icons() {
        let config = Config {
            status_icons: StatusIcons {
                playing: "▶".to_string(),
                paused: "⏸".to_string(),
                stopped: "⏹".to_string(),
            },
            ..Default::default()
        };

        let mut state = PlayerState {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            playing: true,
            ..PlayerState::default()
        };
        state.set_service("org.mpris.MediaPlayer2.spotify");

        assert_eq!(get_field_value("status_icon", &state, &config), "▶");
        state.playing = false;
        assert_eq!(get_field_value("status_icon", &state, &config), "⏸");
    }

    #[test]
    fn test_timer_tokens() {
        let config = Config::default();
        let mut state = PlayerState {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            playing: true,
            length: Some(300.0), // 5:00
            ..PlayerState::default()
        };
        state.update_playback_dbus("Playing".to_string(), 75.0, 1.0); // 1:15 elapsed, 3:45 remaining

        assert_eq!(get_field_value("position", &state, &config), "01:15");
        assert_eq!(get_field_value("elapsed", &state, &config), "01:15");
        assert_eq!(get_field_value("remaining", &state, &config), "03:45");
        assert_eq!(get_field_value("countdown", &state, &config), "03:45");
        assert_eq!(get_field_value("length", &state, &config), "05:00");
        assert_eq!(get_field_value("duration", &state, &config), "05:00");
    }

    #[test]
    fn test_field_aware_scrolling_only_title() {
        let config = Config {
            format: "{player_icon} {title:10} - {artist}".to_string(),
            scroll_mode: ScrollMode::Marquee,
            ..Default::default()
        };

        let mut player_state = PlayerState {
            title: "Super Long Song Title".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            playing: true,
            ..PlayerState::default()
        };
        player_state.set_service("org.mpris.MediaPlayer2.spotify");

        let mut scroll_states = ScrollStateMap::new();

        let frame1 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame1, " Super Long - Artist");

        let frame2 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame2, " uper Long  - Artist");

        // Player icon, artist and " - " did not scroll!
    }

    #[test]
    fn test_field_aware_multiple_fields_different_modes() {
        let config = Config {
            format: "{title:6:marquee} | {artist:4:bounce}".to_string(),
            ..Default::default()
        };

        let player_state = PlayerState {
            title: "ABCDEFG".to_string(),
            artist: "12345".to_string(),
            playing: true,
            ..PlayerState::default()
        };

        let mut scroll_states = ScrollStateMap::new();

        let frame1 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame1, "ABCDEF | 1234");

        let frame2 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame2, "BCDEFG | 2345");
    }

    #[test]
    fn test_field_aware_block_scroll() {
        let config = Config {
            format: "[scroll:10]{title} - {artist}[/scroll] | {album}".to_string(),
            scroll_mode: ScrollMode::Marquee,
            ..Default::default()
        };

        let player_state = PlayerState {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "MyAlbum".to_string(),
            playing: true,
            ..PlayerState::default()
        };

        let mut scroll_states = ScrollStateMap::new();

        let frame1 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame1, "Song - Art | MyAlbum");

        let frame2 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame2, "ong - Arti | MyAlbum");
    }

    #[test]
    fn test_stopped_state_output() {
        let mut config = Config::default();
        let mut state = PlayerState::default();
        let mut scroll_states = ScrollStateMap::new();
        let mut last_output = String::new();

        // 1. Default format_stopped is empty -> outputs empty text (Waybar auto-hides)
        print_status(&config, &mut state, &mut scroll_states, &mut last_output, false);
        let parsed: serde_json::Value = serde_json::from_str(&last_output).unwrap();
        assert_eq!(parsed["text"], "");
        assert_eq!(parsed["class"], serde_json::json!(["stopped"]));

        // 2. Custom format_stopped -> outputs custom placeholder
        config.format_stopped = "{status_icon} No Media".to_string();
        config.status_icons.stopped = "⏹".to_string();
        last_output.clear();
        print_status(&config, &mut state, &mut scroll_states, &mut last_output, false);
        let parsed: serde_json::Value = serde_json::from_str(&last_output).unwrap();
        assert_eq!(parsed["text"], "⏹ No Media");
        assert_eq!(parsed["class"], serde_json::json!(["stopped"]));
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

        let mut scroll_states = ScrollStateMap::new();
        let mut last_output = String::new();

        print_status(
            &config,
            &mut state,
            &mut scroll_states,
            &mut last_output,
            false,
        );

        // Verify that the produced output is strictly valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&last_output).expect("Must be valid JSON");
        assert_eq!(parsed["class"], serde_json::json!(["playing", "spotify"]));
        assert!(parsed["text"]
            .as_str()
            .unwrap()
            .contains("Song \"With Quotes\""));
    }

    #[test]
    fn test_constant_rendered_length_during_marquee_scroll() {
        let config = Config {
            format: "{player_icon} {title:15:marquee} | {artist}".to_string(),
            scroll_mode: ScrollMode::Marquee,
            ..Default::default()
        };

        let mut player_state = PlayerState {
            title: "Super Long Song Title That Exceeds Width".to_string(),
            artist: "Fixed Artist".to_string(),
            playing: true,
            ..PlayerState::default()
        };
        player_state.set_service("org.mpris.MediaPlayer2.spotify");

        let mut scroll_states = ScrollStateMap::new();
        let expected_len = " ".chars().count() + 15 + " | Fixed Artist".chars().count();

        // Run through multiple full scroll cycles and ensure every single frame has exact constant character count
        for _ in 0..50 {
            let frame = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
            assert_eq!(
                frame.chars().count(),
                expected_len,
                "Frame length fluctuates: {:?}",
                frame
            );
        }
    }
}
