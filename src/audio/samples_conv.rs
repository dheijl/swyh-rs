//! SIMD-accelerated sample format conversion utilities.
//!
//! Converts captured f32 audio samples to integer PCM formats (i16/i24, little/big endian)
//! using SSE2 f32x4 SIMD registers via the `wide` crate.

use wide::f32x4;

use crate::enums::streaming::BitDepth;

/// conversion constant for f32 sample to i32
const F32_TO_I32: f32 = (i32::MAX as f32) + 1.0;
/// XMM register constant
static F32_TO_I32_SIMD: f32x4 = f32x4::splat(F32_TO_I32);

/// convert f32 samples to i32 for flac encoding
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate)
pub(crate) fn samples_to_i32(f32_samples: &[f32], i32_samples: &mut Vec<i32>, bd: BitDepth) {
    debug_assert!(
        f32_samples.len() & 1 == 0,
        "Number of FLAC samples should always be even!"
    );
    i32_samples.clear();
    let chunks = f32_samples.chunks_exact(4);
    let remainder = chunks.remainder();
    chunks.for_each(|chunk| {
        // chunks are guaranteed to be 4 elements
        let f32_array = f32x4::new(*chunk.as_array().unwrap());
        let i_array = f32_to_i32(bd, f32_array);
        i32_samples.extend_from_slice(&i_array);
    });
    if remainder.len() == 2 {
        let f32_array = f32x4::new([remainder[0], remainder[1], 0.0, 0.0]);
        let i_array = f32_to_i32(bd, f32_array);
        i32_samples.extend_from_slice(&i_array[0..2]);
    }
}

/// convert 4 contiguous f32 samples to an i32 array (scaled to bitdepth)
#[inline(always)]
pub(crate) fn f32_chunk_to_i32(bd: BitDepth, samples: &[f32; 4]) -> [i32; 4] {
    f32_to_i32(bd, f32x4::new(*samples))
}

/// convert 4 f32 samples in an f32x4 to 4 i32 samples (using SIMD)
#[inline(always)]
pub(crate) fn f32_to_i32(bd: BitDepth, f32_simd: f32x4) -> [i32; 4] {
    let fchunk_i32 = f32_simd * F32_TO_I32_SIMD;
    let s4i = fchunk_i32.trunc_int() >> bd.shift_value();
    s4i.to_array()
}

#[inline(always)]
pub(crate) fn i32_to_i16le(i32_array: &[i32; 4], buf: &mut [u8]) {
    // assert to remove bounds checks
    assert!(buf.len() == 8);
    buf[0..=1].copy_from_slice(&(i32_array[0] as i16).to_le_bytes());
    buf[2..=3].copy_from_slice(&(i32_array[1] as i16).to_le_bytes());
    buf[4..=5].copy_from_slice(&(i32_array[2] as i16).to_le_bytes());
    buf[6..=7].copy_from_slice(&(i32_array[3] as i16).to_le_bytes());
}

#[inline(always)]
pub(crate) fn i32_to_i24le(i32_array: &[i32; 4], buf: &mut [u8]) {
    // assert to remove bounds checks
    assert!(buf.len() == 12);
    buf[0..=2].copy_from_slice(&i32_array[0].to_le_bytes()[..=2]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_le_bytes()[..=2]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_le_bytes()[..=2]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_le_bytes()[..=2]);
}

#[inline(always)]
pub(crate) fn i32_to_i16be(i32_array: &[i32; 4], buf: &mut [u8]) {
    // assert to remove bounds checks
    assert!(buf.len() == 8);
    buf[0..=1].copy_from_slice(&(i32_array[0] as i16).to_be_bytes());
    buf[2..=3].copy_from_slice(&(i32_array[1] as i16).to_be_bytes());
    buf[4..=5].copy_from_slice(&(i32_array[2] as i16).to_be_bytes());
    buf[6..=7].copy_from_slice(&(i32_array[3] as i16).to_be_bytes());
}

#[inline(always)]
pub(crate) fn i32_to_i24be(i32_array: &[i32; 4], buf: &mut [u8]) {
    // assert to remove bounds checks
    assert!(buf.len() == 12);
    buf[0..=2].copy_from_slice(&i32_array[0].to_be_bytes()[1..]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_be_bytes()[1..]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_be_bytes()[1..]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_be_bytes()[1..]);
}

/// Downmix coefficient for the center, side, and rear channels when folding multichannel
/// audio into stereo. -3 dB ≈ 0.7071, the ITU-R BS.775 / ATSC A/52 stereo-downmix value.
const DOWNMIX_ATTEN: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Downmix interleaved multichannel f32 samples to interleaved stereo (L,R,L,R,...).
///
/// `channels` is the number of channels per frame in the input. The function assumes
/// standard WAVE channel order (FL, FR, FC, LFE, BL, BR, SL, SR, ...). Channels beyond
/// the 8th in WAVE order are summed equally into both L and R as a best-effort fallback.
///
/// Coefficients follow ITU-R BS.775 (also used by Blu-ray players and ffmpeg's default `-ac 2` downmix):
///   L = FL + 0.7071·FC + 0.7071·BL + 0.7071·SL
///   R = FR + 0.7071·FC + 0.7071·BR + 0.7071·SR
/// LFE is dropped. The result is hard-clamped to ±1.0 to prevent integer overflow when
/// the f32 samples are later converted to i16/i24 by `samples_to_i32`.
///
/// Special cases:
/// - 1 channel: duplicated to L=R.
/// - 2 channels: copied unchanged.
///
/// `stereo` is cleared and refilled, reusing its allocation across calls.
pub(crate) fn downmix_to_stereo(samples: &[f32], channels: u16, stereo: &mut Vec<f32>) {
    stereo.clear();
    match channels {
        0 | 2 => stereo.extend_from_slice(samples),
        1 => {
            stereo.reserve(samples.len() * 2);
            for &s in samples {
                stereo.push(s);
                stereo.push(s);
            }
        }
        n => {
            let n = n as usize;
            let frames = samples.len() / n;
            stereo.reserve(frames * 2);
            for frame in samples.chunks_exact(n) {
                // Standard WAVE channel order: FL, FR, FC, LFE, BL, BR, SL, SR.
                let fl = frame[0];
                let fr = frame[1];
                let fc = if n >= 3 { frame[2] } else { 0.0 };
                // frame[3] is LFE — intentionally dropped.
                let bl = if n >= 5 { frame[4] } else { 0.0 };
                let br = if n >= 6 { frame[5] } else { 0.0 };
                let sl = if n >= 7 { frame[6] } else { 0.0 };
                let sr = if n >= 8 { frame[7] } else { 0.0 };
                let mut l = fl + DOWNMIX_ATTEN * (fc + bl + sl);
                let mut r = fr + DOWNMIX_ATTEN * (fc + br + sr);
                // Any extra channels beyond standard 7.1 are mixed equally into both sides.
                if n > 8 {
                    for &extra in &frame[8..] {
                        l += DOWNMIX_ATTEN * extra;
                        r += DOWNMIX_ATTEN * extra;
                    }
                }
                stereo.push(l.clamp(-1.0, 1.0));
                stereo.push(r.clamp(-1.0, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_to_i32_i16_range() {
        let arr = f32x4::new([1.0, -1.0, 0.5, 0.0]);
        let result = f32_to_i32(BitDepth::Bits16, arr);
        assert_eq!(result[0], i16::MAX as i32);
        assert_eq!(result[1], i16::MIN as i32);
        //assert_eq!(result[2], i16::MAX as i32 / 2);
        assert_eq!(result[3], 0i32);
    }

    #[test]
    fn test_downmix_stereo_passthrough() {
        let input = vec![0.1, -0.2, 0.3, -0.4];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 2, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn test_downmix_mono_to_stereo() {
        let input = vec![0.5, -0.5, 0.25];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 1, &mut out);
        assert_eq!(out, vec![0.5, 0.5, -0.5, -0.5, 0.25, 0.25]);
    }

    #[test]
    fn test_downmix_5_1_bs775() {
        // One frame: FL=0.1, FR=0.2, FC=0.3, LFE=0.9 (dropped), BL=0.4, BR=0.5
        let input = vec![0.1, 0.2, 0.3, 0.9, 0.4, 0.5];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 6, &mut out);
        assert_eq!(out.len(), 2);
        let expected_l = 0.1 + DOWNMIX_ATTEN * (0.3 + 0.4);
        let expected_r = 0.2 + DOWNMIX_ATTEN * (0.3 + 0.5);
        assert!(
            (out[0] - expected_l).abs() < 1e-6,
            "L: {} vs {}",
            out[0],
            expected_l
        );
        assert!(
            (out[1] - expected_r).abs() < 1e-6,
            "R: {} vs {}",
            out[1],
            expected_r
        );
    }

    #[test]
    fn test_downmix_clamps_overflow() {
        // 5.1 frame engineered so L = 1.0 + 0.707*1.0 + 0.707*1.0 ≈ 2.41 → clamps to 1.0
        let input = vec![1.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 6, &mut out);
        assert_eq!(out, vec![1.0, 1.0]);
        // Negative side
        let input = vec![-1.0, -1.0, -1.0, 0.0, -1.0, -1.0];
        downmix_to_stereo(&input, 6, &mut out);
        assert_eq!(out, vec![-1.0, -1.0]);
    }

    #[test]
    fn test_downmix_frame_count() {
        // 12 samples / 6 channels = 2 frames → 2 stereo frames = 4 samples out
        let input = vec![0.0; 12];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 6, &mut out);
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn test_downmix_reuses_buffer() {
        // Calling twice with different inputs must clear any prior content.
        let mut out = vec![99.0, 99.0, 99.0];
        downmix_to_stereo(&[0.0; 6], 6, &mut out);
        assert_eq!(out, vec![0.0, 0.0]);
    }
}
