use std::env;

use crate::mpris::PositionMode;

#[derive(Debug)]
pub enum ScrollMode {
    Wrapping,
    Reset,
}

#[derive(Debug)]
pub struct Config {
    pub delay: u64,
    pub width: usize,
    pub blocked: Vec<String>,
    pub scroll_mode: ScrollMode,
    pub format: String,              // Metadata format (supports {title}, {artist}, {album})
    pub position_enabled: bool,      // Enable/disable appending dynamic position
    pub position_mode: PositionMode, // Increasing or Remaining
}

impl Config {
    pub fn default() -> Self {
        Self {
            delay: 1000,
            width: 40,
            blocked: Vec::new(),
            scroll_mode: ScrollMode::Wrapping,
            format: "{title} - {artist}".to_string(),
            position_enabled: false,
            position_mode: PositionMode::Increasing,
        }
    }

    /// Parses command-line arguments:
    ///   - "-s" for speed (affects delay calculation)
    ///   - "-w" for width
    ///   - "-b" for blocked services (comma-separated list)
    ///   - "--scroll" for scroll mode ("wrapping" or "reset")
    ///   - "--format" for metadata format (without position placeholder)
    ///   - "-p" for position enabled flag ("enable" or "disable")
    ///   - "--position-mode" for position style ("increasing" or "remaining")
    pub fn from_args() -> Self {
        let mut config = Self::default();
        let args: Vec<String> = env::args().skip(1).collect();
        let mut iter = args.iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-s" => {
                    if let Some(s) = iter.next().and_then(|s| s.parse::<u32>().ok()) {
                        // The higher the speed value, the lower the delay.
                        config.delay = (1000u64)
                            .saturating_sub((s as u64).saturating_mul(9))
                            .max(100);
                    }
                }
                "-w" => {
                    if let Some(w) = iter.next().and_then(|w| w.parse::<usize>().ok()) {
                        config.width = w;
                    }
                }
                "-b" => {
                    if let Some(b) = iter.next() {
                        config.blocked = b
                            .split(',')
                            .map(|s| s.trim().to_lowercase())
                            .collect();
                    }
                }
                "--scroll" => {
                    if let Some(mode) = iter.next() {
                        config.scroll_mode = match mode.to_lowercase().as_str() {
                            "reset" => ScrollMode::Reset,
                            "wrapping" => ScrollMode::Wrapping,
                            _ => config.scroll_mode,
                        };
                    }
                }
                "--format" => {
                    if let Some(fmt) = iter.next() {
                        config.format = fmt.to_string();
                    }
                }
                "-p" => {
                    if let Some(flag) = iter.next() {
                        config.position_enabled = flag.to_lowercase() == "enable";
                    }
                }
                "--position-mode" => {
                    if let Some(mode) = iter.next() {
                        config.position_mode = match mode.to_lowercase().as_str() {
                            "remaining" => PositionMode::Remaining,
                            _ => PositionMode::Increasing,
                        };
                    }
                }
                _ => {}
            }
        }
        config
    }
}