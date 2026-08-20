//! Builders for the SlimProto `strm` command frame (server -> client), used
//! to start or stop playback on a connected squeezelite client.
//!
//! Byte layout verified against squeezelite's `slimproto.h` (`struct
//! strm_packet`) and `slimserver`'s `Slim::Player::Squeezebox::stream_s`,
//! not from memory: a 24-byte fixed struct (no padding) immediately
//! followed, in the same frame payload, by a raw HTTP request line the
//! client sends verbatim on the new connection it opens back to us.
//!
//! **The outer envelope is not symmetric with client -> server frames.**
//! `frames::read_frame` parses client -> server frames (`HELO`, `STAT`, ...)
//! as `opcode[4] + length(BE u32, payload-only) + payload`. Server -> client
//! frames use a *different* shape (verified against squeezelite's
//! `slimproto_run()` read loop in `slimproto.c`): a **2-byte** BE length
//! covering `opcode[4] + payload` together, i.e.
//! `length(BE u16) + opcode[4] + payload`. Getting this wrong doesn't
//! produce a clean parse error on the client — squeezelite reads garbage,
//! desyncs, and resets the TCP connection instead.

use std::net::Ipv4Addr;

/// `/stream/swyh.flac` is the same path `Controller::play` already builds
/// for UPnP renderers (`src/rendercontrol/control.rs`), so
/// `streaming_server.rs` needs no changes to serve squeezelite too.
const STREAM_PATH: &str = "/stream/swyh.flac";

/// Build a `strm` "start" frame telling the client to open an HTTP
/// connection to `server_ip:server_port` and decode FLAC.
///
/// FLAC-only: the `pcm_*` fields are meaningless for a non-PCM format and
/// are set to `'?'`, matching `slimserver`'s own behavior for `formatbyte =
/// 'f'`.
pub fn build_strm_start(server_ip: Ipv4Addr, server_port: u16) -> Vec<u8> {
    let request_line = format!("GET {STREAM_PATH} HTTP/1.0\r\n\r\n");
    build_strm_frame(b's', &request_line, server_ip, server_port)
}

/// Build a `strm` "stop" frame telling the client to halt playback
/// immediately. All fields besides `command` are ignored by the client for
/// `'q'`, so they're left zeroed and no request line is sent.
pub fn build_strm_stop() -> Vec<u8> {
    build_strm_frame(b'q', "", Ipv4Addr::UNSPECIFIED, 0)
}

fn build_strm_frame(
    command: u8,
    request_line: &str,
    server_ip: Ipv4Addr,
    server_port: u16,
) -> Vec<u8> {
    let starting = command == b's';
    let pcm_field = if starting { b'?' } else { 0 }; // '?': unused for flac
    let mut payload = Vec::with_capacity(24 + request_line.len());
    payload.push(command);
    payload.push(if starting { b'1' } else { 0 }); // autostart
    payload.push(if starting { b'f' } else { 0 }); // format: flac
    payload.push(pcm_field); // pcm_sample_size
    payload.push(pcm_field); // pcm_sample_rate
    payload.push(pcm_field); // pcm_channels
    payload.push(pcm_field); // pcm_endianness
    payload.push(0); // threshold: 0 KB, fine to autostart immediately
    payload.push(0); // spdif_enable
    payload.push(0); // transition_period
    payload.push(if starting { b'0' } else { 0 }); // transition_type: none
    payload.push(0); // flags
    payload.push(0); // output_threshold
    payload.push(0); // slaves
    payload.extend_from_slice(&0u32.to_be_bytes()); // replay_gain
    payload.extend_from_slice(&server_port.to_be_bytes());
    payload.extend_from_slice(&server_ip.octets());
    payload.extend_from_slice(request_line.as_bytes());
    debug_assert_eq!(payload.len(), 24 + request_line.len());

    // envelope: length(BE u16, covers opcode+payload) + opcode[4] + payload
    let body_len = 4 + payload.len();
    let mut frame = Vec::with_capacity(2 + body_len);
    frame.extend_from_slice(&(body_len as u16).to_be_bytes());
    frame.extend_from_slice(b"strm");
    frame.extend_from_slice(&payload);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strm_start_frame_layout() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let frame = build_strm_start(ip, 5901);

        let declared_len = u16::from_be_bytes(frame[0..2].try_into().unwrap());
        assert_eq!(declared_len as usize, frame.len() - 2); // covers opcode+payload
        assert_eq!(&frame[2..6], b"strm");
        let payload = &frame[6..];

        assert_eq!(payload[0], b's'); // command
        assert_eq!(payload[1], b'1'); // autostart
        assert_eq!(payload[2], b'f'); // format
        assert_eq!(&payload[3..7], b"????"); // pcm_* fields
        assert_eq!(payload[7], 0); // threshold
        assert_eq!(payload[8], 0); // spdif_enable
        assert_eq!(payload[9], 0); // transition_period
        assert_eq!(payload[10], b'0'); // transition_type
        assert_eq!(payload[11], 0); // flags
        assert_eq!(payload[12], 0); // output_threshold
        assert_eq!(payload[13], 0); // slaves
        assert_eq!(&payload[14..18], &0u32.to_be_bytes()); // replay_gain
        assert_eq!(&payload[18..20], &5901u16.to_be_bytes()); // server_port
        assert_eq!(&payload[20..24], &ip.octets()); // server_ip

        let request_line = std::str::from_utf8(&payload[24..]).unwrap();
        assert_eq!(request_line, "GET /stream/swyh.flac HTTP/1.0\r\n\r\n");
    }

    #[test]
    fn strm_stop_frame_layout() {
        let frame = build_strm_stop();

        let declared_len = u16::from_be_bytes(frame[0..2].try_into().unwrap());
        assert_eq!(declared_len, 4 + 24); // opcode + fixed struct, no request line
        assert_eq!(&frame[2..6], b"strm");

        let payload = &frame[6..];
        assert_eq!(payload.len(), 24);
        assert_eq!(payload[0], b'q'); // command
        // everything else is zeroed for a stop command
        assert!(payload[1..].iter().all(|&b| b == 0));
    }
}
