use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use config::Config;
use mpris::events::MprisEventHandler;
use scroll::ScrollState;
use player::PlayerState;
use tokio::sync::mpsc;

mod config;
mod mpris;
mod scroll;
mod player;
mod utils;

use utils::print_status;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::parse());
    let scroll_state = Arc::new(Mutex::new(ScrollState::new()));
    let last_output = Arc::new(Mutex::new(String::new()));
    let player_state = Arc::new(Mutex::new(PlayerState::default()));
    let (tx, mut rx) = mpsc::channel(8);
    let block_list = config.blocked.clone();

    // Spawn MPRIS event handler
    {
        let player_state1 = player_state.clone();
        let tx1 = tx.clone();
        let player_state2 = player_state.clone();
        let tx2 = tx.clone();
        let block_list = block_list.clone();
        tokio::spawn(async move {
            let mut event_handler = MprisEventHandler::new(
                move |meta, pos, playback_status, service| {
                    let mut player_state = player_state1.lock().unwrap();
                    player_state.update_from_metadata(&meta);
                    player_state.set_service(&service);
                    let is_playing = playback_status == "Playing";
                    player_state.update_playback_dbus(is_playing, pos);
                    let _ = tx1.try_send(());
                },
                move |_meta, pos, _service| {
                    let mut player_state = player_state2.lock().unwrap();
                    player_state.reset_position_cache(pos);
                    let _ = tx2.try_send(());
                },
                block_list,
            )
            .await
            .expect("Failed to create MPRIS event handler");
            let _ = event_handler.handle_events().await;
        });
    }

    // Spawn status printer
    {
        let player_state = player_state.clone();
        let scroll_state = scroll_state.clone();
        let last_output = last_output.clone();
        let config = config.clone();
        tokio::spawn(async move {
            while let Some(_) = rx.recv().await {
                let mut player_state = player_state.lock().unwrap();
                let mut scroll_state = scroll_state.lock().unwrap();
                let mut last_output = last_output.lock().unwrap();
                print_status(&config, &mut player_state, &mut scroll_state, &mut last_output);
            }
        });
    }

    // Main loop: periodic update
    loop {
        tokio::time::sleep(Duration::from_millis(config.delay)).await;
        let mut player_state = player_state.lock().unwrap();
        if player_state.playing {
            let mut scroll_state = scroll_state.lock().unwrap();
            let mut last_output = last_output.lock().unwrap();
            print_status(&config, &mut player_state, &mut scroll_state, &mut last_output);
        }
    }
}