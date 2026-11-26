//! MPRIS module: re-exports and module declarations for submodules.

pub mod connection;
pub mod events;
pub mod metadata;
pub mod proxies;

#[allow(unused_imports)]
pub use connection::{
    MprisError, PlaybackPriority, find_active_service, find_best_active_service,
    get_active_player_names, get_position, get_position_from_proxy, is_blocked,
};
#[allow(unused_imports)]
pub use events::{
    CalibrationStepResult, MOVEMENT_DELTA_THRESHOLD_SECS, MprisEvent, MprisEventHandler,
};
#[allow(unused_imports)]
pub use metadata::{
    TrackMetadata, extract_artists, extract_first_artist_from_str, extract_joined_string_array,
    extract_length_seconds, extract_metadata, extract_string_or_first_item, is_local_media_url,
    is_no_track,
};
#[allow(unused_imports)]
pub use proxies::{MediaPlayer2PlayerProxy, PlayerctldProxy};
