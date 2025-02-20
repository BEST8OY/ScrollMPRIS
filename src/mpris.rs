use dbus::blocking::{Connection, stdintf::org_freedesktop_dbus::Properties};
use std::collections::HashMap;
use std::time::Duration;
use serde::{Serialize, Deserialize};

const PLAYING_ICON: &str = " 󲒌 ";
const PAUSED_ICON: &str = " 󲏉 ";
const DBUS_TIMEOUT: Duration = Duration::from_millis(500);

///
/// Represents an MPRIS player with its associated service, playback status, title, and artist.
///
#[derive(Debug, Serialize, Deserialize)]
pub struct MprisPlayer {
    pub service: String,
    pub playback_status: String,
    pub title: Option<String>,
    pub artist: Option<String>,
}

impl MprisPlayer {
    ///
    /// Constructs the display parts (icon, metadata text, CSS status class) based on the player's state.
    /// - For "Playing": returns (PLAYING_ICON, "artist - title", "playing")
    /// - For "Paused": returns (PAUSED_ICON, "artist - title", "paused")
    /// - For "Stopped": returns ("", "", "stopped")
    ///
    pub fn output_parts(&self) -> (String, String, String) {
        let status = self.playback_status.to_lowercase();
        if status == "stopped" {
            return (String::new(), String::new(), "stopped".to_string());
        }
        let icon = match status.as_str() {
            "playing" => PLAYING_ICON,
            "paused" => PAUSED_ICON,
            _ => "",
        }
        .to_string();

        let metadata = match (&self.artist, &self.title) {
            (Some(artist), Some(title)) => format!("{} - {}", title, artist),
            (None, Some(title)) => title.clone(),
            _ => self.playback_status.clone(),
        };

        (icon, metadata, status)
    }
}

///
/// Establishes a D-Bus session connection. Returns None if the connection fails.
///
fn dbus_connection() -> Option<Connection> {
    Connection::new_session().ok()
}

///
/// Extracts title and artist from the metadata hash map.
///
fn extract_metadata(
    metadata: &HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>,
) -> (Option<String>, Option<String>) {
    let title = metadata
        .get("xesam:title")
        .and_then(|v| v.0.as_str())
        .map(String::from);

    let artist = metadata
        .get("xesam:artist")
        .and_then(|v| {
            v.0.as_iter()
                .and_then(|mut iter| iter.next())
                .and_then(|item| item.as_str())
                .map(String::from)
        });

    (title, artist)
}

///
/// Retrieves the active MPRIS player using playerctld. Queries for the property "PlayerNames" via the
/// "com.github.altdesktop.playerctld" interface and selects the first available player to fetch its status
/// and metadata. If any query fails, None is returned.
///
pub fn get_active_player() -> Option<MprisPlayer> {
    let conn = dbus_connection()?;

    let proxy = conn.with_proxy(
        "org.mpris.MediaPlayer2.playerctld",
        "/org/mpris/MediaPlayer2",
        DBUS_TIMEOUT,
    );

    let player_names: Vec<String> = proxy.get("com.github.altdesktop.playerctld", "PlayerNames").ok()?;
    let active_service = player_names.first()?.to_string();

    let player_proxy = conn.with_proxy(&active_service, "/org/mpris/MediaPlayer2", DBUS_TIMEOUT);
    let playback_status: String = player_proxy.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus").ok()?;
    let metadata_map: Option<HashMap<String, dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>>> =
        player_proxy.get("org.mpris.MediaPlayer2.Player", "Metadata").ok();

    let (title, artist) = if let Some(ref metadata) = metadata_map {
        extract_metadata(metadata)
    } else {
        (None, None)
    };

    Some(MprisPlayer {
        service: active_service,
        playback_status,
        title,
        artist,
    })
}
