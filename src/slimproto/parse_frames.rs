//! Parsing for SlimProto client -> server frames: `HELO` is decoded into
//! [`SlimHelo`]; anything else comes back as [`Frame::Other`] with its
//! payload intact for the caller to act on or log.

use std::io::{self, Read};

/// Fixed portion of a parsed `HELO` frame — the SlimProto client handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlimHelo {
    pub device_id: u8,
    pub revision: u8,
    pub mac: [u8; 6],
    pub uuid: Option<[u8; 16]>,
    pub wlan_channel_list: u16,
    pub bytes_received: u64,
    pub capabilities: String,
    pub lang: Option<[u8; 2]>,
}

/// A parsed SlimProto frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Helo(SlimHelo),
    /// Any frame whose opcode we don't act on yet (`STAT`, `DSCO`, ...),
    /// with its payload intact so the caller can debug-log its content.
    Other {
        opcode: [u8; 4],
        payload: Vec<u8>,
    },
}

/// Byte length of the fixed `HELO_packet` fields after the generic 8-byte
/// opcode+length header: deviceid(1) + revision(1) + mac(6) + uuid(16) +
/// wlan_channellist(2) + bytes_received_H(4) + bytes_received_L(4) + lang(2).
const HELO_FIXED_LEN: usize = 36;

/// Upper bound on a single frame's declared payload length. Real `HELO`/
/// `STAT`/etc. payloads are at most a few hundred bytes; this just stops a
/// garbled or malicious `length` field from making us allocate an
/// attacker-chosen amount of memory before validating anything else.
const MAX_FRAME_PAYLOAD_LEN: u32 = 8 * 1024;

/// Read one SlimProto frame from `stream`: an 8-byte `opcode`+`length`
/// header (length is big-endian, payload-only), followed by `length` bytes
/// of payload. `HELO` frames are parsed into [`SlimHelo`]; anything else is
/// returned as [`Frame::Other`] with its payload drained from the stream
/// (so the caller can keep reading subsequent frames) and handed back
/// intact, so the caller can debug-log it.
pub fn read_frame(stream: &mut impl Read) -> io::Result<Frame> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let opcode: [u8; 4] = header[0..4].try_into().unwrap();
    let length = u32::from_be_bytes(header[4..8].try_into().unwrap());
    if length > MAX_FRAME_PAYLOAD_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "SlimProto frame payload too large: {length} bytes, max {MAX_FRAME_PAYLOAD_LEN}"
            ),
        ));
    }

    let mut payload = vec![0u8; length as usize];
    stream.read_exact(&mut payload)?;

    if &opcode == b"HELO" {
        Ok(Frame::Helo(parse_helo(&payload)?))
    } else {
        Ok(Frame::Other { opcode, payload })
    }
}

/// `uuid` and `lang` from the wire format are intentionally not parsed: real
/// squeezelite always zero-fills `uuid` and never sets `lang` in `sendHELO()`
/// (see `slimproto.c`), so there is nothing meaningful to store yet.
fn parse_helo(payload: &[u8]) -> io::Result<SlimHelo> {
    if payload.len() < HELO_FIXED_LEN {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "HELO payload too short: {} bytes, expected at least {HELO_FIXED_LEN}",
                payload.len()
            ),
        ));
    }
    let device_id = payload[0];
    let revision = payload[1];
    let mac: [u8; 6] = payload[2..8].try_into().unwrap();
    // payload[8..24] is uuid, intentionally skipped
    let wlan_channel_list = u16::from_be_bytes(payload[24..26].try_into().unwrap());
    let bytes_received_h = u32::from_be_bytes(payload[26..30].try_into().unwrap());
    let bytes_received_l = u32::from_be_bytes(payload[30..34].try_into().unwrap());
    let bytes_received = (u64::from(bytes_received_h) << 32) | u64::from(bytes_received_l);
    // payload[34..36] is lang, intentionally skipped
    let capabilities = String::from_utf8_lossy(&payload[HELO_FIXED_LEN..]).into_owned();

    Ok(SlimHelo {
        device_id,
        revision,
        mac,
        uuid: None,
        wlan_channel_list,
        bytes_received,
        capabilities,
        lang: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn build_helo_bytes(mac: [u8; 6], capabilities: &str) -> Vec<u8> {
        let mut payload = vec![0u8; HELO_FIXED_LEN];
        payload[0] = 12; // deviceid: squeezeplay
        payload[1] = 0; // revision
        payload[2..8].copy_from_slice(&mac);
        // uuid (8..24) left zeroed
        payload[24..26].copy_from_slice(&0x4000u16.to_be_bytes());
        payload[26..30].copy_from_slice(&0u32.to_be_bytes()); // bytes_received_H
        payload[30..34].copy_from_slice(&12345u32.to_be_bytes()); // bytes_received_L
        // lang (34..36) left zeroed
        payload.extend_from_slice(capabilities.as_bytes());

        let mut frame = Vec::new();
        frame.extend_from_slice(b"HELO");
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    #[test]
    fn parses_helo_frame() {
        let mac = [0x00, 0x04, 0x20, 0x12, 0x34, 0x56];
        let bytes = build_helo_bytes(mac, "Model=squeezelite,AccuratePlayPoints=1,Firmware=v1.0");
        let mut cursor = Cursor::new(bytes);

        let frame = read_frame(&mut cursor).expect("failed to read HELO frame");
        let Frame::Helo(helo) = frame else {
            panic!("expected Frame::Helo, got {frame:?}");
        };
        assert_eq!(helo.device_id, 12);
        assert_eq!(helo.revision, 0);
        assert_eq!(helo.mac, mac);
        assert_eq!(helo.wlan_channel_list, 0x4000);
        assert_eq!(helo.bytes_received, 12345);
        assert_eq!(
            helo.capabilities,
            "Model=squeezelite,AccuratePlayPoints=1,Firmware=v1.0"
        );
    }

    #[test]
    fn skips_unknown_frame_and_reads_next() {
        let mut bytes = Vec::new();
        // an unrelated frame (e.g. STAT) with a 5-byte payload we don't understand yet
        bytes.extend_from_slice(b"STAT");
        bytes.extend_from_slice(&5u32.to_be_bytes());
        bytes.extend_from_slice(b"junk!");
        // followed by a real HELO frame
        bytes.extend_from_slice(&build_helo_bytes([1, 2, 3, 4, 5, 6], "Model=squeezelite"));
        let mut cursor = Cursor::new(bytes);

        let first = read_frame(&mut cursor).expect("failed to read first frame");
        assert_eq!(
            first,
            Frame::Other {
                opcode: *b"STAT",
                payload: b"junk!".to_vec()
            }
        );

        let second = read_frame(&mut cursor).expect("failed to read second frame");
        let Frame::Helo(helo) = second else {
            panic!("expected Frame::Helo, got {second:?}");
        };
        assert_eq!(helo.mac, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rejects_truncated_helo_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"HELO");
        bytes.extend_from_slice(&10u32.to_be_bytes());
        bytes.extend_from_slice(&[0u8; 10]);
        let mut cursor = Cursor::new(bytes);

        let err = read_frame(&mut cursor).expect_err("expected a parse error");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn rejects_oversized_frame_without_allocating_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"HELO");
        bytes.extend_from_slice(&(MAX_FRAME_PAYLOAD_LEN + 1).to_be_bytes());
        // deliberately no payload bytes follow: if `read_frame` tried to
        // read the (oversized) payload it would fail with UnexpectedEof
        // instead, so getting InvalidData here proves it was rejected
        // before attempting the read/allocation.
        let mut cursor = Cursor::new(bytes);

        let err = read_frame(&mut cursor).expect_err("expected the oversized frame to be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
