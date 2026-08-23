use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use ScrollMPRIS::config::Config;
use ScrollMPRIS::mpris::events::MprisEventHandler;
use ScrollMPRIS::player::PlayerState;
use ScrollMPRIS::utils::{ScrollStateMap, print_status};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    let mut scroll_states = ScrollStateMap::new();
    let mut last_output = String::new();
    let player_state = Arc::new(Mutex::new(PlayerState::default()));
    let (tx, mut rx) = mpsc::channel(16);
    let block_list = config.blocked.clone();

    // Write PID
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let pid = std::process::id();
    let filename = format!("/tmp/scrollbarmpris/{}.pid", timestamp);
    fs::create_dir_all("/tmp/scrollbarmpris").expect("Failed to create directory at /tmp");
    fs::write(&filename, pid.to_string()).unwrap();

    // Spawn MPRIS event handler
    {
        let player_state = player_state.clone();
        let tx = tx.clone();
        let block_list = block_list.clone();
        tokio::spawn(async move {
            let mut backoff = Duration::from_millis(500);
            loop {
                let player_state1 = player_state.clone();
                let tx1 = tx.clone();
                let player_state2 = player_state.clone();
                let tx2 = tx.clone();
                let block_list = block_list.clone();

                match MprisEventHandler::new(
                    move |meta, pos, playback_status, service, rate| {
                        let mut state = player_state1.lock().unwrap();
                        state.update_from_metadata(&meta);
                        state.set_service(&service);
                        state.update_playback_dbus(playback_status, pos, rate);
                        let _ = tx1.try_send(());
                    },
                    move |_meta, pos, _service| {
                        let mut state = player_state2.lock().unwrap();
                        state.reset_position_cache(pos);
                        let _ = tx2.try_send(());
                    },
                    block_list,
                )
                .await
                {
                    Ok(mut event_handler) => {
                        let start = std::time::Instant::now();
                        if let Err(e) = event_handler.handle_events().await {
                            eprintln!("MPRIS event handler error: {e}");
                        }
                        if start.elapsed() >= Duration::from_secs(5) {
                            backoff = Duration::from_millis(500);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize MPRIS event handler: {e}");
                    }
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        });
    }

    // Unified Actor Loop: single owner of stdout, scroll_states, and last_output
    let mut ticker = tokio::time::interval(Duration::from_millis(config.delay));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Emit initial status (e.g. stopped) so Waybar immediately receives state
    {
        let mut state = player_state.lock().unwrap();
        print_status(
            &config,
            &mut state,
            &mut scroll_states,
            &mut last_output,
            false,
        );
    }

    loop {
        tokio::select! {
            Some(_) = rx.recv() => {
                let mut state = player_state.lock().unwrap();
                print_status(
                    &config,
                    &mut state,
                    &mut scroll_states,
                    &mut last_output,
                    false,
                );
            }
            _ = ticker.tick() => {
                let mut state = player_state.lock().unwrap();
                if state.playing {
                    print_status(
                        &config,
                        &mut state,
                        &mut scroll_states,
                        &mut last_output,
                        true,
                    );
                }
            }
        }
    }
}
