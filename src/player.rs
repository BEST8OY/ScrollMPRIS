// Minimal state data structures for lyrics and player

use crate::mpris::metadata::TrackMetadata;
use std::time::Instant;

#[derive(Debug, PartialEq, Default)]
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
}

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
    pub fn update_playback_dbus(&mut self, playback_status: String, position: f64) {
        self.playing = playback_status == "Playing";
        self.status = playback_status;
        self.last_position = position;
        self.last_update = Some(Instant::now());
        self.position = position;
    }
    pub fn estimate_position(&self) -> f64 {
        if self.playing {
            if let Some(instant) = self.last_update {
                let elapsed = instant.elapsed().as_secs_f64();
                return self.last_position + elapsed;
            }
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
}
