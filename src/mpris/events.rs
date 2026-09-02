//! Event watching and event handler registration for MPRIS using zbus.

use futures_util::StreamExt;
use std::pin::Pin;
use std::time::Duration;
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};
use zbus::names::BusName;

use crate::mpris::connection::{MprisError, find_active_service, get_dbus_conn, get_position};
use crate::mpris::metadata::{TrackMetadata, extract_metadata};
use crate::mpris::proxies::{MediaPlayer2PlayerProxy, PlayerctldProxy};

/// Tuning constants for transient buffering position calibration.
pub const CALIBRATION_INITIAL_DELAY: Duration = Duration::from_millis(400);
pub const CALIBRATION_POLL_INTERVAL: Duration = Duration::from_millis(800);
pub const MAX_CALIBRATION_ATTEMPTS: u8 = 15;
pub const MOVEMENT_DELTA_THRESHOLD_SECS: f64 = 0.08;

/// Result of stepping the calibration state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationStepResult {
    Continue,
    Confirmed,
    TimedOut,
}

/// Transient calibration state machine for detecting when a player actually starts
/// advancing audio position after buffering.
#[derive(Debug)]
struct CalibrationTracker {
    timer: Option<Pin<Box<tokio::time::Sleep>>>,
    attempts: u8,
    confirmed: bool,
    anchor_pos: f64,
}

impl CalibrationTracker {
    fn new(is_playing: bool, initial_pos: f64) -> Self {
        let mut tracker = Self {
            timer: None,
            attempts: 0,
            confirmed: false,
            anchor_pos: initial_pos,
        };
        if is_playing {
            tracker.arm(initial_pos);
        }
        tracker
    }

    fn arm(&mut self, anchor_pos: f64) {
        self.attempts = 0;
        self.confirmed = false;
        self.anchor_pos = anchor_pos;
        self.timer = Some(Box::pin(tokio::time::sleep(CALIBRATION_INITIAL_DELAY)));
    }

    fn disarm(&mut self) {
        self.timer = None;
    }

    /// Wait for the next calibration tick if armed, or pend indefinitely if disarmed.
    async fn tick(&mut self) {
        if let Some(ref mut timer) = self.timer {
            timer.as_mut().await;
        } else {
            futures_util::future::pending().await
        }
    }

    /// Step the calibration state machine given the newly fetched authoritative position.
    fn on_step(&mut self, real_pos: f64) -> CalibrationStepResult {
        self.attempts += 1;
        let moved = (real_pos - self.anchor_pos).abs() > MOVEMENT_DELTA_THRESHOLD_SECS;

        if moved {
            if !self.confirmed {
                // Audio started advancing; schedule 1 final confirmation check to lock in steady-state sync
                self.confirmed = true;
                self.timer = Some(Box::pin(tokio::time::sleep(CALIBRATION_POLL_INTERVAL)));
                CalibrationStepResult::Continue
            } else {
                // Steady-state verified: disarm calibration for the remainder of this track
                self.disarm();
                CalibrationStepResult::Confirmed
            }
        } else if self.attempts < MAX_CALIBRATION_ATTEMPTS {
            // Still buffering (real_pos unchanged from anchor); retry
            self.timer = Some(Box::pin(tokio::time::sleep(CALIBRATION_POLL_INTERVAL)));
            CalibrationStepResult::Continue
        } else {
            // Reached maximum attempts (slow network timeout): disarm
            self.disarm();
            CalibrationStepResult::TimedOut
        }
    }
}

/// Events emitted by the MPRIS event handler to the main actor.
#[derive(Debug, Clone, PartialEq)]
pub enum MprisEvent {
    TrackChange {
        metadata: TrackMetadata,
        service: String,
        position: f64,
        playback_status: String,
        rate: f64,
    },
    StatusChange {
        playback_status: String,
        position: f64,
        rate: f64,
    },
    Seeked {
        position: f64,
    },
    Calibrated {
        position: f64,
    },
    CalibrationTimeout,
}

/// Event handler managing MPRIS signals, player discovery, and lifecycle monitoring.
pub struct MprisEventHandler {
    event_tx: tokio::sync::mpsc::Sender<MprisEvent>,
    block_list: Vec<String>,
    current_service: String,
    last_track: TrackMetadata,
    last_playback_status: String,
    conn: zbus::Connection,
}

impl MprisEventHandler {
    /// Create a new MPRIS event handler.
    pub async fn new(
        event_tx: tokio::sync::mpsc::Sender<MprisEvent>,
        block_list: Vec<String>,
    ) -> Result<Self, MprisError> {
        let conn = get_dbus_conn().await?;

        let mut handler = Self {
            event_tx,
            block_list,
            current_service: String::new(),
            last_track: TrackMetadata::default(),
            last_playback_status: String::new(),
            conn,
        };

        // Perform initial player discovery (silently handle if no active player)
        let _ = handler.discover_active_player().await;

        Ok(handler)
    }

    fn emit(&self, event: MprisEvent) {
        let _ = self.event_tx.try_send(event);
    }

    /// Discover the active MPRIS player and switch to it if found.
    pub async fn discover_active_player(&mut self) -> Result<(), MprisError> {
        match find_active_service(&self.block_list).await {
            Ok(Some(service)) => {
                if service != self.current_service {
                    self.switch_to_player(&service).await?;
                }
            }
            Ok(None) => {
                if !self.current_service.is_empty() {
                    self.deactivate_player();
                }
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Switch active tracking to the specified player service.
    async fn switch_to_player(&mut self, service: &str) -> Result<(), MprisError> {
        let proxy = MediaPlayer2PlayerProxy::builder(&self.conn)
            .destination(service.to_string())?
            .build()
            .await?;

        let map = proxy.metadata().await.unwrap_or_default();
        let meta = extract_metadata(&map);
        let position = get_position(service).await.unwrap_or(0.0);
        let rate = proxy.rate().await.unwrap_or(1.0);
        let playback_status = proxy
            .playback_status()
            .await
            .unwrap_or_else(|_| "Stopped".to_string());

        self.current_service = service.to_string();
        self.last_track = meta.clone();
        self.last_playback_status = playback_status.clone();

        self.emit(MprisEvent::TrackChange {
            metadata: meta,
            service: service.to_string(),
            position,
            playback_status,
            rate,
        });
        Ok(())
    }

    /// Reset player state and notify listeners of player deactivation.
    fn deactivate_player(&mut self) {
        self.current_service.clear();
        self.last_track = TrackMetadata::default();
        self.last_playback_status.clear();

        self.emit(MprisEvent::TrackChange {
            metadata: TrackMetadata::default(),
            service: String::new(),
            position: 0.0,
            playback_status: String::new(),
            rate: 1.0,
        });
    }

    /// Deactivate current player and immediately attempt re-discovery.
    async fn deactivate_and_rediscover(&mut self) -> Result<(), MprisError> {
        self.deactivate_player();
        let _ = self.discover_active_player().await;
        Ok(())
    }

    /// Main event loop: watches D-Bus NameOwnerChanged, playerctld changes, and player signals.
    pub async fn handle_events(&mut self) -> Result<(), MprisError> {
        let dbus_proxy = DBusProxy::new(&self.conn).await?;
        let mut name_owner_stream = dbus_proxy.receive_name_owner_changed().await?;

        loop {
            if !self.current_service.is_empty() {
                // Active player loop
                if let Err(e) = self.run_active_player_loop(&mut name_owner_stream).await {
                    eprintln!("Error in active player loop: {e}");
                    self.deactivate_and_rediscover().await?;
                }
            } else {
                // Idle loop: 100% event-driven wait for NameOwnerChanged or playerctld signals (zero IPC polling)
                let playerctld_proxy = if dbus_proxy
                    .name_has_owner(
                        BusName::from_static_str("org.mpris.MediaPlayer2.playerctld").unwrap(),
                    )
                    .await
                    .unwrap_or(false)
                {
                    PlayerctldProxy::new(&self.conn).await.ok()
                } else {
                    None
                };
                let mut playerctld_stream = if let Some(ref p) = playerctld_proxy {
                    Some(p.receive_player_names_changed().await)
                } else {
                    None
                };

                tokio::select! {
                    Some(signal) = name_owner_stream.next() => {
                        if let Ok(args) = signal.args() {
                            let name = args.name.as_str();
                            let is_mpris = name.starts_with("org.mpris.MediaPlayer2.");
                            let old_owner = args.old_owner.as_deref().unwrap_or("");
                            let new_owner = args.new_owner.as_deref().unwrap_or("");
                            if is_mpris && old_owner != new_owner && !new_owner.is_empty() {
                                let _ = self.discover_active_player().await;
                            }
                        }
                    }
                    Some(_) = async {
                        if let Some(ref mut s) = playerctld_stream {
                            s.next().await
                        } else {
                            futures_util::future::pending().await
                        }
                    } => {
                        let _ = self.discover_active_player().await;
                    }
                }
            }
        }
    }

    /// Inner event loop for the currently active player.
    async fn run_active_player_loop(
        &mut self,
        name_owner_stream: &mut NameOwnerChangedStream,
    ) -> Result<(), MprisError> {
        let service = self.current_service.clone();
        let proxy = MediaPlayer2PlayerProxy::builder(&self.conn)
            .destination(service.clone())?
            .build()
            .await?;

        let mut seeked_stream = proxy.receive_seeked().await?;
        let mut status_stream = proxy.receive_playback_status_changed().await;
        let mut metadata_stream = proxy.receive_metadata_changed().await;
        let mut rate_stream = proxy.receive_rate_changed().await;

        // Subscribe to playerctld if available (checking ownership to prevent autostart)
        let dbus_proxy = DBusProxy::new(&self.conn).await?;
        let playerctld_proxy = if dbus_proxy
            .name_has_owner(
                BusName::from_static_str("org.mpris.MediaPlayer2.playerctld").unwrap(),
            )
            .await
            .unwrap_or(false)
        {
            PlayerctldProxy::new(&self.conn).await.ok()
        } else {
            None
        };
        let mut playerctld_stream = if let Some(ref p) = playerctld_proxy {
            Some(p.receive_player_names_changed().await)
        } else {
            None
        };

        // Initialize transient calibration tracker with authoritative player position
        let initial_pos = proxy
            .position()
            .await
            .map(|us| us as f64 / 1_000_000.0)
            .unwrap_or(0.0);
        let mut tracker =
            CalibrationTracker::new(self.last_playback_status == "Playing", initial_pos);

        loop {
            tokio::select! {
                // 0. Transient post-buffering position calibration for streaming players (e.g. Spotify)
                _ = tracker.tick() => {
                    if self.last_playback_status == "Playing" && !self.current_service.is_empty() {
                        match proxy.position().await {
                            Ok(microsecs) => {
                                let real_pos = microsecs as f64 / 1_000_000.0;
                                self.emit(MprisEvent::Calibrated { position: real_pos });
                                if tracker.on_step(real_pos) == CalibrationStepResult::TimedOut {
                                    self.emit(MprisEvent::CalibrationTimeout);
                                }
                            }
                            Err(_) => {
                                if tracker.on_step(tracker.anchor_pos) == CalibrationStepResult::TimedOut {
                                    self.emit(MprisEvent::CalibrationTimeout);
                                }
                            }
                        }
                    } else {
                        tracker.disarm();
                    }
                }

                // 1. Seeked signal from active player
                Some(signal) = seeked_stream.next() => {
                    if let Ok(args) = signal.args() {
                        let pos_sec = args.position as f64 / 1_000_000.0;
                        tracker.arm(pos_sec);
                        self.emit(MprisEvent::Seeked { position: pos_sec });
                    }
                }

                // 2. Playback status changed
                Some(_) = status_stream.next() => {
                    let status = proxy.playback_status().await.unwrap_or_else(|_| "Stopped".to_string());
                    if status != self.last_playback_status {
                        self.last_playback_status = status.clone();
                        let position = proxy
                            .position()
                            .await
                            .map(|us| us as f64 / 1_000_000.0)
                            .unwrap_or(0.0);
                        let rate = proxy.rate().await.unwrap_or(1.0);
                        if status == "Playing" {
                            tracker.arm(position);
                        } else {
                            tracker.disarm();
                        }
                        self.emit(MprisEvent::StatusChange {
                            playback_status: status,
                            position,
                            rate,
                        });
                    }
                }

                // 3. Metadata changed
                Some(_) = metadata_stream.next() => {
                    if let Ok(map) = proxy.metadata().await {
                        let new_track = extract_metadata(&map);
                        if new_track != self.last_track {
                            self.last_track = new_track.clone();
                            let position = proxy
                                .position()
                                .await
                                .map(|us| us as f64 / 1_000_000.0)
                                .unwrap_or(0.0);
                            let rate = proxy.rate().await.unwrap_or(1.0);
                            if self.last_playback_status == "Playing" {
                                tracker.arm(position);
                            } else {
                                tracker.disarm();
                            }
                            self.emit(MprisEvent::TrackChange {
                                metadata: new_track,
                                service: self.current_service.clone(),
                                position,
                                playback_status: self.last_playback_status.clone(),
                                rate,
                            });
                        }
                    }
                }

                // 4. Playback rate changed
                Some(_) = rate_stream.next() => {
                    let rate = proxy.rate().await.unwrap_or(1.0);
                    let position = proxy
                        .position()
                        .await
                        .map(|us| us as f64 / 1_000_000.0)
                        .unwrap_or(0.0);
                    self.emit(MprisEvent::StatusChange {
                        playback_status: self.last_playback_status.clone(),
                        position,
                        rate,
                    });
                }


                // 5. Playerctld player order changed
                Some(_) = async {
                    if let Some(ref mut s) = playerctld_stream {
                        s.next().await
                    } else {
                        futures_util::future::pending().await
                    }
                } => {
                    match find_active_service(&self.block_list).await {
                        Ok(Some(ref best_service)) if best_service == &self.current_service => {
                            // Same player remains active: do not tear down streams
                        }
                        Ok(Some(best_service)) => {
                            self.switch_to_player(&best_service).await?;
                            return Ok(());
                        }
                        Ok(None) => {
                            self.deactivate_player();
                            return Ok(());
                        }
                        Err(_) => {}
                    }
                }

                // 6. D-Bus NameOwnerChanged (player exited, launched, or replaced)
                Some(signal) = name_owner_stream.next() => {
                    if let Ok(args) = signal.args() {
                        let name = args.name.as_str();
                        let old_owner = args.old_owner.as_deref().unwrap_or("");
                        let new_owner = args.new_owner.as_deref().unwrap_or("");
                        let is_current = name == self.current_service;
                        let is_mpris = name.starts_with("org.mpris.MediaPlayer2.");
                        let owner_changed = old_owner != new_owner;

                        if is_current && owner_changed {
                            // Reconnect to clear stale proxy cache across PID boundaries
                            self.deactivate_and_rediscover().await?;
                            return Ok(());
                        } else if is_mpris && owner_changed {
                            match find_active_service(&self.block_list).await {
                                Ok(Some(ref best_service)) if best_service != &self.current_service => {
                                    self.switch_to_player(best_service).await?;
                                    return Ok(());
                                }
                                Ok(None) => {
                                    self.deactivate_player();
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracker_initial_state_playing() {
        let tracker = CalibrationTracker::new(true, 0.0);
        assert!(tracker.timer.is_some());
        assert_eq!(tracker.attempts, 0);
        assert!(!tracker.confirmed);
        assert_eq!(tracker.anchor_pos, 0.0);
    }

    #[tokio::test]
    async fn test_tracker_initial_state_paused() {
        let tracker = CalibrationTracker::new(false, 0.0);
        assert!(tracker.timer.is_none());
    }

    #[tokio::test]
    async fn test_tracker_buffering_and_steady_state_convergence() {
        let mut tracker = CalibrationTracker::new(true, 0.0);

        // Step 1: Still buffering at 0.0s
        assert_eq!(tracker.on_step(0.0), CalibrationStepResult::Continue);
        assert_eq!(tracker.attempts, 1);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 2: Still buffering at 0.0s
        assert_eq!(tracker.on_step(0.0), CalibrationStepResult::Continue);
        assert_eq!(tracker.attempts, 2);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 3: Audio starts playing (moved past delta threshold)
        assert_eq!(tracker.on_step(0.4), CalibrationStepResult::Continue);
        assert_eq!(tracker.attempts, 3);
        assert!(tracker.confirmed);
        assert!(tracker.timer.is_some()); // Armed for final confirmation

        // Step 4: Final confirmation step verifies continuous playback
        assert_eq!(tracker.on_step(1.6), CalibrationStepResult::Confirmed);
        assert_eq!(tracker.attempts, 4);
        assert!(tracker.timer.is_none()); // Disarmed!
    }

    #[tokio::test]
    async fn test_tracker_mid_track_resume_buffering() {
        let mut tracker = CalibrationTracker::new(true, 45.0);

        // Step 1: Mid-track buffering (authoritative pos still 45.0)
        assert_eq!(tracker.on_step(45.0), CalibrationStepResult::Continue);
        assert_eq!(tracker.attempts, 1);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 2: Resumed playback moves to 45.3s (delta 0.3 > 0.2)
        assert_eq!(tracker.on_step(45.3), CalibrationStepResult::Continue);
        assert!(tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 3: Steady-state confirmation
        assert_eq!(tracker.on_step(46.5), CalibrationStepResult::Confirmed);
        assert!(tracker.timer.is_none()); // Disarmed
    }

    #[tokio::test]
    async fn test_tracker_max_attempts_timeout() {
        let mut tracker = CalibrationTracker::new(true, 0.0);
        for i in 1..=MAX_CALIBRATION_ATTEMPTS {
            let res = tracker.on_step(0.0);
            if i < MAX_CALIBRATION_ATTEMPTS {
                assert_eq!(res, CalibrationStepResult::Continue);
                assert!(tracker.timer.is_some());
            } else {
                assert_eq!(res, CalibrationStepResult::TimedOut);
                assert!(tracker.timer.is_none()); // Disarmed after max attempts
            }
        }
    }
}
