//! Minimal track metadata struct and metadata querying for MPRIS.

use dbus::nonblock::Proxy;
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use crate::mpris::connection::{get_dbus_conn, TIMEOUT, MprisError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Option<f64>,
}

/// Helper to extract a string that might be a single value or the first in an array.
/// The MPRIS spec says artist/album are arrays of strings, but some players send a single string.
fn extract_optional_string(variant: &dbus::arg::Variant<Box<dyn dbus::arg::RefArg + 'static>>) -> Option<String> {
    use dbus::arg::ArgType;
    match variant.0.arg_type() {
        ArgType::Array => {
            if let Some(mut iter) = variant.0.as_iter() {
                iter.next().and_then(|v| v.as_str()).map(str::to_string)
            } else {
                None
            }
        }
        ArgType::String => variant.0.as_str().map(str::to_string),
        _ => None,
    }
}

/// Extract metadata fields from a D-Bus property map.
pub fn extract_metadata(map: &dbus::arg::PropMap) -> TrackMetadata {
    let title = map.get("xesam:title").and_then(|v| v.0.as_str()).map(str::to_string).unwrap_or_default();
    let artist = map.get("xesam:artist").and_then(extract_optional_string).unwrap_or_default();
    let album = map.get("xesam:album").and_then(extract_optional_string).unwrap_or_default();
    let length = map.get("mpris:length").and_then(|v| v.0.as_u64()).map(|l| l as f64 / 1_000_000.0);
    TrackMetadata { title, artist, album, length }
}

/// Query metadata for a specific MPRIS player service.
#[allow(dead_code)]
pub async fn get_metadata(service: &str) -> Result<TrackMetadata, MprisError> {
    if service.is_empty() {
        return Ok(TrackMetadata::default());
    }
    let conn = get_dbus_conn().await?;
    let proxy = Proxy::new(service, "/org/mpris/MediaPlayer2", TIMEOUT, conn);
    let metadata: Option<dbus::arg::PropMap> = Properties::get(&proxy, "org.mpris.MediaPlayer2.Player", "Metadata").await.ok();
    Ok(metadata.map(|map| extract_metadata(&map)).unwrap_or_default())
}
