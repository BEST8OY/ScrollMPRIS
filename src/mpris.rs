use std::collections::HashMap;
use std::time::Duration;

use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::Properties};
use serde::{Deserialize, Serialize};
use regex::Regex;

/// Default icon for unknown services.
const DEFAULT_ICON: &str = "";
const TIMEOUT: Duration = Duration::from_millis(500);

/// Picks an icon that represents the service based on its name.
fn icon_for(service: &str) -> &'static str {
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
        DEFAULT_ICON
    }
}

/// Returns a suitable playback status indicator.
fn status_indicator(status: &str) -> &'static str {
    match status {
        "playing" => "",
        "paused" => "",
        _ => "",
    }
}

/// Formats time (in microseconds) to a mm:ss or hh:mm:ss string.
pub fn format_position(microseconds: i64) -> String {
    let total_seconds = microseconds / 1_000_000;
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

/// Mode for displaying track position.
#[derive(Debug, PartialEq, Clone, Copy, clap::ValueEnum, Serialize, Deserialize)]
pub enum PositionMode {
    /// Show elapsed time
    Increasing,
    /// Show remaining time
    Remaining,
}

/// Metadata and playback state for a player.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MprisPlayer {
    pub service: String,
    pub playback_status: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub position: Option<i64>,
    pub length: Option<i64>,
}

impl MprisPlayer {
    /// Format metadata using a custom string.
    pub fn formatted_metadata(&self, fmt: &str) -> String {
        if self.playback_status.to_lowercase() == "stopped" {
            return String::new();
        }

        let title = self.title.as_deref().unwrap_or("").trim();
        let artist = self.artist.as_deref().unwrap_or("").trim();
        let album = self.album.as_deref().unwrap_or("").trim();

        let re = Regex::new(r"\{(title|artist|album)\}").unwrap();

        // Split format string into segments: text and placeholders
        let mut segments = Vec::new();
        let mut last = 0;
        for cap in re.captures_iter(fmt) {
            let m = cap.get(0).unwrap();
            let key = cap.get(1).unwrap().as_str();
            let value = match key {
                "title" => title,
                "artist" => artist,
                "album" => album,
                _ => "",
            };
            // Text before this placeholder
            segments.push((fmt[last..m.start()].to_string(), None));
            // The placeholder value
            segments.push(("".to_string(), Some(value)));
            last = m.end();
        }
        // Trailing text after last placeholder
        if last < fmt.len() {
            segments.push((fmt[last..].to_string(), None));
        }

        // Build output, skipping separators if adjacent field is missing
        let mut output = String::new();
        let mut prev_field_present = false;
        let mut i = 0;
        while i < segments.len() {
            let (ref text, ref field_opt) = segments[i];
            if let Some(field) = field_opt {
                if !field.is_empty() {
                    output.push_str(field);
                    prev_field_present = true;
                } else {
                    prev_field_present = false;
                }
                i += 1;
            } else {
                // This is a separator/prefix/suffix
                // Only add if:
                // - It's the very first segment (prefix), or
                // - It's the very last segment (suffix), or
                // - Both previous and next fields are present
                let is_first = i == 0;
                let is_last = i == segments.len() - 1;
                let next_field_present = if i + 1 < segments.len() {
                    if let Some(Some(field)) = segments.get(i + 1).map(|(_, f)| *f) {
                        !field.is_empty()
                    } else {
                        false
                    }
                } else {
                    false
                };
                if is_first || is_last || (prev_field_present && next_field_present) {
                    output.push_str(text);
                }
                i += 1;
            }
        }

        output.trim().to_string()
    }

    /// Get formatted position string.
    pub fn get_position(&self, mode: PositionMode) -> String {
        match (self.position, self.length) {
            (Some(pos), Some(len)) if mode == PositionMode::Remaining => {
                let remaining = len.saturating_sub(pos);
                format_position(remaining)
            }
            (Some(pos), _) => format_position(pos),
            _ => String::new(),
        }
    }

    /// Get icon and status string.
    pub fn icon_and_status(&self) -> (String, String) {
        let status_lower = self.playback_status.to_lowercase();
        if status_lower == "stopped" {
            return (String::new(), status_lower);
        }
        let icon = format!("{} {}", icon_for(&self.service), status_indicator(&status_lower));
        (icon, status_lower)
    }
}

/// Attempts to create a D-Bus session connection.
fn connection() -> Option<Connection> {
    Connection::new_session().ok()
}

/// Extracts metadata fields from the given hashmap.
fn extract_metadata(
    map: &HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>,
) -> (Option<String>, Option<String>, Option<String>) {
    let title = map
        .get("xesam:title")
        .and_then(|v| v.0.as_str())
        .map(String::from);
    let artist = map.get("xesam:artist").and_then(|v| {
        v.0.as_iter()
            .and_then(|mut iter| iter.next())
            .and_then(|a| a.as_str())
            .map(String::from)
    });
    let album = map
        .get("xesam:album")
        .and_then(|v| v.0.as_str())
        .map(String::from);
    (title, artist, album)
}

/// Returns a list of active MPRIS players available through D-Bus.
///
/// # Returns
/// A vector of MprisPlayer instances for all active players.
pub fn active_players() -> Vec<MprisPlayer> {
    let conn = match connection() {
        Some(c) => c,
        None => return Vec::new(),
    };

    let proxy = conn.with_proxy("org.mpris.MediaPlayer2.playerctld", "/org/mpris/MediaPlayer2", TIMEOUT);
    let player_names: Vec<String> = proxy
        .get("com.github.altdesktop.playerctld", "PlayerNames")
        .unwrap_or_default();

    player_names
        .into_iter()
        .filter_map(|service| {
            let player_proxy = conn.with_proxy(&service, "/org/mpris/MediaPlayer2", TIMEOUT);
            let playback_status: String = player_proxy.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus").ok()?;
            let metadata: Option<HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>> =
                player_proxy.get("org.mpris.MediaPlayer2.Player", "Metadata").ok();
            let (title, artist, album) = metadata
                .as_ref()
                .map_or((None, None, None), extract_metadata);
            let length: Option<i64> = metadata
                .as_ref()
                .and_then(|map| map.get("mpris:length"))
                .and_then(|v| v.0.as_i64());
            let position: Option<i64> = player_proxy
                .get("org.mpris.MediaPlayer2.Player", "Position")
                .ok();
            Some(MprisPlayer {
                service,
                playback_status,
                title,
                artist,
                album,
                position,
                length,
            })
        })
        .collect()
}

/// Fetch a player by its D-Bus service name.
///
/// # Arguments
/// * `service` - The D-Bus service name.
///
/// # Returns
/// An Option containing the MprisPlayer if found.
pub fn get_player_by_service(service: &str) -> Option<MprisPlayer> {
    // Establish a D-Bus session connection
    let conn = connection()?;
    let player_proxy = conn.with_proxy(service, "/org/mpris/MediaPlayer2", TIMEOUT);
    let playback_status: String = player_proxy.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus").ok()?;
    let metadata: Option<HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>> =
        player_proxy.get("org.mpris.MediaPlayer2.Player", "Metadata").ok();
    let (title, artist, album) = metadata
        .as_ref()
        .map_or((None, None, None), extract_metadata);
    let length: Option<i64> = metadata
        .as_ref()
        .and_then(|map| map.get("mpris:length"))
        .and_then(|v| v.0.as_i64());
    let position: Option<i64> = player_proxy
        .get("org.mpris.MediaPlayer2.Player", "Position")
        .ok();
    Some(MprisPlayer {
        service: service.to_string(),
        playback_status,
        title,
        artist,
        album,
        position,
        length,
    })
}