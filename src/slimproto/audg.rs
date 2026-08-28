//! Builder for the SlimProto `audg` (audio gain / volume) command frame
//! (server -> client).
//!
//! Byte layout verified against squeezelite's `slimproto.h` (`struct
//! audg_packet`) and `process_audg` in `slimproto.c`
//!
//! ```c
//! struct audg_packet {
//!     char  opcode[4];
//!     u32_t old_gainL;  // unused
//!     u32_t old_gainR;  // unused
//!     u8_t  adjust;     // non-zero: apply gainL/gainR; zero: client forces unity gain
//!     u8_t  preamp;     // unused
//!     u32_t gainL;
//!     u32_t gainR;
//! };
//! ```
//! 18-byte payload after the 4-byte opcode (which, like `strm`, is handled
//! by the shared frame envelope — see
//! [`crate::slimproto::frame_envelope::build_frame_envelope`] — not this
//! struct), no padding (same `#pragma pack(push, 1)` as `strm_packet`).
//!
//! `gainL`/`gainR` are 16.16 fixed-point (`FIXED_ONE = 0x10000` in
//! `squeezelite.h` is unity/100% gain): `process_audg` computes
//! `set_volume(adjust ? gainL : FIXED_ONE, adjust ? gainR : FIXED_ONE)`, so
//! `adjust` must be non-zero or the client ignores `gainL`/`gainR`
//! entirely. No separate L/R balance is exposed here (matches swyh-rs's
//! single-value UPnP volume slider) — both channels always get the same
//! gain, a plain linear percent-to-gain mapping rather than LMS's own
//! perceptual volume curve.

use crate::slimproto::frame_envelope::build_frame_envelope;

const FIXED_ONE: f64 = 65536.0;

/// Build an `audg` frame setting both channels' gain to `volume_percent`
/// (clamped to `0..=100`).
pub fn build_audg(volume_percent: i32) -> Vec<u8> {
    let pct = volume_percent.clamp(0, 100);
    let gain = ((pct as f64 / 100.0) * FIXED_ONE).round() as u32;

    let mut payload = Vec::with_capacity(18);
    payload.extend_from_slice(&0u32.to_be_bytes()); // old_gainL: unused
    payload.extend_from_slice(&0u32.to_be_bytes()); // old_gainR: unused
    payload.push(1); // adjust: apply gainL/gainR (0 would force unity gain client-side)
    payload.push(0); // preamp: unused
    payload.extend_from_slice(&gain.to_be_bytes()); // gainL
    payload.extend_from_slice(&gain.to_be_bytes()); // gainR
    debug_assert_eq!(payload.len(), 18);

    build_frame_envelope(b"audg", &payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(frame: &[u8]) -> &[u8] {
        &frame[6..]
    }

    #[test]
    fn audg_frame_envelope() {
        let frame = build_audg(50);
        let declared_len = u16::from_be_bytes(frame[0..2].try_into().unwrap());
        assert_eq!(declared_len, 4 + 18); // opcode + payload
        assert_eq!(declared_len as usize, frame.len() - 2);
        assert_eq!(&frame[2..6], b"audg");
        assert_eq!(payload(&frame).len(), 18);
    }

    #[test]
    fn audg_frame_layout_and_fields() {
        let frame = build_audg(50);
        let p = payload(&frame);

        assert_eq!(&p[0..4], &0u32.to_be_bytes()); // old_gainL
        assert_eq!(&p[4..8], &0u32.to_be_bytes()); // old_gainR
        assert_eq!(p[8], 1); // adjust
        assert_eq!(p[9], 0); // preamp
        assert_eq!(&p[10..14], &32768u32.to_be_bytes()); // gainL: 50% -> 32768
        assert_eq!(&p[14..18], &32768u32.to_be_bytes()); // gainR
    }

    #[test]
    fn audg_gain_at_zero_percent() {
        let frame = build_audg(0);
        let p = payload(&frame);
        assert_eq!(&p[10..14], &0u32.to_be_bytes());
        assert_eq!(&p[14..18], &0u32.to_be_bytes());
    }

    #[test]
    fn audg_gain_at_hundred_percent() {
        let frame = build_audg(100);
        let p = payload(&frame);
        assert_eq!(&p[10..14], &65536u32.to_be_bytes()); // FIXED_ONE: unity gain
        assert_eq!(&p[14..18], &65536u32.to_be_bytes());
    }

    #[test]
    fn audg_clamps_out_of_range_input() {
        let over = build_audg(150);
        let under = build_audg(-20);
        assert_eq!(payload(&over)[10..14], 65536u32.to_be_bytes());
        assert_eq!(payload(&under)[10..14], 0u32.to_be_bytes());
    }
}
