use std::{env, thread, time::Duration};
use serde_json::json;

mod mpris {
    use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::Properties};
    use std::collections::HashMap;
    use std::time::Duration;
    use serde::{Serialize, Deserialize};

    const DEFAULT_ICON: &str = "";
    const TIMEOUT: Duration = Duration::from_millis(500);

    /// Chooses an icon based on the service name.
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
        } else {
            DEFAULT_ICON
        }
    }

    /// Returns a playback indicator based on the player status.
    fn status_indicator(status: &str) -> &'static str {
        match status {
            "playing" => "",
            "paused"  => "",
            _         => "",
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct MprisPlayer {
        pub service: String,
        pub playback_status: String,
        pub title: Option<String>,
        pub artist: Option<String>,
    }

    impl MprisPlayer {
        /// Returns the display icon, metadata, and normalized status.
        pub fn display_parts(&self) -> (String, String, String) {
            let status_lower = self.playback_status.to_lowercase();
            if status_lower == "stopped" {
                return (String::new(), String::new(), status_lower);
            }
            let icon = format!("{} {}", icon_for(&self.service), status_indicator(&status_lower));
            let metadata = match (&self.title, &self.artist) {
                (Some(t), Some(a)) => format!("{} - {}", t, a),
                (Some(t), None)    => t.clone(),
                _                  => self.playback_status.clone(),
            };
            (icon, metadata, status_lower)
        }
    }

    /// Attempts to create a D-Bus session connection.
    fn connection() -> Option<Connection> {
        Connection::new_session().ok()
    }

    /// Extracts the title and artist from the metadata hashmap.
    fn extract_metadata(map: &HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>) -> (Option<String>, Option<String>) {
        let title = map.get("xesam:title")
            .and_then(|v| v.0.as_str())
            .map(String::from);
        let artist = map.get("xesam:artist")
            .and_then(|v| {
                v.0.as_iter()
                    .and_then(|mut iter| iter.next())
                    .and_then(|val| val.as_str())
                    .map(String::from)
            });
        (title, artist)
    }

    /// Returns a list of active MPRIS players.
    pub fn active_players() -> Vec<MprisPlayer> {
        let conn = match connection() {
            Some(c) => c,
            None => return Vec::new(),
        };

        let proxy = conn.with_proxy("org.mpris.MediaPlayer2.playerctld", "/org/mpris/MediaPlayer2", TIMEOUT);
        let player_names: Vec<String> = proxy.get("com.github.altdesktop.playerctld", "PlayerNames")
            .unwrap_or_default();

        player_names.into_iter().filter_map(|service| {
            let player_proxy = conn.with_proxy(&service, "/org/mpris/MediaPlayer2", TIMEOUT);
            let playback_status: String = player_proxy.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus").ok()?;
            let metadata: Option<HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>> =
                player_proxy.get("org.mpris.MediaPlayer2.Player", "Metadata").ok();
            let (title, artist) = metadata.as_ref().map_or((None, None), extract_metadata);
            Some(MprisPlayer { service, playback_status, title, artist })
        }).collect()
    }
}

mod scroll {
    pub const WRAP_SPACER: &str = "   ";
    pub const RESET_HOLD: usize = 2;

    /// Holds the state for wrapping scroll mode.
    pub struct WrappingState {
        pub offset: usize,
        last_text: String,
    }
    
    impl WrappingState {
        pub fn new() -> Self {
            Self {
                offset: 0,
                last_text: String::new(),
            }
        }
    }
    
    /// Returns a substring of the padded text using modulo arithmetic.
    /// If the text changes, the scroll state is reinitialized.
    pub fn wrapping(text: &str, state: &mut WrappingState, width: usize) -> String {
        if text != state.last_text {
            state.last_text = text.to_string();
            state.offset = 0;
        }
        let padded = format!("{}{}", text, WRAP_SPACER);
        let chars: Vec<char> = padded.chars().collect();
        if chars.len() <= width {
            return text.to_string();
        }
        let frame: String = (0..width)
            .map(|i| chars[(state.offset + i) % chars.len()])
            .collect();
        state.offset = state.offset.wrapping_add(1);
        frame
    }
    
    /// Holds the state for reset scroll mode.
    pub struct ResetState {
        pub offset: usize,
        pub hold: usize,
        last_text: String,
    }
    
    impl ResetState {
        pub fn new() -> Self {
            Self {
                offset: 0,
                hold: 0,
                last_text: String::new(),
            }
        }
    }
    
    /// Scrolls text in reset mode with a fixed delay at the start and end.
    /// If the text changes, the scroll state is reinitialized.
    pub fn reset(text: &str, state: &mut ResetState, width: usize) -> String {
        if text != state.last_text {
            state.last_text = text.to_string();
            state.offset = 0;
            state.hold = 0;
        }
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= width {
            return text.to_string();
        }
        let max_offset = chars.len() - width;
        let frame: String = chars.iter().skip(state.offset).take(width).collect();
    
        if state.offset == 0 || state.offset == max_offset {
            if state.hold < RESET_HOLD {
                state.hold += 1;
            } else {
                state.hold = 0;
                state.offset = if state.offset == max_offset { 0 } else { state.offset + 1 };
            }
        } else {
            state.offset += 1;
        }
        frame
    }
}

#[derive(Debug, PartialEq)]
enum ScrollMode {
    Wrapping,
    Reset,
}

struct Config {
    delay: u64,
    width: usize,
    blocked: Vec<String>,
    scroll_mode: ScrollMode,
}

impl Config {
    /// Returns default configuration values.
    fn default() -> Self {
        Self {
            delay: 1000,
            width: 40,
            blocked: Vec::new(),
            scroll_mode: ScrollMode::Wrapping,
        }
    }

    /// Parses command-line arguments to create a configuration.
    fn from_args() -> Self {
        let mut config = Self::default();
        let args: Vec<String> = env::args().skip(1).collect();
        let mut iter = args.iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-s" => {
                    if let Some(s) = iter.next().and_then(|s| s.parse::<u32>().ok()) {
                        // Adjust delay based on the provided speed value.
                        config.delay = (1000u64)
                            .saturating_sub((s as u64).saturating_mul(9))
                            .max(100);
                    }
                },
                "-w" => {
                    if let Some(w) = iter.next().and_then(|w| w.parse::<usize>().ok()) {
                        config.width = w;
                    }
                },
                "-b" => {
                    if let Some(b) = iter.next() {
                        config.blocked = b.split(',')
                            .map(|s| s.trim().to_lowercase())
                            .collect();
                    }
                },
                "--scroll" => {
                    if let Some(mode) = iter.next() {
                        config.scroll_mode = match mode.to_lowercase().as_str() {
                            "reset" => ScrollMode::Reset,
                            "wrapping" => ScrollMode::Wrapping,
                            _ => config.scroll_mode,
                        };
                    }
                },
                _ => {},
            }
        }
        config
    }
}

/// Updates the status display based on the active media player.
fn update_status(
    config: &Config,
    reset_state: &mut scroll::ResetState,
    wrapping_state: &mut scroll::WrappingState,
) {
    let players = mpris::active_players();
    if players.is_empty() {
        println!("{}", json!({"text": "", "class": "none"}));
        return;
    }

    // Choose the first unblocked player
    if let Some(player) = players.iter().find(|p| {
        !config.blocked.iter()
            .any(|b| p.service.to_lowercase().contains(b))
    }) {
        let (icon, metadata, norm) = player.display_parts();
        let class = if norm == "stopped" { "stopped" } else { norm.as_str() };
        let display_text = if metadata.chars().count() > config.width {
            match config.scroll_mode {
                ScrollMode::Wrapping => {
                    let text = scroll::wrapping(&metadata, wrapping_state, config.width);
                    format!("{} {}", icon, text)
                },
                ScrollMode::Reset => {
                    let text = scroll::reset(&metadata, reset_state, config.width);
                    format!("{} {}", icon, text)
                },
            }
        } else {
            format!("{} {}", icon, metadata)
        };
        println!("{}", json!({"text": display_text, "class": class}));
    } else {
        println!("{}", json!({"text": "", "class": "none"}));
    }
}

fn main() {
    let config = Config::from_args();
    let mut reset_state = scroll::ResetState::new();
    let mut wrapping_state = scroll::WrappingState::new();

    loop {
        update_status(&config, &mut reset_state, &mut wrapping_state);
        thread::sleep(Duration::from_millis(config.delay));
    }
}
