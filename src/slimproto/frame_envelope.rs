//! Shared SlimProto server -> client frame envelope, used by every
//! server -> client opcode (`strm`, `audg`, ...): `length(BE u16, covers
//! opcode+payload) + opcode[4] + payload`.

/// Wrap a payload in its SlimProto server -> client frame envelope.
pub(crate) fn build_frame_envelope(opcode: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let body_len = 4 + payload.len();
    let mut frame = Vec::with_capacity(2 + body_len);
    frame.extend_from_slice(&(body_len as u16).to_be_bytes());
    frame.extend_from_slice(opcode);
    frame.extend_from_slice(payload);
    frame
}
