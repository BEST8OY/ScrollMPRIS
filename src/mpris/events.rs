//! Event watching and event handler registration for MPRIS.

use dbus::nonblock::Proxy;
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use dbus::message::MatchRule;
use dbus::channel::MatchingReceiver;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::mpris::connection::{get_active_player_names, is_blocked, TIMEOUT, MprisError};
use crate::mpris::metadata::{TrackMetadata, extract_metadata};

const MPRIS_PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const DBUS_PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const PLAYERCTL_SENDER: &str = "com.github.altdesktop.playerctld";

pub struct MprisEventHandler<F, G>
where
    F: FnMut(TrackMetadata, f64, String, String) + Send + 'static,
    G: FnMut(TrackMetadata, f64, String) + Send + 'static,
{
    on_track_change: F,
    on_seek: G,
    block_list: Arc<Vec<String>>,
    current_service: String,
    last_track: TrackMetadata,
    last_playback_status: String,
    conn: Arc<dbus::nonblock::SyncConnection>,
    msg_rx: mpsc::Receiver<dbus::message::Message>,
}

impl<F, G> MprisEventHandler<F, G>
where
    F: FnMut(TrackMetadata, f64, String, String) + Send + 'static,
    G: FnMut(TrackMetadata, f64, String) + Send + 'static,
{
    pub async fn new(
        on_track_change: F,
        on_seek: G,
        block_list: Vec<String>,
    ) -> Result<Self, MprisError> {
        let (resource, conn) = dbus_tokio::connection::new_session_sync()
            .map_err(|_| MprisError::NoConnection)?;
        tokio::spawn(async move { resource.await });

        let (tx, rx) = mpsc::channel::<dbus::message::Message>(8);

        Self::add_match_rule(&conn, MatchRule::new_signal(DBUS_PROPERTIES_INTERFACE, "PropertiesChanged").static_clone(), tx.clone()).await?;
        Self::add_match_rule(&conn, MatchRule::new_signal(DBUS_PROPERTIES_INTERFACE, "PropertiesChanged").with_sender(PLAYERCTL_SENDER).static_clone(), tx.clone()).await?;
        Self::add_match_rule(&conn, MatchRule::new_signal(MPRIS_PLAYER_INTERFACE, "Seeked").static_clone(), tx.clone()).await?;

        let mut handler = Self {
            on_track_change,
            on_seek,
            block_list: Arc::new(block_list),
            current_service: String::new(),
            last_track: TrackMetadata::default(),
            last_playback_status: String::new(),
            conn,
            msg_rx: rx,
        };

        // Initial player discovery
        if let Ok(names) = get_active_player_names().await {
            if let Some(service) = names.iter().find(|s| !is_blocked(s, &handler.block_list)) {
                handler.update_current_player(service).await?;
            }
        }

        Ok(handler)
    }

    async fn add_match_rule(
        conn: &Arc<dbus::nonblock::SyncConnection>,
        rule: MatchRule<'static>,
        tx: mpsc::Sender<dbus::message::Message>,
    ) -> Result<(), MprisError> {
        conn.add_match(rule.clone()).await?;
        let conn_clone = Arc::clone(conn);
        MatchingReceiver::start_receive(
            &*conn_clone,
            rule,
            Box::new(move |msg, _| {
                let _ = tx.try_send(msg);
                true
            }),
        );
        Ok(())
    }

    async fn update_current_player(&mut self, service: &str) -> Result<(), MprisError> {
        let proxy = Proxy::new(service, "/org/mpris/MediaPlayer2", TIMEOUT, self.conn.clone());
        let metadata: Option<dbus::arg::PropMap> = Properties::get(&proxy, MPRIS_PLAYER_INTERFACE, "Metadata").await.ok();
        let meta = metadata.map(|map| extract_metadata(&map)).unwrap_or_default();
        let position: f64 = Properties::get::<i64>(&proxy, MPRIS_PLAYER_INTERFACE, "Position").await.ok().map(|p| p as f64 / 1_000_000.0).unwrap_or(0.0);
        let playback_status: String = Properties::get::<String>(&proxy, MPRIS_PLAYER_INTERFACE, "PlaybackStatus").await.ok().unwrap_or_else(|| "Stopped".to_string());

        self.current_service = service.to_string();
        self.last_track = meta.clone();
        let playback_status_str = playback_status.clone();
        self.last_playback_status = playback_status;
        (self.on_track_change)(meta, position, playback_status_str, service.to_string());
        Ok(())
    }

    pub async fn handle_events(&mut self) -> Result<(), MprisError> {
        while let Some(msg) = self.msg_rx.recv().await {
            self.handle_message(msg).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, msg: dbus::message::Message) -> Result<(), MprisError> {
        match (msg.interface().as_deref(), msg.member().as_deref()) {
            (Some(MPRIS_PLAYER_INTERFACE), Some("Seeked")) => self.handle_seek(msg).await?,
            (Some(DBUS_PROPERTIES_INTERFACE), _) => self.handle_properties_changed(msg).await?,
            _ => {}
        }
        Ok(())
    }

    async fn handle_seek(&mut self, msg: dbus::message::Message) -> Result<(), MprisError> {
        if self.current_service.is_empty() {
            return Ok(());
        }
        if let Some(pos) = msg.read1::<i64>().ok() {
            let sec = pos as f64 / 1_000_000.0;
            (self.on_seek)(self.last_track.clone(), sec, self.current_service.clone());
        }
        Ok(())
    }

    async fn handle_properties_changed(&mut self, msg: dbus::message::Message) -> Result<(), MprisError> {
        if let Some(interface_name) = msg.read1::<&str>().ok() {
            match interface_name {
                "org.mpris.MediaPlayer2" | "org.freedesktop.DBus.Properties" | "com.github.altdesktop.playerctld" => {
                    self.handle_player_names_changed(msg).await?;
                }
                MPRIS_PLAYER_INTERFACE => {
                    self.handle_player_properties_changed(msg).await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_player_names_changed(&mut self, msg: dbus::message::Message) -> Result<(), MprisError> {
        let changed: Option<dbus::arg::PropMap> = msg.read2().ok().map(|(_, c): (String, dbus::arg::PropMap)| c);
        if let Some(changed) = changed {
            if changed.contains_key("PlayerNames") {
                if let Ok(names) = get_active_player_names().await {
                    if let Some(service) = names.iter().find(|s| !is_blocked(s, &self.block_list)) {
                        if *service != self.current_service {
                            self.update_current_player(service).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_player_properties_changed(&mut self, msg: dbus::message::Message) -> Result<(), MprisError> {
        if self.current_service.is_empty() {
            return Ok(());
        }
        let player_proxy = Proxy::new(
            &self.current_service,
            "/org/mpris/MediaPlayer2",
            TIMEOUT,
            self.conn.clone(),
        );
        let changed: Option<dbus::arg::PropMap> = msg.read2().ok().map(|(_, c): (String, dbus::arg::PropMap)| c);
        if let Some(changed) = changed {
            let mut metadata_changed = false;
            let mut status_changed = false;

            if changed.contains_key("Metadata") {
                if let Ok(metadata) = Properties::get::<dbus::arg::PropMap>(&player_proxy, MPRIS_PLAYER_INTERFACE, "Metadata").await {
                    let new_track = extract_metadata(&metadata);
                    if new_track != self.last_track {
                        self.last_track = new_track;
                        metadata_changed = true;
                    }
                }
            }

            if changed.contains_key("PlaybackStatus") {
                if let Ok(status) = Properties::get::<String>(&player_proxy, MPRIS_PLAYER_INTERFACE, "PlaybackStatus").await {
                    if status != self.last_playback_status {
                        self.last_playback_status = status;
                        status_changed = true;
                    }
                }
            }

            if changed.contains_key("Position") {
                if let Some(pos_var) = changed.get("Position") {
                    if let Some(pos) = pos_var.0.as_i64() {
                        let sec = pos as f64 / 1_000_000.0;
                        (self.on_seek)(self.last_track.clone(), sec, self.current_service.clone());
                    }
                }
            }

            if metadata_changed || status_changed {
                let position = Properties::get::<i64>(&player_proxy, MPRIS_PLAYER_INTERFACE, "Position")
                    .await
                    .map(|p| p as f64 / 1_000_000.0)
                    .unwrap_or(0.0);
                (self.on_track_change)(self.last_track.clone(), position, self.last_playback_status.clone(), self.current_service.clone());
            }
        }
        Ok(())
    }
}
