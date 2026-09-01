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
        }
    }
}

/// Default position drift threshold (in seconds) before applying calibration corrections.
pub const DEFAULT_CALIBRATION_DRIFT_THRESHOLD: f64 = 0.25;

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
    }
    pub fn estimate_position(&self) -> f64 {
        if self.playing
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
    }

    /// Calibrate estimated position against authoritative player position.
    /// Updates anchor instant and position only if detected drift exceeds `threshold`.
    /// Returns true if a correction was applied.
    pub fn calibrate_position(&mut self, real_position: f64, threshold: f64) -> bool {
        if self.playing {
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
        state.update_playback_dbus("Playing".to_string(), 0.0, 1.0);
        // Simulate 2 seconds of buffering where local clock advanced to 2.0s
        state.last_update = Some(Instant::now() - Duration::from_secs(2));

        assert!((state.estimate_position() - 2.0).abs() < 0.1);

        // Authoritative position from player is still 0.0s (just finished buffering)
        let corrected = state.calibrate_position(0.0, 0.25);
        assert!(corrected);
        assert!((state.estimate_position() - 0.0).abs() < 0.1);
        assert_eq!(state.last_position, 0.0);
    }

    #[test]
    fn test_calibrate_position_ignores_small_delta() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Playing".to_string(), 10.0, 1.0);
        state.last_update = Some(Instant::now() - Duration::from_millis(1000));

        // Estimated is ~11.0s; real position reported as 10.95s (drift 0.05s < 0.25s)
        let corrected = state.calibrate_position(10.95, 0.25);
        assert!(!corrected);
    }

    #[test]
    fn test_calibrate_position_noop_when_paused() {
        let mut state = PlayerState::default();
        state.update_playback_dbus("Paused".to_string(), 5.0, 1.0);
        let corrected = state.calibrate_position(0.0, 0.25);
        assert!(!corrected);
    }
}
