use std::collections::HashMap;

use crate::config::{Config, PositionMode};
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

/// Retrieve the string value for a metadata token.
pub fn get_field_value(field: &str, player_state: &PlayerState) -> String {
    match field.to_lowercase().as_str() {
        "title" => player_state.title.clone(),
        "artist" => player_state.artist.clone(),
        "album" => player_state.album.clone(),
        "player" => player_state
            .get_service()
            .and_then(extract_player_name)
            .unwrap_or_default(),
        "status" => player_state.status.clone(),
        "position" => {
            let pos = player_state.estimate_position();
            format_position(pos)
        }
        "length" => player_state
            .length
            .map(format_position)
            .unwrap_or_default(),
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

/// Check if the format string or CLI configuration requests field-aware scrolling.
pub fn has_field_scroll_directives(format: &str, scroll_targets: &[String]) -> bool {
    if !scroll_targets.is_empty() {
        return true;
    }
    if format.contains("[scroll") {
        return true;
    }
    FIELD_SPEC_RE.is_match(format)
}

/// Formats metadata for tooltip without truncating or scrolling, resolving all fields to full text.
pub fn format_tooltip(format: &str, player_state: &PlayerState) -> String {
    let unwrapped = BLOCK_RE.replace_all(format, "$2");
    FIELD_RE
        .replace_all(&unwrapped, |caps: &regex::Captures| {
            let field = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            get_field_value(field, player_state)
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

/// Render scrolled text according to configuration, format directives, and player state.
pub fn render_scrolled_format(
    config: &Config,
    player_state: &PlayerState,
    scroll_states: &mut ScrollStateMap,
    advance: bool,
) -> String {
    let is_frozen = config.freeze_on_pause && !player_state.playing;

    if !has_field_scroll_directives(&config.format, &config.scroll_targets) {
        let full_text = format_tooltip(&config.format, player_state);
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
        let resolved_inner = format_tooltip(inner, player_state);
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
        let is_target_scroll = config.scroll_targets.contains(&field_lower);
        let val = get_field_value(&field_lower, player_state);
        let key = format!("{field_lower}_{field_idx}");
        field_idx += 1;

        if is_explicit_scroll || is_target_scroll {
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
    scroll_states: &mut ScrollStateMap,
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

    let scrolled_text = render_scrolled_format(config, player_state, scroll_states, advance);
    let tooltip = format_tooltip(&config.tooltip_format, player_state);

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
        let icon = get_icon(
            player_state,
            &config.icon_format,
            config.no_status_icon,
            config.switch_icons,
        );
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
    fn test_format_tooltip() {
        let state = PlayerState {
            title: "Super Long Song Title".to_string(),
            artist: "Famous Artist".to_string(),
            album: "Hit Album".to_string(),
            ..PlayerState::default()
        };
        let result = format_tooltip("{title:10} - {artist} | {album}", &state);
        assert_eq!(result, "Super Long Song Title - Famous Artist | Hit Album");
    }

    #[test]
    fn test_field_aware_scrolling_only_title() {
        let mut config = Config::default();
        config.format = "{title:10} - {artist}".to_string();
        config.scroll_mode = ScrollMode::Marquee;

        let player_state = PlayerState {
            title: "Super Long Song Title".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            playing: true,
            ..PlayerState::default()
        };

        let mut scroll_states = ScrollStateMap::new();

        let frame1 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame1, "Super Long - Artist");

        let frame2 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame2, "uper Long  - Artist");

        // Artist and " - " did not scroll!
    }

    #[test]
    fn test_field_aware_scrolling_targets_cli() {
        let mut config = Config::default();
        config.format = "{title} - {artist}".to_string();
        config.scroll_targets = vec!["title".to_string()];
        config.width = 10;
        config.scroll_mode = ScrollMode::Marquee;

        let player_state = PlayerState {
            title: "Super Long Song Title".to_string(),
            artist: "Artist".to_string(),
            playing: true,
            ..PlayerState::default()
        };

        let mut scroll_states = ScrollStateMap::new();

        let frame1 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame1, "Super Long - Artist");

        let frame2 = render_scrolled_format(&config, &player_state, &mut scroll_states, true);
        assert_eq!(frame2, "uper Long  - Artist");
    }

    #[test]
    fn test_field_aware_multiple_fields_different_modes() {
        let mut config = Config::default();
        config.format = "{title:6:marquee} | {artist:4:bounce}".to_string();

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
        let mut config = Config::default();
        config.format = "[scroll:10]{title} - {artist}[/scroll] | {album}".to_string();
        config.scroll_mode = ScrollMode::Marquee;

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
        let mut config = Config::default();
        config.format = "{title:15:marquee} | {artist}".to_string();
        config.scroll_mode = ScrollMode::Marquee;

        let player_state = PlayerState {
            title: "Super Long Song Title That Exceeds Width".to_string(),
            artist: "Fixed Artist".to_string(),
            playing: true,
            ..PlayerState::default()
        };

        let mut scroll_states = ScrollStateMap::new();
        let expected_len = 15 + " | Fixed Artist".chars().count();

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

