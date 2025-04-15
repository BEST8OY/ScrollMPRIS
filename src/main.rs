use std::{thread, time::Duration};

mod config;
mod mpris;
mod scroll;

use anyhow::Result;
use config::Config;
use mpris::active_players;
use scroll::{reset, wrapping, ResetState, WrappingState};

/// Updates the display status by selecting the first non-blocked player from active players.
/// It processes static metadata, scrolls it using the selected mode, and appends the dynamic position when enabled.
fn update_status(
    config: &Config,
    reset_state: &mut ResetState,
    wrapping_state: &mut WrappingState,
) -> Result<()> {
    let players = active_players();
    if players.is_empty() {
        println!("{}", serde_json::json!({"text": "", "class": "none"}));
        return Ok(());
    }

    if let Some(player) = players.iter().find(|p| {
        !config
            .blocked
            .iter()
            .any(|b| p.service.to_lowercase().contains(b))
    }) {
        let (icon, normalized_status) = player.icon_and_status();

        let class = if normalized_status == "stopped" {
            "stopped"
        } else {
            normalized_status.as_str()
        };

        let static_text = player.formatted_metadata(&config.format);
        let scrolled_text = match config.scroll_mode {
            config::ScrollMode::Wrapping => {
                wrapping(&static_text, wrapping_state, config.width)
            }
            config::ScrollMode::Reset => reset(&static_text, reset_state, config.width),
        };

        let position_text = if config.position_enabled {
            let pos_text = player.get_position(config.position_mode);
            if !pos_text.is_empty() {
                format!(" {}", pos_text)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let output = if !icon.is_empty() && !scrolled_text.is_empty() {
            format!("{} {}{}", icon, scrolled_text, position_text)
        } else {
            format!("{}{}", icon, scrolled_text)
        };
        println!("{}", serde_json::json!({"text": output, "class": class}));
    } else {
        println!("{}", serde_json::json!({"text": "", "class": "none"}));
    }
    Ok(())
}

fn main() -> Result<()> {
    let config = Config::parse();
    let mut reset_state = ResetState::new();
    let mut wrapping_state = WrappingState::new();

    loop {
        update_status(&config, &mut reset_state, &mut wrapping_state)?;
        thread::sleep(Duration::from_millis(config.delay));
    }
}
