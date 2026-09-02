//! D-Bus connection management and player discovery for MPRIS.

use tokio::sync::OnceCell;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use crate::mpris::proxies::{MediaPlayer2PlayerProxy, PlayerctldProxy};

/// Errors that can occur during MPRIS operations.
#[derive(thiserror::Error, Debug)]
pub enum MprisError {
    #[error("D-Bus error: {0}")]
    ZBus(#[from] zbus::Error),
    #[error("D-Bus FDO error: {0}")]
    Fdo(#[from] zbus::fdo::Error),
    #[error("No connection to D-Bus: {0}")]
    NoConnection(zbus::Error),
}

/// Global D-Bus connection singleton
static DBUS_CONNECTION: OnceCell<zbus::Connection> = OnceCell::const_new();

/// Get or create a shared D-Bus session connection.
pub async fn get_dbus_conn() -> Result<zbus::Connection, MprisError> {
    DBUS_CONNECTION
        .get_or_try_init(|| async {
            zbus::Connection::session()
                .await
                .map_err(MprisError::NoConnection)
        })
        .await
        .cloned()
}

/// Get list of active MPRIS player service names using dual-tier discovery.
///
/// Tier 1: Query playerctld (recency-ordered active players).
/// Tier 2: Fallback to standard D-Bus daemon ListNames (for environments without playerctld).
pub async fn get_active_player_names() -> Result<Vec<String>, MprisError> {
    let conn = get_dbus_conn().await?;
    let dbus_proxy = DBusProxy::new(&conn).await?;

    // Tier 1: Query playerctld (check name_has_owner first to avoid unintended D-Bus autostart)
    if dbus_proxy
        .name_has_owner(BusName::from_static_str("org.mpris.MediaPlayer2.playerctld").unwrap())
        .await
        .unwrap_or(false)
        && let Ok(proxy) = PlayerctldProxy::new(&conn).await
        && let Ok(names) = proxy.player_names().await
        && !names.is_empty()
    {
        return Ok(names);
    }

    // Tier 2: Fallback to D-Bus daemon ListNames
    let names = dbus_proxy.list_names().await?;
    let mpris_names: Vec<String> = names
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with("org.mpris.MediaPlayer2."))
        .collect();

    Ok(mpris_names)
}

/// Find the first active MPRIS player service that is not in the block list.
pub async fn find_active_service(block_list: &[String]) -> Result<Option<String>, MprisError> {
    let names = get_active_player_names().await?;
    Ok(names.into_iter().find(|s| !is_blocked(s, block_list)))
}

/// Check if a player service name should be blocked (case-insensitive substring match).
pub fn is_blocked(service: &str, block_list: &[String]) -> bool {
    let service_lower = service.to_lowercase();
    block_list
        .iter()
        .any(|b| service_lower.contains(&b.to_lowercase()))
}

/// Query current dynamic position directly using native uncached property proxy.
pub async fn get_position(service: &str) -> Result<f64, MprisError> {
    if service.is_empty() {
        return Ok(0.0);
    }
    let conn = get_dbus_conn().await?;
    let proxy = MediaPlayer2PlayerProxy::builder(&conn)
        .destination(service.to_string())?
        .build()
        .await?;

    let microsecs = proxy.position().await?;
    Ok(microsecs as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blocked() {
        let block_list = vec!["firefox".to_string(), "vlc".to_string()];
        assert!(is_blocked(
            "org.mpris.MediaPlayer2.firefox.instance_1",
            &block_list
        ));
        assert!(is_blocked("org.mpris.MediaPlayer2.vlc", &block_list));
        assert!(is_blocked("org.mpris.MediaPlayer2.FIREFOX", &block_list));
        assert!(!is_blocked("org.mpris.MediaPlayer2.spotify", &block_list));
        assert!(!is_blocked("org.mpris.MediaPlayer2.mpv", &block_list));
    }

    #[test]
    fn test_is_blocked_empty_list() {
        let empty_list = vec![];
        assert!(!is_blocked("org.mpris.MediaPlayer2.spotify", &empty_list));
    }

    #[tokio::test]
    async fn test_get_position_empty_service() {
        assert_eq!(get_position("").await.unwrap(), 0.0);
    }

    #[tokio::test]
    async fn test_get_position_invalid_bus_name() {
        assert!(get_position("invalid name with spaces").await.is_err());
    }
}
