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
//!
//! **Lpcm/Wav/Rf64 are always sent headerless.** squeezelite's PCM decoder
//! (`pcm.c`) only sniffs a WAV/AIFF header when reading a local file or a
//! `pcm_check_header` runtime flag is set, neither of which applies to our
//! live HTTP stream by default — sending a WAV/RF64 header inline would
//! corrupt playback (the header isn't a multiple of the sample frame size,
//! so misalignment persists for the rest of the stream). So all three map
//! to squeezelite's raw-PCM decoder (`format = 'p'`) with the *real* sample
//! rate/depth/channels/endianness declared in the `strm` packet itself —
//! sufficient information for the decoder — and `server::streaming_server`
//! is told (via the `?slim=1` query marker every `strm`-triggered request
//! carries) to drop the WAV/RF64 container header for this connection
//! specifically. UPnP renderers are unaffected: they never send that
//! marker, so they still get a real WAV/RF64 file.

use crate::enums::streaming::{BitDepth, StreamingFormat};
use crate::rendercontrol::StreamInfo;
use std::net::Ipv4Addr;

/// squeezelite's `sample_rates[]` table (`pcm.c`'s `pcm_open`): the strm
/// packet's 1-byte `pcm_sample_rate` field is `'0' + index` into this
/// table. Only the entries swyh-rs can actually produce (see
/// `SAMPLE_RATES` in `globals/statics.rs`) are listed here; squeezelite's
/// own table has further entries (up to 1536000) that swyh-rs never needs.
const PCM_SAMPLE_RATES: [u32; 15] = [
    11025, 22050, 32000, 44100, 48000, 8000, 12000, 16000, 24000, 96000, 88200, 176400, 192000,
    352800, 384000,
];

/// Encode `rate` as the strm packet's `pcm_sample_rate` byte, or `None` if
/// it has no code in squeezelite's table (shouldn't happen for any rate in
/// `SAMPLE_RATES`, but better to fail loudly than send an undefined code).
fn pcm_rate_code(rate: u32) -> Option<u8> {
    PCM_SAMPLE_RATES
        .iter()
        .position(|&r| r == rate)
        .map(|i| b'0' + i as u8)
}

/// Encode `bits_per_sample` as the strm packet's `pcm_sample_size` byte:
/// squeezelite computes `bytes_per_sample = byte - '0' + 1`.
fn pcm_size_code(bits_per_sample: BitDepth) -> u8 {
    match bits_per_sample {
        BitDepth::Bits16 => b'1', // 2 bytes/sample
        BitDepth::Bits24 => b'2', // 3 bytes/sample
    }
}

/// Build a `strm` "start" frame telling the client to open an HTTP
/// connection to `server_ip:streaminfo.server_port` and decode
/// `streaminfo.streaming_format`.
///
/// Flac is self-describing (`pcm_*` fields set to `'?'`, format `'f'`).
/// Lpcm/Wav/Rf64 all map to `'p'` — see this module's doc comment for the
/// headerless-PCM rationale. Returns `Err` instead of silently sending an
/// undefined/wrong `pcm_sample_rate` if `streaminfo.sample_rate` has no
/// code in squeezelite's table.
pub fn build_strm_start(server_ip: Ipv4Addr, streaminfo: &StreamInfo) -> Result<Vec<u8>, String> {
    let fmt = streaminfo.streaming_format;
    // `?slim=1` marks every strm-triggered request, not just Wav/Rf64: it
    // also tells `get_dlna_headers` (streaming_server.rs) to skip the
    // UPnP-only `TransferMode.dlna.org` header for *any* SlimProto format.
    // Forcing the WAV/RF64 header-drain for Flac/Lpcm too is a harmless
    // no-op there, since `wav_header_size()` is already 0 for both.
    let request_line = format!("GET /stream/swyh.{}?slim=1 HTTP/1.0\r\n\r\n", fmt.as_str());

    let (format, pcm_sample_size, pcm_sample_rate, pcm_channels, pcm_endianness) =
        if fmt == StreamingFormat::Flac {
            (b'f', b'?', b'?', b'?', b'?')
        } else {
            let rate = pcm_rate_code(streaminfo.sample_rate).ok_or_else(|| {
                format!(
                    "SlimProto: {} Hz has no strm pcm_sample_rate code",
                    streaminfo.sample_rate
                )
            })?;
            (
                b'p',
                pcm_size_code(streaminfo.bits_per_sample),
                rate,
                b'2',                                          // always stereo
                if fmt.needs_wav_hdr() { b'1' } else { b'0' }, // Wav/Rf64: little-endian, Lpcm: big-endian
            )
        };

    let mut payload = Vec::with_capacity(24 + request_line.len());
    payload.push(b's'); // command
    payload.push(b'1'); // autostart
    payload.push(format);
    payload.push(pcm_sample_size);
    payload.push(pcm_sample_rate);
    payload.push(pcm_channels);
    payload.push(pcm_endianness);
    payload.push(0); // threshold: 0 KB, fine to autostart immediately
    payload.push(0); // spdif_enable
    payload.push(0); // transition_period
    payload.push(b'0'); // transition_type: none
    payload.push(0); // flags
    payload.push(0); // output_threshold
    payload.push(0); // slaves
    payload.extend_from_slice(&0u32.to_be_bytes()); // replay_gain
    payload.extend_from_slice(&streaminfo.server_port.to_be_bytes());
    payload.extend_from_slice(&server_ip.octets());
    payload.extend_from_slice(request_line.as_bytes());
    debug_assert_eq!(payload.len(), 24 + request_line.len());

    Ok(build_frame_envelope(b"strm", &payload))
}

/// Build a `strm` "stop" frame telling the client to halt playback
/// immediately. All fields besides `command` are ignored by the client for
/// `'q'`, so they're left zeroed and no request line is sent.
pub fn build_strm_stop() -> Vec<u8> {
    build_control_frame(b'q')
}

/// Build a `strm` "status" (time) request frame — a no-op as far as
/// playback goes, used purely as a heartbeat: squeezelite's control
/// connection has its own idle-read timeout (observed ~35-40s in the wild)
/// and treats hitting it as the connection dying, tearing down any
/// in-progress stream along with it. A real SlimProto server avoids this by
/// periodically sending *something*; this is that something. Same
/// all-other-fields-zeroed shape as `build_strm_stop`.
pub fn build_strm_status() -> Vec<u8> {
    build_control_frame(b't')
}

/// Build a minimal strm frame carrying just `command`, every other field
/// zeroed and no request line — used for `'q'` and `'t'`, where the client
/// ignores everything but `command`.
fn build_control_frame(command: u8) -> Vec<u8> {
    let mut payload = vec![0u8; 24];
    payload[0] = command;
    build_frame_envelope(b"strm", &payload)
}

/// Wrap a payload in its SlimProto server -> client frame envelope:
/// `length(BE u16, covers opcode+payload) + opcode[4] + payload`. Shared
/// with `audg::build_audg` — this envelope shape applies to every
/// server -> client opcode, not just `strm`.
pub(crate) fn build_frame_envelope(opcode: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let body_len = 4 + payload.len();
    let mut frame = Vec::with_capacity(2 + body_len);
    frame.extend_from_slice(&(body_len as u16).to_be_bytes());
    frame.extend_from_slice(opcode);
    frame.extend_from_slice(payload);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::globals::statics::SAMPLE_RATES;

    fn streaminfo(
        sample_rate: u32,
        bits_per_sample: BitDepth,
        streaming_format: StreamingFormat,
    ) -> StreamInfo {
        StreamInfo {
            sample_rate,
            bits_per_sample,
            streaming_format,
            server_port: 5901,
        }
    }

    #[test]
    fn strm_start_flac_frame_layout() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let info = streaminfo(44100, BitDepth::Bits24, StreamingFormat::Flac);
        let frame = build_strm_start(ip, &info).expect("flac never fails to encode");

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
        assert_eq!(
            request_line,
            "GET /stream/swyh.flac?slim=1 HTTP/1.0\r\n\r\n"
        );
    }

    #[test]
    fn strm_start_lpcm_frame_layout() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let info = streaminfo(48000, BitDepth::Bits16, StreamingFormat::Lpcm);
        let frame = build_strm_start(ip, &info).expect("48000 Hz is a known rate");
        let payload = &frame[6..];

        assert_eq!(payload[2], b'p'); // format
        assert_eq!(payload[3], b'1'); // pcm_sample_size: 16-bit -> 2 bytes
        assert_eq!(payload[4], b'4'); // pcm_sample_rate: index 4 -> 48000
        assert_eq!(payload[5], b'2'); // pcm_channels: stereo
        assert_eq!(payload[6], b'0'); // pcm_endianness: big-endian (DLNA L16/L24 convention)

        let request_line = std::str::from_utf8(&payload[24..]).unwrap();
        assert_eq!(
            request_line,
            "GET /stream/swyh.lpcm?slim=1 HTTP/1.0\r\n\r\n"
        );
    }

    #[test]
    fn strm_start_wav_frame_layout() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let info = streaminfo(88200, BitDepth::Bits24, StreamingFormat::Wav);
        let frame = build_strm_start(ip, &info).expect("88200 Hz is a known rate");
        let payload = &frame[6..];

        assert_eq!(payload[2], b'p'); // format
        assert_eq!(payload[3], b'2'); // pcm_sample_size: 24-bit -> 3 bytes
        assert_eq!(payload[4], b':'); // pcm_sample_rate: index 10 -> 88200
        assert_eq!(payload[5], b'2'); // pcm_channels: stereo
        assert_eq!(payload[6], b'1'); // pcm_endianness: little-endian (WAV convention)

        let request_line = std::str::from_utf8(&payload[24..]).unwrap();
        assert_eq!(request_line, "GET /stream/swyh.wav?slim=1 HTTP/1.0\r\n\r\n");
    }

    #[test]
    fn pcm_rate_code_covers_every_supported_sample_rate() {
        for &rate in SAMPLE_RATES {
            assert!(
                pcm_rate_code(rate).is_some(),
                "no strm pcm_sample_rate code for {rate} Hz"
            );
        }
    }

    #[test]
    fn pcm_rate_code_rejects_unknown_rate() {
        assert_eq!(pcm_rate_code(22000), None);
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

    #[test]
    fn strm_status_frame_layout() {
        let frame = build_strm_status();

        let declared_len = u16::from_be_bytes(frame[0..2].try_into().unwrap());
        assert_eq!(declared_len, 4 + 24);
        assert_eq!(&frame[2..6], b"strm");

        let payload = &frame[6..];
        assert_eq!(payload.len(), 24);
        assert_eq!(payload[0], b't'); // command
        assert!(payload[1..].iter().all(|&b| b == 0));
    }
}
