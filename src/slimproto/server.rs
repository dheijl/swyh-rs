use crate::enums::messages::MessageType;
use crate::globals::statics::{THREAD_STACK, get_msgchannel, get_slim_renderers_mut};
use crate::slimproto::frames::{self, Frame};
use crate::slimproto::types::SlimRenderer;
use anyhow::{Context, Result};
use ecow::{EcoString, eco_format};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

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
    loop {
        match frames::read_frame(&mut stream) {
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
                        continue;
                    }
                }
                let renderer = SlimRenderer::from_helo(&helo, remote_addr, peer_port, write_stream);
                if let Err(e) = get_msgchannel()
                    .0
                    .send(MessageType::SlimHelo(Box::new(renderer)))
                {
                    log::error!("SlimProto {peer}: failed to send SlimHelo message: {e}");
                }
            }
            Ok(Frame::Other { opcode, length }) => {
                log::debug!(
                    "SlimProto {peer}: ignoring {:?} frame ({length} bytes)",
                    String::from_utf8_lossy(&opcode)
                );
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
