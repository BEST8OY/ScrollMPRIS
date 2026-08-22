//! MPRIS module: re-exports and module declarations for submodules.

pub mod connection;
pub mod events;
pub mod metadata;
pub mod proxies;

#[allow(unused_imports)]
pub use connection::{find_active_service, get_active_player_names, get_position, is_blocked, MprisError};
#[allow(unused_imports)]
pub use metadata::{
    extract_joined_string_array, extract_length_seconds, extract_metadata,
    extract_string_or_first_item, is_no_track, TrackMetadata,
};
#[allow(unused_imports)]
pub use proxies::{MediaPlayer2PlayerProxy, PlayerctldProxy};

