use std::env;
use std::thread;
use std::time::Duration;
use serde_json::json;

mod mpris {
    use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::Properties};
    use std::collections::HashMap;
    use std::time::Duration;
    use serde::{Serialize, Deserialize};

    const DEFAULT_SERVICE_ICON: &str = "";
    const DBUS_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns a service-specific icon if a mapping is found.
    fn get_service_icon(service: &str) -> &'static str {
        let service = service.to_lowercase();
        if service.contains("spotify") {
            ""
        } else if service.contains("vlc") {
            "嗢"
        } else {
            DEFAULT_SERVICE_ICON
        }
    }

    /// Returns the playback indicator based on the status.
    fn playback_indicator(status: &str) -> &'static str {
        match status {
            "playing" => "",
            "paused"  => "",
            _         => "",
        }
    }

    /// Represents an MPRIS player.
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct MprisPlayer {
        pub service: String,
        pub playback_status: String,
        pub title: Option<String>,
        pub artist: Option<String>,
    }

    impl MprisPlayer {
        /// Returns a tuple containing the combined icon (service icon + playback indicator),
        /// metadata string, and the normalized playback status.
        pub fn output_parts(&self) -> (String, String, String) {
            let status = self.playback_status.to_lowercase();
            if status == "stopped" {
                return (String::new(), String::new(), status);
            }
            let icon = format!("{} {}", get_service_icon(&self.service), playback_indicator(&status));
            let metadata = match (&self.title, &self.artist) {
                (Some(t), Some(a)) => format!("{} - {}", t, a),
                (Some(t), None)    => t.clone(),
                _                  => self.playback_status.clone(),
            };
            (icon, metadata, status)
        }
    }

    /// Attempts to establish a D-Bus session connection.
    fn dbus_connection() -> Option<Connection> {
        Connection::new_session().ok()
    }

    /// Extracts title and artist metadata from a D-Bus metadata hash map.
    fn extract_metadata(
        metadata: &HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>,
    ) -> (Option<String>, Option<String>) {
        let title = metadata.get("xesam:title")
            .and_then(|v| v.0.as_str())
            .map(String::from);
        let artist = metadata.get("xesam:artist")
            .and_then(|v| v.0.as_iter()
                .and_then(|mut iter| iter.next())
                .and_then(|item| item.as_str())
                .map(String::from)
            );
        (title, artist)
    }

    /// Retrieves a list of active MPRIS players.
    pub fn get_active_players() -> Vec<MprisPlayer> {
        // Try to establish a connection. If fails, return empty vector.
        let conn = if let Some(c) = dbus_connection() {
            c
        } else {
            return Vec::new();
        };
    
        let proxy = conn.with_proxy(
            "org.mpris.MediaPlayer2.playerctld",
            "/org/mpris/MediaPlayer2",
            DBUS_TIMEOUT,
        );
    
        let player_names: Vec<String> = proxy
            .get("com.github.altdesktop.playerctld", "PlayerNames")
            .unwrap_or_default();
    
        player_names.into_iter().filter_map(|service| {
            let player_proxy = conn.with_proxy(&service, "/org/mpris/MediaPlayer2", DBUS_TIMEOUT);
            let playback_status: String = player_proxy
                .get("org.mpris.MediaPlayer2.Player", "PlaybackStatus")
                .ok()?;
            let metadata_map: Option<HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>> =
                player_proxy.get("org.mpris.MediaPlayer2.Player", "Metadata").ok();
            let (title, artist) = metadata_map
                .as_ref()
                .map_or((None, None), |map| extract_metadata(map));
            Some(MprisPlayer { service, playback_status, title, artist })
        }).collect()
    }
}

mod scroll {
    pub const SCROLL_SPACER: &str = "   ";

    /// Scrolls text by taking a substring of the padded text based on the offset.
    pub fn scroll_text(text: &str, offset: usize, width: usize) -> String {
        let padded = format!("{}{}", text, SCROLL_SPACER);
        let chars: Vec<char> = padded.chars().collect();
        if chars.len() <= width {
            return text.to_string();
        }
        (0..width)
            .map(|i| chars[(offset + i) % chars.len()])
            .collect()
    }
}

struct Config {
    delay: u64,
    width: usize,
    blocked_players: Vec<String>,
}

impl Config {
    fn default() -> Self {
        Self {
            delay: 1000,
            width: 40,
            blocked_players: Vec::new(),
        }
    }

    /// Parses command-line arguments into a configuration.
    fn from_args() -> Self {
        let mut config = Self::default();
        let args: Vec<String> = env::args().skip(1).collect();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-s" => if let Some(speed_str) = iter.next() {
                    if let Ok(speed) = speed_str.parse::<u32>() {
                        // Map speed (0: slow=1000ms, 100: fast=100ms)
                        config.delay = (1000u64).saturating_sub((speed as u64).saturating_mul(9)).max(100);
                    }
                },
                "-w" => if let Some(width_str) = iter.next() {
                    if let Ok(w) = width_str.parse::<usize>() {
                        config.width = w;
                    }
                },
                "-b" => if let Some(blocked) = iter.next() {
                    config.blocked_players = blocked.split(',')
                        .map(|s| s.trim().to_lowercase())
                        .collect();
                },
                _ => continue,
            }
        }
        config
    }
}

/// Selects an active (non-blocked) player, prepares display text using scrolling as needed,
/// and outputs JSON on each update.
fn update_status(config: &Config, scroll_offset: &mut usize) {
    let players = mpris::get_active_players();
    if players.is_empty() {
        println!("{}", json!({"text": "", "class": "none"}));
        return;
    }

    let player = players.iter().find(|p| {
        !config.blocked_players.iter().any(|b| p.service.to_lowercase().contains(b))
    }).unwrap_or(&players[0]);

    let (icon, metadata, normalized_status) = player.output_parts();
    let status_class = if normalized_status == "stopped" { "stopped" } else { normalized_status.as_str() };

    let display_text = if metadata.chars().count() > config.width {
        let scrolled = scroll::scroll_text(&metadata, *scroll_offset, config.width);
        *scroll_offset = scroll_offset.wrapping_add(1);
        format!("{} {}", icon, scrolled)
    } else {
        format!("{} {}", icon, metadata)
    };
    println!("{}", json!({"text": display_text, "class": status_class}));
}

fn main() {
    let config = Config::from_args();
    let mut scroll_offset = 0;
    loop {
        update_status(&config, &mut scroll_offset);
        thread::sleep(Duration::from_millis(config.delay));
    }
}
