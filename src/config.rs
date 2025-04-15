use clap::{Parser, ValueEnum};
use crate::mpris::PositionMode;

/// Scrolling mode for the text output.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScrollMode {
    Wrapping,
    Reset,
}

/// Configuration for ScrollMPRIS, parsed from command-line arguments.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Scroll speed (0: slow=1000ms, 100: fast=100ms)
    #[arg(short = 's', long = "speed", default_value_t = 0)]
    pub speed: u32,

    /// Maximum width for the scrolling text
    #[arg(short = 'w', long = "width", default_value_t = 40)]
    pub width: usize,

    /// Block certain players (comma-separated list)
    #[arg(short = 'b', long = "blocked", value_delimiter = ',', default_value = "")] 
    pub blocked: Vec<String>,

    /// Choose scrolling behavior: "wrapping" for continuous loop, "reset" to restart after finishing
    #[arg(long = "scroll", value_enum, default_value_t = ScrollMode::Wrapping)]
    pub scroll_mode: ScrollMode,

    /// Metadata format (supports {title}, {artist}, {album})
    #[arg(long = "format", default_value = "{title} - {artist}")]
    pub format: String,

    /// Enable position display (show track time info)
    #[arg(short = 'p', long = "position", default_value_t = false, action = clap::ArgAction::SetTrue)]
    pub position_enabled: bool,

    /// Position style: "increasing" or "remaining"
    #[arg(long = "position-mode", default_value = "increasing")]
    pub position_mode: PositionMode,

    /// Delay in milliseconds (calculated from speed)
    #[arg(skip)]
    pub delay: u64,
}

impl Config {
    pub fn parse() -> Self {
        let mut config = <Self as Parser>::parse();
        // Calculate delay from speed
        config.delay = (1000u64)
            .saturating_sub((config.speed as u64).saturating_mul(9))
            .max(100);
        // Normalize blocked list
        config.blocked = config.blocked.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
        config
    }
}