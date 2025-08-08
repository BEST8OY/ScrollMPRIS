//! Minimal D-Bus connection and player discovery for MPRIS.

use dbus::nonblock::{SyncConnection, Proxy};
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use std::sync::Arc;
use std::time::Duration;

pub const TIMEOUT: Duration = Duration::from_millis(5000);

#[derive(thiserror::Error, Debug)]
pub enum MprisError {
    #[error("DBus error: {0}")]
    DBus(#[from] dbus::Error),
    #[error("No connection to D-Bus")]
    NoConnection,
}

pub async fn get_dbus_conn() -> Result<Arc<SyncConnection>, MprisError> {
    static ONCE: once_cell::sync::OnceCell<Arc<SyncConnection>> = once_cell::sync::OnceCell::new();
    if let Some(conn) = ONCE.get() {
        return Ok(conn.clone());
    }
    let (resource, conn) = dbus_tokio::connection::new_session_sync()
        .map_err(|_| MprisError::NoConnection)?;
    tokio::spawn(async move { resource.await });
    let _ = ONCE.set(conn.clone());
    Ok(conn)
}

pub async fn get_active_player_names() -> Result<Vec<String>, MprisError> {
    let conn = get_dbus_conn().await?;
    let proxy = Proxy::new(
        "org.mpris.MediaPlayer2.playerctld",
        "/org/mpris/MediaPlayer2",
        TIMEOUT,
        conn,
    );
    let result = Properties::get(&proxy, "com.github.altdesktop.playerctld", "PlayerNames").await;
    Ok(result.unwrap_or_default())
}

pub fn is_blocked(service: &str, block_list: &[String]) -> bool {
    block_list.iter().any(|b| service.to_lowercase().contains(b))
}
