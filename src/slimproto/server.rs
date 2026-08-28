//! SlimProto TCP control server: accepts client connections, parses
//! frames, and drives `HELO`/`STRM`/`AUDG`/heartbeat traffic for each
//! connected [`SlimRenderer`].

use crate::enums::messages::MessageType;
use crate::globals::statics::{THREAD_STACK, get_msgchannel, get_slim_renderers_mut};
use crate::slimproto::parse_frames::{self, Frame};
use crate::slimproto::types::SlimRenderer;
use anyhow::{Context, Result};
use ecow::{EcoString, eco_format};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// How often to send a `strm 't'` heartbeat on an otherwise-idle control
/// connection. Comfortably under squeezelite's own idle-read timeout
/// (observed ~35-40s in the wild — see
/// [`crate::slimproto::strm::build_strm_status`]).
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

/// Bind the SlimProto TCP control port and accept client connections forever,
/// spawning one thread per connection. Each SlimProto client (squeezelite
/// instance) holds a single long-lived connection, so a thread-per-client
/// model is the natural fit — unlike the HTTP streaming server, there's no
/// need for a fixed pool of acceptor threads serving short-lived requests.
///
/// Binds `local_addr` specifically (like `streaming_server::run_server`
/// does).
pub fn run_server(local_addr: IpAddr, port: u16) -> Result<()> {
    let listener = TcpListener::bind((local_addr, port))
        .with_context(|| format!("failed to bind SlimProto TCP listener on {local_addr}:{port}"))?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let jh = thread::Builder::new()
                    .name("slim_client".into())
                    .stack_size(THREAD_STACK)
                    .spawn(move || handle_connection(stream));
                if let Err(e) = jh {
                    log::error!("failed to spawn SlimProto client thread: {e:?}");
                }
            }
            Err(e) => log::warn!("SlimProto accept error: {e}"),
        }
    }
    Ok(())
}

/// Read frames from a single connected SlimProto client until it disconnects.
/// `HELO` becomes a [`MessageType::SlimHelo`] for the UI to turn into a
/// renderer + button; anything else is read and discarded so the connection
/// stays open, matching real server behavior even though nothing acts on
/// those frames yet.
fn handle_connection(mut stream: TcpStream) {
    let peer = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "<unknown>".into());
    // Resolved once here instead of per-frame: stable for the life of the
    // connection. `remote_addr` is IP only, matching
    // `SlimRenderer::remote_addr`'s convention; `peer_port` is the
    // connection's source port, which a reconnect from the same IP gets
    // fresh — see `SlimRenderer::peer_port` for why that matters.
    let peer_addr: Option<SocketAddr> = stream.peer_addr().ok();
    let remote_addr: Option<EcoString> = peer_addr.map(|a| eco_format!("{}", a.ip()));
    let peer_port: u16 = peer_addr.map(|a| a.port()).unwrap_or_default();
    log::info!("SlimProto client connected from {peer}");
    // Coalesce a run of consecutively-ignored same-opcode frames (mostly
    // `STAT`, sent roughly every heartbeat interval) into a single debug
    // line instead of one per frame; logs again as soon as a different
    // opcode is seen.
    let mut last_ignored_opcode: Option<[u8; 4]> = None;
    let mut logged_first_stat = false;
    loop {
        match parse_frames::read_frame(&mut stream) {
            Ok(Frame::Helo(helo)) => {
                log::info!(
                    "SlimProto HELO from {peer}: device_id={} revision={} mac={:02x?} caps={}",
                    helo.device_id,
                    helo.revision,
                    helo.mac,
                    helo.capabilities
                );
                let Some(remote_addr) = remote_addr.clone() else {
                    log::warn!("SlimProto {peer}: dropping HELO, peer address unavailable");
                    continue;
                };
                let write_stream = match stream.try_clone() {
                    Ok(s) => Arc::new(Mutex::new(s)),
                    Err(e) => {
                        log::warn!(
                            "SlimProto {peer}: dropping HELO, failed to clone stream for writes: {e}"
                        );
                        continue;
                    }
                };
                // a reconnect from an already-known client: refresh its
                // write handle and peer_port in place instead of silently
                // dropping the HELO, so playback commands sent after a
                // reconnect land on the live connection instead of a dead one
                {
                    let mut renderers = get_slim_renderers_mut();
                    if let Some(existing) =
                        renderers.iter_mut().find(|r| r.remote_addr == remote_addr)
                    {
                        log::info!(
                            "SlimProto {peer}: renderer {remote_addr} reconnected, refreshing write handle"
                        );
                        existing.write_stream = write_stream;
                        existing.peer_port = peer_port;
                        spawn_heartbeat(existing.clone(), peer.clone());
                        continue;
                    }
                }
                let renderer = SlimRenderer::from_helo(&helo, remote_addr, peer_port, write_stream);
                spawn_heartbeat(renderer.clone(), peer.clone());
                if let Err(e) = get_msgchannel()
                    .0
                    .send(MessageType::SlimHelo(Box::new(renderer)))
                {
                    log::error!("SlimProto {peer}: failed to send SlimHelo message: {e}");
                }
            }
            Ok(Frame::Other { opcode, payload }) => {
                if &opcode == b"STAT" {
                    // STAT is sent very frequently and we don't act on its contents
                    // only the very first one per connection gets logged in full;
                    // every one after that collapses into a single summary
                    // line instead of one per frame like the other opcodes
                    if !logged_first_stat {
                        log_frame_detail(&peer, &opcode, &payload);
                        logged_first_stat = true;
                    } else if last_ignored_opcode != Some(opcode) {
                        log::debug!("SlimProto {peer}: ignoring \"STAT\" frames");
                        last_ignored_opcode = Some(opcode);
                    }
                } else {
                    log_frame_detail(&peer, &opcode, &payload);
                    last_ignored_opcode = None;
                }
            }
            Err(e) => {
                log::info!("SlimProto client {peer} disconnected: {e}");
                if let Some(remote_addr) = remote_addr
                    && let Err(e) = get_msgchannel().0.send(MessageType::SlimDisconnected {
                        remote_addr,
                        peer_port,
                    })
                {
                    log::error!("SlimProto {peer}: failed to send SlimDisconnected message: {e}");
                }
                return;
            }
        }
    }
}

/// Debug-log an unhandled frame's opcode and full payload, hex-dumped.
fn log_frame_detail(peer: &str, opcode: &[u8; 4], payload: &[u8]) {
    log::debug!(
        "SlimProto {peer}: {:?} frame ({} bytes): {payload:02x?}",
        String::from_utf8_lossy(opcode),
        payload.len(),
    );
}

/// Periodically send a `strm 't'` heartbeat on `renderer`'s control
/// connection, so squeezelite's own idle-read timeout never fires on an
/// otherwise-quiet connection — see [`HEARTBEAT_INTERVAL`]. Self-terminates
/// on the first write failure (the connection died; the read-loop thread on
/// `handle_connection` handles the actual cleanup/reconnect bookkeeping),
/// so a reconnect just leaves this to notice on its next tick and exit,
/// while a fresh heartbeat thread is spawned for the new connection.
fn spawn_heartbeat(renderer: SlimRenderer, peer: String) {
    let jh = thread::Builder::new()
        .name("slim_heartbeat".into())
        .stack_size(THREAD_STACK)
        .spawn({
            let peer = peer.clone();
            move || {
                loop {
                    thread::sleep(HEARTBEAT_INTERVAL);
                    if let Err(e) = renderer.send_strm_status() {
                        log::debug!("SlimProto {peer}: heartbeat stopped: {e}");
                        return;
                    }
                }
            }
        });
    if let Err(e) = jh {
        log::warn!("SlimProto {peer}: failed to spawn heartbeat thread: {e}");
    }
}
