//! Track metadata struct and metadata querying for MPRIS using zbus.

use std::collections::HashMap;
use zvariant::OwnedValue;

use crate::mpris::connection::{MprisError, get_dbus_conn};
use crate::mpris::proxies::MediaPlayer2PlayerProxy;

/// Normalized track metadata from an MPRIS player.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Option<f64>,
}

/// Check if an MPRIS `mpris:trackid` represents the canonical "NoTrack" sentinel
/// defined in the MPRIS specification (e.g. `/org/mpris/MediaPlayer2/TrackList/NoTrack`,
/// `/org/mpris/MediaPlayer2/NoTrack`, `NoTrack`, or `/`).
pub fn is_no_track(trackid: &str) -> bool {
    let trimmed = trackid.trim();
    trimmed.is_empty()
        || trimmed == "/"
        || trimmed.ends_with("/NoTrack")
        || trimmed.eq_ignore_ascii_case("NoTrack")
}

/// Helper to extract a string from an `OwnedValue` that may be a `Str` or an `Array` of strings.
pub fn extract_string_or_first_item(val: &OwnedValue) -> Option<String> {
    if let Ok(s) = <&str>::try_from(val) {
        let trimmed = s.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    match &**val {
        zvariant::Value::Str(s) => {
            let trimmed = s.as_str().trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        zvariant::Value::Array(arr) => {
            for elem in arr.iter() {
                if let Ok(s) = <&str>::try_from(elem) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Helper to extract string or join array of strings with commas (for multi-artist / multi-album).
pub fn extract_joined_string_array(val: &OwnedValue) -> Option<String> {
    if let Ok(s) = <&str>::try_from(val) {
        let trimmed = s.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    match &**val {
        zvariant::Value::Str(s) => {
            let trimmed = s.as_str().trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        zvariant::Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|elem| {
                    if let Ok(s) = <&str>::try_from(elem) {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            Some(trimmed.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if strings.is_empty() {
                None
            } else {
                Some(strings.join(", "))
            }
        }
        _ => None,
    }
}

/// Helper to extract track length in seconds (from microseconds).
pub fn extract_length_seconds(val: &OwnedValue) -> Option<f64> {
    if let Ok(microsecs) = i64::try_from(val) {
        return Some(microsecs as f64 / 1_000_000.0);
    }
    if let Ok(microsecs) = u64::try_from(val) {
        return Some(microsecs as f64 / 1_000_000.0);
    }
    None
}

/// Extract metadata fields from a D-Bus property map.
pub fn extract_metadata(map: &HashMap<String, OwnedValue>) -> TrackMetadata {
    let trackid = map
        .get("mpris:trackid")
        .and_then(extract_string_or_first_item);
    let title = map
        .get("xesam:title")
        .and_then(extract_string_or_first_item)
        .unwrap_or_default();

    // Check for explicit "NoTrack" sentinel
    if let Some(ref id) = trackid
        && is_no_track(id)
        && title.is_empty()
    {
        return TrackMetadata::default();
    }

    let artist = map
        .get("xesam:artist")
        .and_then(extract_joined_string_array)
        .unwrap_or_default();
    let album = map
        .get("xesam:album")
        .and_then(extract_joined_string_array)
        .unwrap_or_default();
    let length = map.get("mpris:length").and_then(extract_length_seconds);

    TrackMetadata {
        title,
        artist,
        album,
        length,
    }
}

/// Query metadata for a specific MPRIS player service.
#[allow(dead_code)]
pub async fn get_metadata(service: &str) -> Result<TrackMetadata, MprisError> {
    if service.is_empty() {
        return Ok(TrackMetadata::default());
    }
    let conn = get_dbus_conn().await?;
    let proxy = MediaPlayer2PlayerProxy::builder(&conn)
        .destination(service)?
        .build()
        .await?;

    let map = proxy.metadata().await.unwrap_or_default();
    Ok(extract_metadata(&map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Value;

    #[test]
    fn test_extract_metadata_standard() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".to_string(),
            OwnedValue::try_from(Value::from("Bohemian Rhapsody")).unwrap(),
        );
        map.insert(
            "xesam:artist".to_string(),
            OwnedValue::try_from(Value::from(vec!["Queen"])).unwrap(),
        );
        map.insert(
            "xesam:album".to_string(),
            OwnedValue::try_from(Value::from("A Night at the Opera")).unwrap(),
        );
        map.insert(
            "mpris:length".to_string(),
            OwnedValue::try_from(Value::from(354_000_000i64)).unwrap(),
        );

        let meta = extract_metadata(&map);
        assert_eq!(meta.title, "Bohemian Rhapsody");
        assert_eq!(meta.artist, "Queen");
        assert_eq!(meta.album, "A Night at the Opera");
        assert_eq!(meta.length, Some(354.0));
    }

    #[test]
    fn test_extract_metadata_single_string_artist() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".to_string(),
            OwnedValue::try_from(Value::from("Starboy")).unwrap(),
        );
        map.insert(
            "xesam:artist".to_string(),
            OwnedValue::try_from(Value::from("The Weeknd")).unwrap(),
        );
        map.insert(
            "xesam:album".to_string(),
            OwnedValue::try_from(Value::from("Starboy")).unwrap(),
        );

        let meta = extract_metadata(&map);
        assert_eq!(meta.title, "Starboy");
        assert_eq!(meta.artist, "The Weeknd");
        assert_eq!(meta.album, "Starboy");
    }

    #[test]
    fn test_extract_metadata_empty() {
        let map = HashMap::new();
        let meta = extract_metadata(&map);
        assert_eq!(meta, TrackMetadata::default());
    }

    #[test]
    fn test_extract_metadata_u64_length() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".to_string(),
            OwnedValue::try_from(Value::from("Song")).unwrap(),
        );
        map.insert(
            "mpris:length".to_string(),
            OwnedValue::try_from(Value::from(180_000_000u64)).unwrap(),
        );

        let meta = extract_metadata(&map);
        assert_eq!(meta.title, "Song");
        assert_eq!(meta.length, Some(180.0));
    }

    #[test]
    fn test_extract_metadata_array_fields() {
        let mut map = HashMap::new();
        map.insert(
            "xesam:title".to_string(),
            OwnedValue::try_from(Value::from(vec!["Array Title", "Alt Title"])).unwrap(),
        );
        map.insert(
            "xesam:artist".to_string(),
            OwnedValue::try_from(Value::from(vec!["Artist A", "Artist B"])).unwrap(),
        );
        map.insert(
            "xesam:album".to_string(),
            OwnedValue::try_from(Value::from(vec!["Array Album 1", "Array Album 2"])).unwrap(),
        );

        let meta = extract_metadata(&map);
        assert_eq!(meta.title, "Array Title");
        assert_eq!(meta.artist, "Artist A, Artist B");
        assert_eq!(meta.album, "Array Album 1, Array Album 2");

        let val_str = OwnedValue::try_from(Value::from("single")).unwrap();
        assert_eq!(
            extract_string_or_first_item(&val_str),
            Some("single".to_string())
        );
        assert_eq!(
            extract_joined_string_array(&val_str),
            Some("single".to_string())
        );

        let val_len_i64 = OwnedValue::try_from(Value::from(120_000_000i64)).unwrap();
        assert_eq!(extract_length_seconds(&val_len_i64), Some(120.0));

        let val_len_u64 = OwnedValue::try_from(Value::from(90_000_000u64)).unwrap();
        assert_eq!(extract_length_seconds(&val_len_u64), Some(90.0));
    }

    #[test]
    fn test_notrack_sentinel() {
        assert!(is_no_track("/org/mpris/MediaPlayer2/TrackList/NoTrack"));
        assert!(is_no_track("/org/mpris/MediaPlayer2/NoTrack"));
        assert!(is_no_track("/NoTrack"));
        assert!(is_no_track("NoTrack"));
        assert!(is_no_track("/"));
        assert!(!is_no_track("/org/mpris/MediaPlayer2/track/123"));

        let mut map = HashMap::new();
        map.insert(
            "mpris:trackid".to_string(),
            OwnedValue::try_from(Value::from("/org/mpris/MediaPlayer2/TrackList/NoTrack")).unwrap(),
        );
        map.insert(
            "xesam:artist".to_string(),
            OwnedValue::try_from(Value::from(vec!["Stale Artist"])).unwrap(),
        );

        let meta = extract_metadata(&map);
        assert_eq!(meta, TrackMetadata::default());
    }
}
