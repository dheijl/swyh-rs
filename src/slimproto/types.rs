//! [`SlimRenderer`]: a connected SlimProto (squeezelite) client, built from
//! its `HELO` handshake.

use crate::globals::statics::{THREAD_STACK, stop_clients_by_ip};
use crate::rendercontrol::StreamInfo;
use crate::slimproto::audg;
use crate::slimproto::frames::SlimHelo;
use crate::slimproto::strm;
use crate::utils::ui_logger::{LogCategory, ui_log};
use ecow::EcoString;
#[cfg(feature = "gui")]
use fltk::button::LightButton;
#[cfg(feature = "gui")]
use fltk::valuator::HorNiceSlider;
use std::io::{self, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(feature = "gui")]
#[derive(Debug, Clone, Default)]
/// The UI elements associated with a SlimProto renderer.
pub struct SlimRendUI {
    pub button: Option<LightButton>,
    pub slider: Option<HorNiceSlider>,
}

/// A connected SlimProto client, built from its `HELO` handshake.
/// `write_stream` is a clone of the write half of the client's single
/// long-lived TCP connection (the read half stays owned by
/// `server::handle_connection`'s read loop) — it's how playback commands
/// (`strm`) get sent back to this specific client.
#[derive(Debug, Clone)]
pub struct SlimRenderer {
    pub player_index: usize,
    /// IP only, no port — same convention as `Controller::remote_addr`.
    pub remote_addr: EcoString,
    /// Source port of the current TCP connection's `peer_addr()`. Reconnects
    /// from the same IP get a fresh ephemeral port, so this — together with
    /// `remote_addr` — identifies *which* connection instance this renderer
    /// is currently bound to; used to tell a stale
    /// [`crate::enums::messages::MessageType::SlimDisconnected`] for an
    /// already-superseded connection apart from one for the current
    /// connection.
    pub peer_port: u16,
    pub mac: [u8; 6],
    pub device_id: u8,
    pub capabilities: String,
    pub playing: bool,
    /// The slider's starting *display* position only — unlike UPnP's
    /// `GetVolume`, squeezelite exposes no "what's your current volume"
    /// query at `HELO` time, so this starts out at 20 % fixed.
    pub volume: i32,
    pub write_stream: Arc<Mutex<TcpStream>>,
    #[cfg(feature = "gui")]
    pub rend_ui: SlimRendUI,
}

impl SlimRenderer {
    pub fn from_helo(
        helo: &SlimHelo,
        remote_addr: EcoString,
        peer_port: u16,
        write_stream: Arc<Mutex<TcpStream>>,
    ) -> Self {
        Self {
            player_index: 0,
            remote_addr,
            peer_port,
            mac: helo.mac,
            device_id: helo.device_id,
            capabilities: helo.capabilities.clone(),
            playing: false,
            volume: 20,
            write_stream,
            #[cfg(feature = "gui")]
            rend_ui: SlimRendUI::default(),
        }
    }

    /// Tell this client to open an HTTP connection back to
    /// `server_ip:streaminfo.server_port` and start decoding
    /// `streaminfo.streaming_format`. Runs synchronously (blocks on the
    /// mutex + a socket write) — call from a background thread, never the
    /// UI thread.
    pub fn send_strm_start(&self, server_ip: Ipv4Addr, streaminfo: StreamInfo) -> io::Result<()> {
        let frame = strm::build_strm_start(server_ip, &streaminfo)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.write_stream
            .lock()
            .expect("SlimRenderer write_stream mutex poisoned")
            .write_all(&frame)
    }

    /// Tell this client to stop playback immediately, and force-drop its
    /// HTTP streaming connection on our own server rather than waiting for
    /// the client to close it (it may not, promptly or ever — see
    /// [`crate::globals::statics::stop_clients_by_ip`]). See
    /// [`Self::send_strm_start`] for threading notes.
    pub fn send_strm_stop(&self) -> io::Result<()> {
        let frame = strm::build_strm_stop();
        let result = self
            .write_stream
            .lock()
            .expect("SlimRenderer write_stream mutex poisoned")
            .write_all(&frame);
        stop_clients_by_ip(&self.remote_addr);
        result
    }

    /// Send a `strm` "status" (time) request — a heartbeat, not a playback
    /// command. See [`crate::slimproto::strm::build_strm_status`] for why
    /// this needs to be sent periodically at all.
    pub fn send_strm_status(&self) -> io::Result<()> {
        let frame = strm::build_strm_status();
        self.write_stream
            .lock()
            .expect("SlimRenderer write_stream mutex poisoned")
            .write_all(&frame)
    }

    /// [`Self::send_strm_start`] on a background thread, so the caller (the
    /// FLTK button callback) is never blocked on the socket write. Mirrors
    /// [`crate::rendercontrol::Renderer::spawn_play`].
    pub fn spawn_strm_start(&self, server_ip: Ipv4Addr, streaminfo: StreamInfo) {
        let renderer = self.clone();
        let spawned = thread::Builder::new()
            .name("slim_strm_start".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                if let Err(e) = renderer.send_strm_start(server_ip, streaminfo) {
                    log::error!(
                        "SlimProto: failed to start playback on {}: {e}",
                        renderer.remote_addr
                    );
                }
            });
        if let Err(e) = spawned {
            log::error!("SlimProto: failed to spawn strm-start thread: {e}");
        }
    }

    /// [`Self::send_strm_stop`] on a background thread. See
    /// [`Self::spawn_strm_start`] for why.
    pub fn spawn_strm_stop(&self) {
        let renderer = self.clone();
        let spawned = thread::Builder::new()
            .name("slim_strm_stop".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                if let Err(e) = renderer.send_strm_stop() {
                    log::error!(
                        "SlimProto: failed to stop playback on {}: {e}",
                        renderer.remote_addr
                    );
                }
            });
        if let Err(e) = spawned {
            log::error!("SlimProto: failed to spawn strm-stop thread: {e}");
        }
    }

    /// Send an `audg` (audio gain) frame setting this client's volume to
    /// `volume_percent`. See [`Self::send_strm_start`] for threading notes.
    pub fn send_set_volume(&self, volume_percent: i32) -> io::Result<()> {
        let frame = audg::build_audg(volume_percent);
        ui_log(
            LogCategory::Info,
            &format!(
                "SlimProto set volume to {volume_percent}% on {}:{}",
                self.model(),
                self.remote_addr
            ),
        );
        self.write_stream
            .lock()
            .expect("SlimRenderer write_stream mutex poisoned")
            .write_all(&frame)
    }

    /// [`Self::send_set_volume`] on a background thread. See
    /// [`Self::spawn_strm_start`] for why.
    pub fn spawn_set_volume(&self, volume_percent: i32) {
        let renderer = self.clone();
        let spawned = thread::Builder::new()
            .name("slim_set_volume".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                if let Err(e) = renderer.send_set_volume(volume_percent) {
                    log::error!(
                        "SlimProto: failed to set volume on {}: {e}",
                        renderer.remote_addr
                    );
                }
            });
        if let Err(e) = spawned {
            log::error!("SlimProto: failed to spawn set-volume thread: {e}");
        }
    }

    /// Best-effort display model, parsed out of the `Model=...` key in the
    /// `HELO` capabilities string (e.g. `Model=squeezelite,...`); falls back
    /// to a generic label if absent.
    pub fn model(&self) -> &str {
        self.capabilities
            .split(',')
            .find_map(|kv| kv.strip_prefix("Model="))
            .unwrap_or("SlimProto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn helo(capabilities: &str) -> SlimHelo {
        SlimHelo {
            device_id: 12,
            revision: 0,
            mac: [0, 1, 2, 3, 4, 5],
            wlan_channel_list: 0,
            bytes_received: 0,
            capabilities: capabilities.to_string(),
        }
    }

    /// A connected loopback `TcpStream`, just to satisfy `SlimRenderer`'s
    /// `write_stream` field in tests that don't exercise actual IO.
    fn dummy_stream() -> Arc<Mutex<TcpStream>> {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
        let addr = listener.local_addr().expect("local_addr");
        Arc::new(Mutex::new(
            TcpStream::connect(addr).expect("connect loopback stream"),
        ))
    }

    #[test]
    fn model_parses_model_capability() {
        let r = SlimRenderer::from_helo(
            &helo("Model=squeezelite,AccuratePlayPoints=1,Firmware=v1.0"),
            "192.168.1.50".into(),
            12345,
            dummy_stream(),
        );
        assert_eq!(r.model(), "squeezelite");
    }

    #[test]
    fn model_falls_back_when_absent() {
        let r = SlimRenderer::from_helo(
            &helo("AccuratePlayPoints=1"),
            "192.168.1.50".into(),
            12345,
            dummy_stream(),
        );
        assert_eq!(r.model(), "SlimProto");
    }
}
