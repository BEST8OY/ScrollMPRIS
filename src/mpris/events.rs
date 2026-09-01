//! Event watching and event handler registration for MPRIS using zbus.

use futures_util::StreamExt;
use std::pin::Pin;
use std::time::Duration;
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};

use crate::mpris::connection::{MprisError, find_active_service, get_dbus_conn, get_position};
use crate::mpris::metadata::{TrackMetadata, extract_metadata};
use crate::mpris::proxies::{MediaPlayer2PlayerProxy, PlayerctldProxy};

/// Tuning constants for transient buffering position calibration.
pub const CALIBRATION_INITIAL_DELAY: Duration = Duration::from_millis(400);
pub const CALIBRATION_POLL_INTERVAL: Duration = Duration::from_millis(800);
pub const MAX_CALIBRATION_ATTEMPTS: u8 = 15;
pub const MOVEMENT_DELTA_THRESHOLD_SECS: f64 = 0.08;

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
    fn on_step(&mut self, real_pos: f64) {
        self.attempts += 1;
        let moved = (real_pos - self.anchor_pos).abs() > MOVEMENT_DELTA_THRESHOLD_SECS;

        if moved {
            if !self.confirmed {
                // Audio started advancing; schedule 1 final confirmation check to lock in steady-state sync
                self.confirmed = true;
                self.timer = Some(Box::pin(tokio::time::sleep(CALIBRATION_POLL_INTERVAL)));
            } else {
                // Steady-state verified: disarm calibration for the remainder of this track
                self.disarm();
            }
        } else if self.attempts < MAX_CALIBRATION_ATTEMPTS {
            // Still buffering (real_pos unchanged from anchor); retry
            self.timer = Some(Box::pin(tokio::time::sleep(CALIBRATION_POLL_INTERVAL)));
        } else {
            // Reached maximum attempts (slow network timeout): disarm
            self.disarm();
        }
    }
}

/// Event handler managing MPRIS signals, player discovery, and lifecycle monitoring.
pub struct MprisEventHandler<F, G, H>
where
    F: FnMut(TrackMetadata, f64, String, String, f64) + Send + 'static,
    G: FnMut(TrackMetadata, f64, String) + Send + 'static,
    H: FnMut(f64) + Send + 'static,
{
    on_track_change: F,
    on_seek: G,
    on_calibrate: H,
    block_list: Vec<String>,
    current_service: String,
    last_track: TrackMetadata,
    last_playback_status: String,
    conn: zbus::Connection,
}

impl<F, G, H> MprisEventHandler<F, G, H>
where
    F: FnMut(TrackMetadata, f64, String, String, f64) + Send + 'static,
    G: FnMut(TrackMetadata, f64, String) + Send + 'static,
    H: FnMut(f64) + Send + 'static,
{
    /// Create a new MPRIS event handler.
    pub async fn new(
        on_track_change: F,
        on_seek: G,
        on_calibrate: H,
        block_list: Vec<String>,
    ) -> Result<Self, MprisError> {
        let conn = get_dbus_conn().await?;

        let mut handler = Self {
            on_track_change,
            on_seek,
            on_calibrate,
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

        (self.on_track_change)(meta, position, playback_status, service.to_string(), rate);
        Ok(())
    }

    /// Reset player state and notify listeners of player deactivation.
    fn deactivate_player(&mut self) {
        self.current_service.clear();
        self.last_track = TrackMetadata::default();
        self.last_playback_status.clear();

        (self.on_track_change)(
            TrackMetadata::default(),
            0.0,
            String::new(),
            String::new(),
            1.0,
        );
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
        let mut idle_ticker = tokio::time::interval(Duration::from_secs(2));
        idle_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            if !self.current_service.is_empty() {
                // Active player loop
                if let Err(e) = self
                    .run_active_player_loop(&dbus_proxy, &mut name_owner_stream)
                    .await
                {
                    eprintln!("Error in active player loop: {e}");
                    self.deactivate_and_rediscover().await?;
                }
            } else {
                // Idle loop: wait for NameOwnerChanged signal or periodic ticker
                tokio::select! {
                    _ = idle_ticker.tick() => {
                        let _ = self.discover_active_player().await;
                    }
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
                }
            }
        }
    }

    /// Inner event loop for the currently active player.
    async fn run_active_player_loop(
        &mut self,
        dbus_proxy: &DBusProxy<'static>,
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

        // Subscribe to playerctld if available
        let playerctld_proxy = PlayerctldProxy::new(&self.conn).await.ok();
        let mut playerctld_stream = if let Some(ref p) = playerctld_proxy {
            Some(p.receive_player_names_changed().await)
        } else {
            None
        };

        let mut liveness_ticker = tokio::time::interval(Duration::from_secs(2));
        liveness_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Initialize transient calibration tracker with authoritative player position
        let initial_pos = get_position(&service).await.unwrap_or(0.0);
        let mut tracker =
            CalibrationTracker::new(self.last_playback_status == "Playing", initial_pos);

        loop {
            tokio::select! {
                // 0. Transient post-buffering position calibration for streaming players (e.g. Spotify)
                _ = tracker.tick() => {
                    if self.last_playback_status == "Playing" && !self.current_service.is_empty() {
                        let real_pos = get_position(&self.current_service).await.unwrap_or(0.0);
                        (self.on_calibrate)(real_pos);
                        tracker.on_step(real_pos);
                    } else {
                        tracker.disarm();
                    }
                }

                // 1. Seeked signal from active player
                Some(signal) = seeked_stream.next() => {
                    if let Ok(args) = signal.args() {
                        let pos_sec = args.position as f64 / 1_000_000.0;
                        tracker.arm(pos_sec);
                        (self.on_seek)(self.last_track.clone(), pos_sec, self.current_service.clone());
                    }
                }

                // 2. Playback status changed
                Some(_) = status_stream.next() => {
                    let status = proxy.playback_status().await.unwrap_or_else(|_| "Stopped".to_string());
                    if status != self.last_playback_status {
                        self.last_playback_status = status.clone();
                        let position = get_position(&self.current_service).await.unwrap_or(0.0);
                        let rate = proxy.rate().await.unwrap_or(1.0);
                        if status == "Playing" {
                            tracker.arm(position);
                        } else {
                            tracker.disarm();
                        }
                        (self.on_track_change)(
                            self.last_track.clone(),
                            position,
                            status,
                            self.current_service.clone(),
                            rate,
                        );
                    }
                }

                // 3. Metadata changed
                Some(_) = metadata_stream.next() => {
                    if let Ok(map) = proxy.metadata().await {
                        let new_track = extract_metadata(&map);
                        if new_track != self.last_track {
                            self.last_track = new_track.clone();
                            let position = get_position(&self.current_service).await.unwrap_or(0.0);
                            let rate = proxy.rate().await.unwrap_or(1.0);
                            if self.last_playback_status == "Playing" {
                                tracker.arm(position);
                            } else {
                                tracker.disarm();
                            }
                            (self.on_track_change)(
                                new_track,
                                position,
                                self.last_playback_status.clone(),
                                self.current_service.clone(),
                                rate,
                            );
                        }
                    }
                }

                // 4. Playback rate changed
                Some(_) = rate_stream.next() => {
                    let rate = proxy.rate().await.unwrap_or(1.0);
                    let position = get_position(&self.current_service).await.unwrap_or(0.0);
                    (self.on_track_change)(
                        self.last_track.clone(),
                        position,
                        self.last_playback_status.clone(),
                        self.current_service.clone(),
                        rate,
                    );
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

                // 7. Authoritative liveness check via D-Bus daemon
                _ = liveness_ticker.tick() => {
                    let has_owner = match zbus::names::BusName::try_from(self.current_service.as_str()) {
                        Ok(bus_name) => matches!(
                            tokio::time::timeout(
                                Duration::from_secs(3),
                                dbus_proxy.name_has_owner(bus_name)
                            ).await,
                            Ok(Ok(true))
                        ),
                        Err(_) => false,
                    };

                    if !has_owner {
                        self.deactivate_and_rediscover().await?;
                        return Ok(());
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
        tracker.on_step(0.0);
        assert_eq!(tracker.attempts, 1);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 2: Still buffering at 0.0s
        tracker.on_step(0.0);
        assert_eq!(tracker.attempts, 2);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 3: Audio starts playing (moved past delta threshold)
        tracker.on_step(0.4);
        assert_eq!(tracker.attempts, 3);
        assert!(tracker.confirmed);
        assert!(tracker.timer.is_some()); // Armed for final confirmation

        // Step 4: Final confirmation step verifies continuous playback
        tracker.on_step(1.6);
        assert_eq!(tracker.attempts, 4);
        assert!(tracker.timer.is_none()); // Disarmed!
    }

    #[tokio::test]
    async fn test_tracker_mid_track_resume_buffering() {
        let mut tracker = CalibrationTracker::new(true, 45.0);

        // Step 1: Mid-track buffering (authoritative pos still 45.0)
        tracker.on_step(45.0);
        assert_eq!(tracker.attempts, 1);
        assert!(!tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 2: Resumed playback moves to 45.3s (delta 0.3 > 0.2)
        tracker.on_step(45.3);
        assert!(tracker.confirmed);
        assert!(tracker.timer.is_some());

        // Step 3: Steady-state confirmation
        tracker.on_step(46.5);
        assert!(tracker.timer.is_none()); // Disarmed
    }

    #[tokio::test]
    async fn test_tracker_max_attempts_timeout() {
        let mut tracker = CalibrationTracker::new(true, 0.0);
        for i in 1..=MAX_CALIBRATION_ATTEMPTS {
            tracker.on_step(0.0);
            if i < MAX_CALIBRATION_ATTEMPTS {
                assert!(tracker.timer.is_some());
            } else {
                assert!(tracker.timer.is_none()); // Disarmed after max attempts
            }
        }
    }
}
