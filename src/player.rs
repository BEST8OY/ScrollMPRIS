// Minimal state data structures for lyrics and player

use crate::mpris::metadata::TrackMetadata;
use std::time::Instant;

#[derive(Debug, PartialEq)]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
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
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
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
        }
    }
}

/// Default position drift threshold (in seconds) before applying calibration corrections.
pub const DEFAULT_CALIBRATION_DRIFT_THRESHOLD: f64 = 0.10;

impl PlayerState {
    pub fn update_from_metadata(&mut self, meta: &TrackMetadata) {
        self.title = meta.title.clone();
        self.artist = meta.artist.clone();
        self.album = meta.album.clone();
        self.length = meta.length;
        self.position = 0.0;
        self.err = None;
        self.last_position = 0.0;
        self.last_update = Some(Instant::now());
        self.calibrated = false;
        // service should be set elsewhere
    }

    pub fn set_service(&mut self, service: &str) {
        self.service = Some(service.to_string());
    }

    pub fn get_service(&self) -> Option<&str> {
        self.service.as_deref()
    }

    pub fn update_playback_dbus(&mut self, playback_status: String, position: f64, rate: f64) {
        self.playing = playback_status == "Playing";
        self.status = playback_status;
        self.last_position = position;
        self.last_update = Some(Instant::now());
        self.position = position;
        self.rate = if rate > 0.0 { rate } else { 1.0 };
        // If playing with non-zero position, audio is already in steady-state playback
        self.calibrated = self.playing && position > 0.0;
    }

    pub fn estimate_position(&self) -> f64 {
        if self.playing && self.calibrated
            && let Some(instant) = self.last_update
        {
            let elapsed = instant.elapsed().as_secs_f64() * self.rate;
            return self.last_position + elapsed;
        }
        self.last_position
    }

    #[allow(dead_code)]
    pub fn has_changed(&self, meta: &TrackMetadata) -> bool {
        self.title != meta.title || self.artist != meta.artist || self.album != meta.album
    }

    pub fn reset_position_cache(&mut self, position: f64) {
        self.last_position = position;
        self.last_update = Some(Instant::now());
        self.position = position;
        self.calibrated = position > 0.0;
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
            if !self.calibrated {
                // Check if audio has started advancing from initial start position
                let delta = (real_position - self.last_position).abs();
                if delta >= threshold || real_position > 0.05 {
                    self.calibrated = true;
                    self.last_position = real_position;
                    self.last_update = Some(Instant::now());
                    self.position = real_position;
                    return true;
                }
                // Still buffering at anchor: refresh anchor to prevent drift
                self.last_position = real_position;
                self.position = real_position;
                return false;
            }

            let estimated = self.estimate_position();
            let diff = (estimated - real_position).abs();
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
    fn test_reset_position_cache_retains_calibration_for_positive_offset() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        assert!(!state.calibrated);

        // Seeking to a non-zero position retains calibration
        state.reset_position_cache(120.0);
        assert!(state.calibrated);
        assert_eq!(state.last_position, 120.0);

        // Seeking to 0.0 uncalibrates for transient buffering hold
        state.reset_position_cache(0.0);
        assert!(!state.calibrated);
        assert_eq!(state.last_position, 0.0);
    }

    #[test]
    fn test_force_calibrate() {
        let mut state = PlayerState::default();
        state.playing = true;
        assert!(!state.calibrated);

        state.force_calibrate();
        assert!(state.calibrated);
        assert!(state.last_update.is_some());
    }
}
