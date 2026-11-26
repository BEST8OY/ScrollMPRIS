// Minimal state data structures for lyrics and player

use crate::mpris::metadata::{TrackMetadata, extract_first_artist_from_str};
use std::time::Instant;

#[derive(Debug, PartialEq)]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
    pub first_artist: String,
    pub album: String,
    pub playing: bool,
    pub status: String,
    pub position: f64,
    pub err: Option<String>,
    pub last_position: f64,
    pub last_update: Option<Instant>,
    pub length: Option<f64>,
    pub service: Option<String>,
    pub rate: f64,
    pub calibrated: bool,
    pub is_local: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            first_artist: String::new(),
            album: String::new(),
            playing: false,
            status: String::new(),
            position: 0.0,
            err: None,
            last_position: 0.0,
            last_update: None,
            length: None,
            service: None,
            rate: 1.0,
            calibrated: false,
            is_local: false,
        }
    }
}

/// Default position drift threshold (in seconds) before applying calibration corrections.
pub const DEFAULT_CALIBRATION_DRIFT_THRESHOLD: f64 = 0.10;

impl PlayerState {
    pub fn update_from_metadata(&mut self, meta: &TrackMetadata) {
        self.title = meta.title.clone();
        self.artist = meta.artist.clone();
        self.first_artist = meta.first_artist.clone();
        self.album = meta.album.clone();
        self.length = meta.length;
        self.is_local = meta.is_local;
        self.position = 0.0;
        self.err = None;
        self.last_position = 0.0;
        self.last_update = Some(Instant::now());
        self.calibrated = false;
        // service should be set elsewhere
    }

    /// Returns the primary/first artist. Uses `first_artist` if available,
    /// or falls back to extracting from `artist`.
    pub fn get_first_artist(&self) -> &str {
        if !self.first_artist.is_empty() {
            &self.first_artist
        } else {
            extract_first_artist_from_str(&self.artist)
        }
    }

    pub fn set_service(&mut self, service: &str) {
        self.service = Some(service.to_string());
    }

    pub fn get_service(&self) -> Option<&str> {
        self.service.as_deref()
    }

    /// Guard against spurious near-zero position reports during active mid-track playback
    /// unless the track is looping back from near its end.
    fn is_spurious_zero(&self, position: f64, current_pos: f64) -> bool {
        let is_track_loop = self
            .length
            .is_some_and(|len| current_pos >= (len - 3.0).max(1.0));
        position <= 0.05 && !is_track_loop && current_pos > 1.0
    }

    pub fn update_playback_dbus(&mut self, playback_status: String, position: f64, rate: f64) {
        self.playing = playback_status == "Playing";
        self.status = playback_status;
        let current_pos = self.estimate_position();
        let effective_position = if self.is_spurious_zero(position, current_pos) {
            current_pos
        } else {
            position
        };

        self.last_position = effective_position;
        self.last_update = Some(Instant::now());
        self.position = effective_position;
        self.rate = if rate > 0.0 { rate } else { 1.0 };
        // Local file:// tracks start immediately; streams hold until CalibrationTracker confirms movement
        self.calibrated = self.playing && self.is_local;
    }

    /// Update playback rate without altering the current calibration state.
    pub fn update_rate(&mut self, position: f64, rate: f64) {
        let current_pos = self.estimate_position();
        let effective_position = if self.is_spurious_zero(position, current_pos) {
            current_pos
        } else {
            position
        };

        self.last_position = effective_position;
        self.last_update = Some(Instant::now());
        self.position = effective_position;
        self.rate = if rate > 0.0 { rate } else { 1.0 };
    }

    pub fn estimate_position(&self) -> f64 {
        if self.playing
            && self.calibrated
            && let Some(instant) = self.last_update
        {
            let elapsed = instant.elapsed().as_secs_f64() * self.rate;
            return self.last_position + elapsed;
        }
        self.last_position
    }

    #[allow(dead_code)]
    pub fn has_changed(&self, meta: &TrackMetadata) -> bool {
        self.title != meta.title
            || self.artist != meta.artist
            || self.first_artist != meta.first_artist
            || self.album != meta.album
    }

    pub fn reset_position_cache(&mut self, position: f64) {
        self.last_position = position;
        self.last_update = Some(Instant::now());
        self.position = position;
        self.calibrated = self.playing && self.is_local;
    }

    /// Force playback calibration to active state (e.g. after transient calibration timeout).
    pub fn force_calibrate(&mut self) {
        self.calibrated = true;
        self.last_update = Some(Instant::now());
    }

    /// Calibrate estimated position against authoritative player position.
    /// When uncalibrated (e.g. initial track buffering), holds at anchor until movement is detected.
    /// When calibrated, applies corrections only if drift exceeds threshold.
    /// Returns true if state was updated/calibrated.
    pub fn calibrate_position(&mut self, real_position: f64, threshold: f64) -> bool {
        if self.playing {
            let current_pos = self.estimate_position();

            // Guard against spurious near-zero position reports during active playback
            if self.is_spurious_zero(real_position, current_pos) {
                return false;
            }

            if !self.calibrated {
                // Check if audio has started advancing from initial start position
                let delta = (real_position - self.last_position).abs();
                if delta >= threshold || (self.last_position == 0.0 && real_position > 0.03) {
                    self.calibrated = true;
                    self.last_position = real_position;
                    self.last_update = Some(Instant::now());
                    self.position = real_position;
                    return true;
                }
                // Still buffering at anchor: keep original anchor intact so incremental
                // movements accumulate against the baseline.
                return false;
            }

            let diff = (current_pos - real_position).abs();
            if diff >= threshold {
                self.last_position = real_position;
                self.last_update = Some(Instant::now());
                self.position = real_position;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_calibrate_position_detects_drift() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 1.0);
        state.force_calibrate();
        // Simulate 2 seconds where local clock advanced to 12.0s
        state.last_update = Some(Instant::now() - Duration::from_secs(2));

        assert!((state.estimate_position() - 12.0).abs() < 0.1);

        // Authoritative position from player is 10.0s (drift = 2.0s)
        let corrected = state.calibrate_position(10.0, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(corrected);
        assert!((state.estimate_position() - 10.0).abs() < 0.1);
        assert_eq!(state.last_position, 10.0);
    }

    #[test]
    fn test_calibrate_position_holds_at_zero_while_buffering() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        assert!(!state.calibrated);

        // While buffering at 0.0s, even if 2 seconds pass, estimate_position stays 0.0s
        state.last_update = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(state.estimate_position(), 0.0);

        // Audio starts advancing to 0.25s: calibration triggers and begins progression!
        let corrected = state.calibrate_position(0.25, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(corrected);
        assert!(state.calibrated);
        assert!((state.estimate_position() - 0.25).abs() < 0.1);
    }

    #[test]
    fn test_calibrate_position_ignores_small_delta() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 1.0);
        state.force_calibrate();
        state.last_update = Some(Instant::now() - Duration::from_millis(1000));

        // Estimated is ~11.0s; real position reported as 10.95s (drift 0.05s < 0.10s)
        let corrected = state.calibrate_position(10.95, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(!corrected);
    }

    #[test]
    fn test_calibrate_position_noop_when_paused() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Paused".to_string(), 5.0, 1.0);
        let corrected = state.calibrate_position(0.0, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(!corrected);
    }

    #[test]
    fn test_reset_position_cache_uncalibrates_on_seek() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 1.0);
        state.force_calibrate();
        assert!(state.calibrated);

        // Seeking to a non-zero position keeps calibrated = false to stay paused at seek anchor
        state.reset_position_cache(120.0);
        assert!(!state.calibrated);
        assert_eq!(state.last_position, 120.0);
        assert_eq!(state.estimate_position(), 120.0);

        // Seeking to 0.0 also keeps calibrated = false for transient buffering hold
        state.reset_position_cache(0.0);
        assert!(!state.calibrated);
        assert_eq!(state.last_position, 0.0);
        assert_eq!(state.estimate_position(), 0.0);
    }

    #[test]
    fn test_calibrate_position_seek_anchor_requires_threshold() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 1.0);
        state.force_calibrate();
        assert!(state.calibrated);

        // User seeks to 60.0s
        state.reset_position_cache(60.0);
        assert!(!state.calibrated);

        // Authoritative position query returns 60.0s (delta = 0.0 < threshold)
        // Even though real_position > 0.03, because last_position != 0.0, it must NOT calibrate
        let corrected = state.calibrate_position(60.0, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(!corrected);
        assert!(!state.calibrated);
        assert_eq!(state.estimate_position(), 60.0);

        // Delta exceeds threshold -> calibrates and resumes progression
        let corrected = state.calibrate_position(60.15, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(corrected);
        assert!(state.calibrated);
        assert_eq!(state.last_position, 60.15);
    }

    #[test]
    fn test_calibrate_position_accumulates_small_steps_from_anchor() {
        let mut state = PlayerState::default();
        state.reset_position_cache(60.0);
        state.playing = true;

        // First small step (0.05 < 0.10): must not calibrate AND must preserve 60.0 anchor
        assert!(!state.calibrate_position(60.05, DEFAULT_CALIBRATION_DRIFT_THRESHOLD));
        assert!(!state.calibrated);
        assert_eq!(state.last_position, 60.0);

        // Second small step (60.11 - 60.0 = 0.11 >= 0.10): accumulates against preserved anchor
        assert!(state.calibrate_position(60.11, DEFAULT_CALIBRATION_DRIFT_THRESHOLD));
        assert!(state.calibrated);
        assert_eq!(state.last_position, 60.11);
    }

    #[test]
    fn test_update_playback_dbus_holds_non_zero_resume_while_buffering() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 45.0, 1.0);
        assert!(!state.calibrated);

        // Even after simulated elapsed time, timer stays anchored at 45.0 while buffering
        state.last_update = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(state.estimate_position(), 45.0);

        // Calibration confirms audio movement (0.05 >= 0.03)
        assert!(state.calibrate_position(45.05, 0.03));
        assert!(state.calibrated);
    }

    #[test]
    fn test_update_rate_preserves_calibration_state() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 20.0, 1.0);
        assert!(!state.calibrated);

        // Rate change during buffering does not prematurely calibrate
        state.update_rate(20.0, 1.5);
        assert!(!state.calibrated);
        assert_eq!(state.rate, 1.5);

        // Rate change during steady playback preserves calibration
        state.force_calibrate();
        state.update_rate(25.0, 2.0);
        assert!(state.calibrated);
        assert_eq!(state.rate, 2.0);
        assert_eq!(state.last_position, 25.0);
    }

    #[test]
    fn test_force_calibrate() {
        let mut state = PlayerState {
            playing: true,
            ..Default::default()
        };
        assert!(!state.calibrated);

        state.force_calibrate();
        assert!(state.calibrated);
        assert!(state.last_update.is_some());
    }

    #[test]
    fn test_update_playback_dbus_guards_spurious_zero_reset() {
        let mut state = PlayerState {
            length: Some(180.0),
            ..Default::default()
        };
        state.update_playback_dbus("Playing".to_string(), 45.0, 1.0);
        state.force_calibrate();
        assert!(state.calibrated);
        assert_eq!(state.last_position, 45.0);

        // Transient buffering / D-Bus stutter reports 0.0s during mid-track playback
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        // Position must NOT reset to 0.0
        assert!(state.last_position >= 45.0);
    }

    #[test]
    fn test_update_playback_dbus_supports_track_loop() {
        let mut state = PlayerState {
            length: Some(180.0),
            ..Default::default()
        };
        state.update_playback_dbus("Playing".to_string(), 178.5, 1.0);
        state.force_calibrate();
        assert!(state.calibrated);

        // Song loops back to 0.0s near or past end of track
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        // Legitimate loop must be allowed
        assert_eq!(state.last_position, 0.0);
    }

    #[test]
    fn test_calibrate_position_guards_spurious_zero_reset() {
        let mut state = PlayerState {
            length: Some(200.0),
            ..Default::default()
        };
        state.update_playback_dbus("Playing".to_string(), 30.0, 1.0);
        state.force_calibrate();
        assert!(state.calibrated);

        // Spurious calibration tick with 0.0s should be ignored
        let corrected = state.calibrate_position(0.0, DEFAULT_CALIBRATION_DRIFT_THRESHOLD);
        assert!(!corrected);
        assert!(state.estimate_position() >= 30.0);
    }

    #[test]
    fn test_first_artist_resolution() {
        let meta = TrackMetadata {
            title: "Song".to_string(),
            artist: "Queen, David Bowie".to_string(),
            first_artist: "Queen".to_string(),
            album: "Album".to_string(),
            length: Some(180.0),
            is_local: false,
        };
        let mut state = PlayerState::default();
        state.update_from_metadata(&meta);
        assert_eq!(state.first_artist, "Queen");
        assert_eq!(state.get_first_artist(), "Queen");

        // Fallback when first_artist is empty
        let fallback_state = PlayerState {
            artist: "Artist A, Artist B".to_string(),
            ..Default::default()
        };
        assert_eq!(fallback_state.get_first_artist(), "Artist A");
    }

    #[test]
    fn test_local_track_skips_buffering_hold() {
        let local_meta = TrackMetadata {
            title: "Local Track".to_string(),
            is_local: true,
            ..Default::default()
        };
        let mut state = PlayerState::default();
        state.update_from_metadata(&local_meta);
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        assert!(state.calibrated);

        state.reset_position_cache(42.0);
        assert!(state.calibrated);

        // Switching to a stream restores buffering hold
        let stream_meta = TrackMetadata {
            title: "Stream Track".to_string(),
            is_local: false,
            ..Default::default()
        };
        state.update_from_metadata(&stream_meta);
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        assert!(!state.calibrated);
    }
}
