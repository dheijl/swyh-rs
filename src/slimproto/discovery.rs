//! SlimProto UDP discovery: replies to squeezelite's broadcast probe so it
//! can find this server's control port.

use anyhow::{Context, Result};
use std::net::UdpSocket;

/// Bind the SlimProto discovery UDP socket on the given port.
///
/// Split out from [`serve`] so tests can bind on port `0` and discover the
/// OS-assigned port via [`UdpSocket::local_addr`].
pub fn bind(port: u16) -> Result<UdpSocket> {
    UdpSocket::bind(("0.0.0.0", port))
        .with_context(|| format!("failed to bind SlimProto discovery socket on port {port}"))
}

/// Discovery request byte squeezelite's `discover_server()` broadcasts.
const DISCOVERY_REQUEST: u8 = b'e';

/// Serve discovery requests on `socket` forever.
///
/// squeezelite's `discover_server()` broadcasts a single byte `'e'` on the
/// SlimProto port and only inspects the *source address* of whatever reply
/// arrives — it never parses the reply payload — so any short reply is
/// sufficient to let it find this server. Datagrams that don't start with
/// `'e'` are ignored rather than answered, so this doesn't reply to
/// unrelated or spoofed traffic on the port.
pub fn serve(socket: UdpSocket) -> ! {
    let mut buf = [0u8; 32];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                if n == 0 || buf[0] != DISCOVERY_REQUEST {
                    log::debug!("ignoring non-discovery datagram from {src}");
                    continue;
                }
                log::debug!("SlimProto discovery request from {src}");
                if let Err(e) = socket.send_to(b"E", src) {
                    log::warn!("failed to send SlimProto discovery reply to {src}: {e}");
                }
            }
            Err(e) => log::warn!("SlimProto discovery recv error: {e}"),
        }
    }
}

/// Bind and serve SlimProto discovery on `port`.
pub fn run_discovery(port: u16) -> Result<()> {
    let socket = bind(port)?;
    serve(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn replies_to_discovery_probe() {
        let server_socket = bind(0).expect("failed to bind test discovery socket");
        let server_addr = server_socket.local_addr().unwrap();
        std::thread::spawn(move || serve(server_socket));

        let client = UdpSocket::bind("0.0.0.0:0").expect("failed to bind test client socket");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.send_to(b"e", server_addr).unwrap();

        let mut buf = [0u8; 32];
        let (n, from) = client
            .recv_from(&mut buf)
            .expect("did not receive a discovery reply in time");
        // the server was bound to the wildcard address 0.0.0.0, so the reply
        // is routed from the concrete loopback address rather than 0.0.0.0
        assert_eq!(from.port(), server_addr.port());
        assert!(from.ip().is_loopback());
        assert!(n > 0);
    }

    #[test]
    fn ignores_non_discovery_datagram() {
        let server_socket = bind(0).expect("failed to bind test discovery socket");
        let server_addr = server_socket.local_addr().unwrap();
        std::thread::spawn(move || serve(server_socket));

        let client = UdpSocket::bind("0.0.0.0:0").expect("failed to bind test client socket");
        client
            .set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        client
            .send_to(b"not a discovery probe", server_addr)
            .unwrap();

        let mut buf = [0u8; 32];
        let result = client.recv_from(&mut buf);
        assert!(
            result.is_err(),
            "expected no reply to a non-discovery datagram, got {result:?}"
        );
    }
}
