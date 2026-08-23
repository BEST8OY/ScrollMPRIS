use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Parser;
use serde::{Deserialize, Serialize};

pub use crate::scroll::ScrollMode;

/// Status glyphs for playback states.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct StatusIcons {
    pub playing: String,
    pub paused: String,
    pub stopped: String,
}

impl Default for StatusIcons {
    fn default() -> Self {
        Self {
            playing: "".to_string(),
            paused: "".to_string(),
            stopped: String::new(),
        }
    }
}

/// Helper function to provide default icon mappings.
pub fn default_icon_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("spotify".to_string(), "".to_string());
    map.insert("vlc".to_string(), "󰕼".to_string());
    map.insert("edge".to_string(), "󰇩".to_string());
    map.insert("firefox".to_string(), "󰈹".to_string());
    map.insert("mpv".to_string(), "".to_string());
    map.insert("chrome".to_string(), "".to_string());
    map.insert("telegramdesktop".to_string(), "".to_string());
    map.insert("tauon".to_string(), "".to_string());
    map.insert("404".to_string(), "".to_string());
    map
}

/// Helper deserializer for string-or-array fields like `blocked`.
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match Option::<StringOrVec>::deserialize(deserializer)? {
        Some(StringOrVec::String(s)) => {
            let list = s
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();
            Ok(Some(list))
        }
        Some(StringOrVec::Vec(v)) => Ok(Some(v)),
        None => Ok(None),
    }
}

/// Status glyphs section inside `config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct StatusIconsSection {
    pub playing: Option<String>,
    pub paused: Option<String>,
    pub stopped: Option<String>,
}

/// Icons section inside `config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct IconsSection {
    pub status: Option<StatusIconsSection>,
    pub players: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Representation of the `config.toml` structure.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ConfigFile {
    pub speed: Option<u32>,
    pub width: Option<usize>,
    #[serde(deserialize_with = "deserialize_string_or_vec", default)]
    pub blocked: Option<Vec<String>>,
    pub scroll_mode: Option<ScrollMode>,
    pub format: Option<String>,
    pub tooltip_format: Option<String>,
    pub freeze: Option<bool>,
    pub freeze_on_pause: Option<bool>,
    pub icons: Option<IconsSection>,
}

/// Raw command-line arguments parsed by `clap`.
#[derive(Debug, Parser, Clone, Default)]
#[command(
    name = "ScrollMPRIS",
    author,
    version,
    about = "A fast, async, scrolling MPRIS module for Waybar written in pure Rust",
    long_about = None
)]
pub struct CliArgs {
    /// Path to optional configuration file (defaults to ~/.config/ScrollMPRIS/config.toml)
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// Generate default configuration TOML to stdout and exit
    #[arg(long = "generate-config")]
    pub generate_config: bool,

    /// Scroll speed (0: slow=1000ms, 100: fast=100ms)
    #[arg(short = 's', long = "speed")]
    pub speed: Option<u32>,

    /// Maximum width for the scrolling text
    #[arg(short = 'w', long = "width")]
    pub width: Option<usize>,

    /// Block certain players (comma-separated list)
    #[arg(
        short = 'b',
        long = "blocked",
        value_delimiter = ','
    )]
    pub blocked: Option<Vec<String>>,

    /// Default scrolling behavior: "marquee", "restart", or "bounce"
    #[arg(long = "scroll", value_enum)]
    pub scroll_mode: Option<ScrollMode>,

    /// Output format template (e.g. "{player_icon} {status_icon} {title:20} - {artist} [{position}/{length}]")
    #[arg(long = "format")]
    pub format: Option<String>,

    /// Metadata format string for tooltip
    #[arg(long = "tooltip-format")]
    pub tooltip_format: Option<String>,

    /// Custom icons JSON
    #[arg(long = "icon-format")]
    pub icon_format_json: Option<String>,

    /// Freeze scrolling and reset text when paused
    #[arg(long = "freeze", action = clap::ArgAction::SetTrue)]
    pub freeze_on_pause: bool,
}

/// Fully resolved active configuration used throughout ScrollMPRIS.
#[derive(Debug, Clone)]
pub struct Config {
    /// Scroll speed (0: slow=1000ms, 100: fast=100ms)
    pub speed: u32,
    /// Maximum width for the scrolling text
    pub width: usize,
    /// Block certain players
    pub blocked: Vec<String>,
    /// Scrolling behavior: Marquee, Restart, or Bounce
    pub scroll_mode: ScrollMode,
    /// Metadata format string
    pub format: String,
    /// Metadata format string for tooltip
    pub tooltip_format: String,
    /// Status glyphs for playback states
    pub status_icons: StatusIcons,
    /// Freeze scrolling and reset text when paused
    pub freeze_on_pause: bool,
    /// Delay in milliseconds (computed from speed)
    pub delay: u64,
    /// Map of player names to icons
    pub icon_format: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            speed: 0,
            width: 40,
            blocked: Vec::new(),
            scroll_mode: ScrollMode::Marquee,
            format: "{player_icon} {status_icon} {title} - {artist}".to_string(),
            tooltip_format: "{player_icon} {status_icon} {title} - {artist} | {album}".to_string(),
            status_icons: StatusIcons::default(),
            freeze_on_pause: false,
            delay: 1000,
            icon_format: default_icon_map(),
        }
    }
}

/// Resolve candidate paths for `config.toml`.
pub fn resolve_config_path(custom_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = custom_path {
        return Some(path.to_path_buf());
    }

    let mut candidate_dirs = Vec::new();

    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg_path = PathBuf::from(xdg);
        candidate_dirs.push(xdg_path.join("ScrollMPRIS"));
        candidate_dirs.push(xdg_path.join("scrollmpris"));
    }

    if let Ok(home) = std::env::var("HOME") {
        let config_home = PathBuf::from(home).join(".config");
        candidate_dirs.push(config_home.join("ScrollMPRIS"));
        candidate_dirs.push(config_home.join("scrollmpris"));
    }

    for dir in candidate_dirs {
        let file = dir.join("config.toml");
        if file.is_file() {
            return Some(file);
        }
    }

    None
}

/// Generates a well-documented sample `config.toml`.
pub fn generate_default_config_toml() -> &'static str {
    r#"# =============================================================================
# ScrollMPRIS Configuration
# =============================================================================
# Place this file at ~/.config/ScrollMPRIS/config.toml
# Or specify a custom path using: ScrollMPRIS --config /path/to/config.toml

# -----------------------------------------------------------------------------
# General & Scrolling Settings
# -----------------------------------------------------------------------------

# Scroll speed from 0 (slowest = 1000ms delay) to 100 (fastest = 100ms delay).
# Formula: delay_ms = max(100, 1000 - speed * 9)
speed = 0

# Default maximum character width for scrolling text blocks.
width = 40

# Default scrolling behavior: "marquee" (continuous loop), "restart" (loop from start), or "bounce" (back and forth).
scroll_mode = "marquee"

# Output format string for Waybar.
# Available tokens:
#   Metadata:  {title}, {artist}, {album}, {player}, {status}
#   Icons:     {player_icon}, {status_icon}, {icon}
#   Timers:    {position} (or {elapsed}), {remaining}, {length} (or {duration})
# Supports inline modifiers like {title:20}, {title:20:bounce}, and [scroll:20]{title}[/scroll].
format = "{player_icon} {status_icon} {title} - {artist}"

# Tooltip format string displayed on hover in Waybar.
tooltip_format = "{player_icon} {status_icon} {title} - {artist} | {album}"

# Players to ignore / block (e.g. ["firefox", "chromium", "edge"]).
blocked = []

# Pause scrolling and reset text to the start when playback is paused.
freeze_on_pause = false

# -----------------------------------------------------------------------------
# Icons & Status Indicator
# -----------------------------------------------------------------------------

# Status glyphs for playback states
[icons.status]
playing = ""
paused = ""
stopped = ""

# Custom icons per player service name.
# "404" defines the fallback icon for unmatched players.
[icons.players]
spotify = ""
vlc = "󰕼"
edge = "󰇩"
firefox = "󰈹"
mpv = ""
chrome = ""
telegramdesktop = ""
tauon = ""
"404" = ""
"#
}

impl Config {
    /// Applies values loaded from a `ConfigFile` struct into this `Config`.
    pub fn apply_config_file(&mut self, file: ConfigFile) {
        if let Some(s) = file.speed {
            self.speed = s;
        }
        if let Some(w) = file.width {
            self.width = w;
        }
        if let Some(b) = file.blocked {
            self.blocked = b;
        }
        if let Some(sm) = file.scroll_mode {
            self.scroll_mode = sm;
        }
        if let Some(f) = file.format {
            self.format = f;
        }
        if let Some(tf) = file.tooltip_format {
            self.tooltip_format = tf;
        }
        if let Some(fr) = file.freeze.or(file.freeze_on_pause) {
            self.freeze_on_pause = fr;
        }

        if let Some(icons_sec) = file.icons {
            if let Some(status) = icons_sec.status {
                if let Some(p) = status.playing {
                    self.status_icons.playing = p;
                }
                if let Some(p) = status.paused {
                    self.status_icons.paused = p;
                }
                if let Some(s) = status.stopped {
                    self.status_icons.stopped = s;
                }
            }
            if let Some(players) = icons_sec.players {
                for (k, v) in players {
                    self.icon_format.insert(k.to_lowercase(), v);
                }
            }
            for (k, v) in icons_sec.extra {
                if let serde_json::Value::String(icon_str) = v {
                    self.icon_format.insert(k.to_lowercase(), icon_str);
                }
            }
        }
    }

    /// Applies command-line argument overrides.
    pub fn apply_cli_overrides(&mut self, cli: CliArgs) {
        if let Some(s) = cli.speed {
            self.speed = s;
        }
        if let Some(w) = cli.width {
            self.width = w;
        }
        if let Some(b) = cli.blocked {
            self.blocked = b;
        }
        if let Some(sm) = cli.scroll_mode {
            self.scroll_mode = sm;
        }
        if let Some(f) = cli.format {
            self.format = f;
        }
        if let Some(tf) = cli.tooltip_format {
            self.tooltip_format = tf;
        }
        if cli.freeze_on_pause {
            self.freeze_on_pause = true;
        }
        if let Some(json) = cli.icon_format_json {
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&json) {
                for (k, v) in parsed {
                    self.icon_format.insert(k.to_lowercase(), v);
                }
            } else {
                eprintln!("Warning: Failed to parse --icon-format JSON string");
            }
        }
    }

    /// Construct `Config` from parsed CLI arguments, merging config file and CLI flags.
    pub fn from_args(args: CliArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Config::default();

        let config_path = resolve_config_path(args.config.as_deref());
        if let Some(ref path) = config_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                match toml::from_str::<ConfigFile>(&content) {
                    Ok(file_cfg) => {
                        config.apply_config_file(file_cfg);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse config file at {}: {e}",
                            path.display()
                        );
                    }
                }
            } else if args.config.is_some() {
                eprintln!(
                    "Warning: Specified config file not found: {}",
                    path.display()
                );
            }
        }

        config.apply_cli_overrides(args);

        // Compute delay from speed
        config.delay = (1000u64)
            .saturating_sub((config.speed as u64).saturating_mul(9))
            .max(100);

        // Normalize blocked
        config.blocked = config
            .blocked
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(config)
    }

    /// Parse arguments from environment/CLI and load configuration.
    pub fn parse() -> Self {
        let args = CliArgs::parse();
        if args.generate_config {
            println!("{}", generate_default_config_toml());
            std::process::exit(0);
        }
        Self::from_args(args).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.speed, 0);
        assert_eq!(config.delay, 1000);
        assert_eq!(config.width, 40);
        assert_eq!(config.scroll_mode, ScrollMode::Marquee);
        assert_eq!(
            config.format,
            "{player_icon} {status_icon} {title} - {artist}"
        );
        assert_eq!(
            config.tooltip_format,
            "{player_icon} {status_icon} {title} - {artist} | {album}"
        );
        assert_eq!(config.status_icons.playing, "");
        assert_eq!(config.status_icons.paused, "");
        assert_eq!(config.status_icons.stopped, "");
        assert!(!config.freeze_on_pause);
        assert_eq!(config.icon_format.get("spotify").unwrap(), "");
    }

    #[test]
    fn test_parse_toml_config_with_custom_status_icons() {
        let toml_str = r#"
            speed = 50
            width = 25
            scroll_mode = "bounce"
            format = "{player_icon} {title:15} | {artist} [{position}] {status_icon}"
            tooltip_format = "{title} - {artist}"
            blocked = ["firefox", "chromium"]
            freeze_on_pause = true

            [icons.status]
            playing = "▶"
            paused = "⏸"
            stopped = "⏹"

            [icons.players]
            foobar = "󰎆"
            "404" = ""
        "#;

        let file_cfg: ConfigFile = toml::from_str(toml_str).unwrap();
        let mut config = Config::default();
        config.apply_config_file(file_cfg);

        // Apply derived computations
        config.delay = (1000u64)
            .saturating_sub((config.speed as u64).saturating_mul(9))
            .max(100);

        assert_eq!(config.speed, 50);
        assert_eq!(config.delay, 550);
        assert_eq!(config.width, 25);
        assert_eq!(config.scroll_mode, ScrollMode::Bounce);
        assert_eq!(
            config.format,
            "{player_icon} {title:15} | {artist} [{position}] {status_icon}"
        );
        assert_eq!(config.tooltip_format, "{title} - {artist}");
        assert_eq!(config.blocked, vec!["firefox", "chromium"]);
        assert!(config.freeze_on_pause);
        assert_eq!(config.status_icons.playing, "▶");
        assert_eq!(config.status_icons.paused, "⏸");
        assert_eq!(config.status_icons.stopped, "⏹");
        assert_eq!(config.icon_format.get("foobar").unwrap(), "󰎆");
        assert_eq!(config.icon_format.get("404").unwrap(), "");
        assert_eq!(config.icon_format.get("spotify").unwrap(), "");
    }

    #[test]
    fn test_string_or_array_deserialization() {
        let toml_str = r#"
            blocked = "firefox, chrome, mpv"
        "#;

        let file_cfg: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(
            file_cfg.blocked,
            Some(vec![
                "firefox".to_string(),
                "chrome".to_string(),
                "mpv".to_string()
            ])
        );
    }

    #[test]
    fn test_cli_overrides_config_file() {
        let toml_str = r#"
            speed = 30
            width = 20
            scroll_mode = "restart"
            format = "{title}"
        "#;

        let file_cfg: ConfigFile = toml::from_str(toml_str).unwrap();
        let mut config = Config::default();
        config.apply_config_file(file_cfg);

        // Simulate CLI args overriding speed and format
        let cli = CliArgs {
            speed: Some(80),
            format: Some("{artist} - {title}".to_string()),
            freeze_on_pause: true,
            ..Default::default()
        };

        config.apply_cli_overrides(cli);
        assert_eq!(config.speed, 80);
        assert_eq!(config.width, 20); // From config file
        assert_eq!(config.scroll_mode, ScrollMode::Restart); // From config file
        assert_eq!(config.format, "{artist} - {title}"); // From CLI
        assert!(config.freeze_on_pause); // From CLI
    }

    #[test]
    fn test_generate_default_config_valid_toml() {
        let default_toml = generate_default_config_toml();
        let parsed: Result<ConfigFile, _> = toml::from_str(default_toml);
        assert!(
            parsed.is_ok(),
            "Generated default config must be valid TOML: {:?}",
            parsed.err()
        );
    }
}
