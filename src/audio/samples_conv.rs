//! SIMD-accelerated sample format conversion utilities.
//!
//! Converts captured f32 audio samples to integer PCM formats (i16/i24, little/big endian)
//! using SSE2 f32x4 SIMD registers via the `wide` crate.
//!
//! # Quantization quality
//!
//! The audio paths (FLAC and LPCM) use unbiased round-to-nearest quantization with explicit
//! ±1.0 clamping at the f32 boundary, plus triangular-PDF (TPDF) dither at 16 bits to
//! decorrelate quantization noise from the signal. This follows the AES recommendation
//! (Lipshitz/Wannamaker/Vanderkooy 1992, JAES Vol 40 No 5). At 24 bits the residual
//! quantization noise floor (~−141 dBFS) is below the threshold of audibility on any
//! consumer playback chain, so dither is skipped.

use wide::f32x4;

use crate::enums::streaming::BitDepth;

/// conversion constant for f32 sample to i32
const F32_TO_I32: f32 = (i32::MAX as f32) + 1.0;
/// XMM register constant
static F32_TO_I32_SIMD: f32x4 = f32x4::splat(F32_TO_I32);
/// f32 clamp rails — applied before quantization so out-of-range samples (which WASAPI
/// shared-mode mixing can produce on intersample peaks) clip to full scale instead of
/// relying on `wide`'s post-conversion saturation. Makes the stereo passthrough path
/// behave identically to the downmix path, which already clamps.
static CLAMP_HI: f32x4 = f32x4::splat(1.0);
static CLAMP_LO: f32x4 = f32x4::splat(-1.0);

/// One LSB at 16-bit, expressed in the post-multiply (i32-scaled) f32 domain.
/// `F32_TO_I32 / 2^15 == 2^31 / 2^15 == 2^16 == 65536.0`.
const LSB_AT_16BIT_POST_MULT: f32 = 65536.0;

/// Half-LSB nudge at 16-bit, applied in the post-multiply f32 domain before
/// `round_int + arithmetic shift`. Without this, the composition `round_int(x) >> 16`
/// is floor-after-round (the shift floors toward −∞), introducing a constant
/// −0.5 LSB DC bias regardless of sign — defeating the whole purpose of
/// round-to-nearest. Adding 32768 before the shift converts floor-shift into
/// round-to-nearest-shift.
static HALF_LSB_16_SIMD: f32x4 = f32x4::splat(32_768.0);
/// Half-LSB nudge at 24-bit (= 2^7). Same logic as the 16-bit case; the bias would
/// be ~−178 dBFS so it's inaudible, but applying the nudge anyway keeps both code
/// paths unbiased and the math symmetric.
static HALF_LSB_24_SIMD: f32x4 = f32x4::splat(128.0);

/// Generate one f32x4 of triangular-PDF (TPDF) dither for 16-bit quantization, in the
/// post-multiply f32 domain. Each lane is `(u1 − u2) · LSB`, with u1, u2 ∼ U[0, 1).
/// Peak amplitude ±1 LSB, mean 0, triangular density on (−1, 1) LSB. Reads from
/// `fastrand`'s per-thread RNG.
#[inline(always)]
fn tpdf_dither_lanes_16_threadlocal() -> f32x4 {
    f32x4::new([
        (fastrand::f32() - fastrand::f32()) * LSB_AT_16BIT_POST_MULT,
        (fastrand::f32() - fastrand::f32()) * LSB_AT_16BIT_POST_MULT,
        (fastrand::f32() - fastrand::f32()) * LSB_AT_16BIT_POST_MULT,
        (fastrand::f32() - fastrand::f32()) * LSB_AT_16BIT_POST_MULT,
    ])
}

/// convert f32 samples to i32 for flac encoding
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate).
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

/// Convert 4 f32 samples in an f32x4 to 4 i32 samples (using SIMD), scaled to the
/// requested bit depth. Steps: clamp ±1.0 → multiply by 2³¹ → add TPDF dither (16-bit
/// only) → round-to-nearest → arithmetic right-shift.
///
/// At 24 bits the dither is skipped: the residual quantization noise floor at 24 bits
/// (~−141 dBFS) is below audibility on any consumer playback chain, so dither would
/// add only noise without perceptual benefit.
///
/// `round_int` saturates out-of-range inputs and maps NaN to 0 (per the `wide` crate's
/// documented contract — it wraps the underlying SSE `CVTPS2DQ` to give defined
/// behaviour). Defence in depth against any path that bypasses the clamp.
#[inline(always)]
pub(crate) fn f32_to_i32(bd: BitDepth, f32_simd: f32x4) -> [i32; 4] {
    let clamped = f32_simd.fast_min(CLAMP_HI).fast_max(CLAMP_LO);
    let scaled = clamped * F32_TO_I32_SIMD;
    // Apply TPDF dither at 16 bits (skipped at 24), then add the half-LSB nudge that
    // converts the subsequent `round_int + arithmetic-shift` into true round-to-nearest.
    // Without the nudge, `>> N` floors toward −∞ and re-introduces a constant −0.5 LSB
    // DC bias even though `round_int` itself rounds correctly.
    let pre_quant = if bd == BitDepth::Bits16 {
        scaled + tpdf_dither_lanes_16_threadlocal() + HALF_LSB_16_SIMD
    } else {
        scaled + HALF_LSB_24_SIMD
    };
    let s4i = pre_quant.round_int() >> bd.shift_value();
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
        0 => {}
        2 => stereo.extend_from_slice(samples),
        1 => {
            stereo.reserve(samples.len() * 2);
            let chunks = samples.chunks_exact(4);
            let remainder = chunks.remainder();
            for chunk in chunks {
                let v = f32x4::new(*chunk.as_array().unwrap());
                stereo.extend_from_slice(v.unpack_lo(v).as_array()); // [a,a,b,b]
                stereo.extend_from_slice(v.unpack_hi(v).as_array()); // [c,c,d,d]
            }
            for &s in remainder {
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

    /// Test-only seeded variant of the TPDF dither generator. The production code
    /// uses `tpdf_dither_lanes_16_threadlocal` (which reads `fastrand`'s thread-local
    /// state); for deterministic tests we want to control the seed directly.
    fn tpdf_dither_lanes_16_seeded(rng: &mut fastrand::Rng) -> f32x4 {
        f32x4::new([
            (rng.f32() - rng.f32()) * LSB_AT_16BIT_POST_MULT,
            (rng.f32() - rng.f32()) * LSB_AT_16BIT_POST_MULT,
            (rng.f32() - rng.f32()) * LSB_AT_16BIT_POST_MULT,
            (rng.f32() - rng.f32()) * LSB_AT_16BIT_POST_MULT,
        ])
    }

    #[test]
    fn test_f32_to_i32_i16_range() {
        // 16-bit conversion is dithered, so output is a random variable within
        // ±1 LSB of the deterministic round-to-nearest value. We assert the
        // tolerance band rather than exact equality.
        let arr = f32x4::new([1.0, -1.0, 0.5, 0.0]);
        let result = f32_to_i32(BitDepth::Bits16, arr);
        // For input 1.0: clamp leaves 1.0, multiply gives ≈ 2^31 (in f32). Adding
        // any dither in (−65536, +65536) plus the 32768 nudge then arithmetic-shifting
        // by 16 always lands at 32767 (positive dither saturates round_int to i32::MAX
        // → 32767; negative dither gives (2^31 − 32768) >> 16 = 32767). Symmetric for
        // −1.0. So [0]/[1] are deterministically i16::MAX / i16::MIN here; the ±1 LSB
        // band is only meaningfully exercised at [3] (input 0.0).
        assert!((result[0] - i16::MAX as i32).abs() <= 1);
        assert!((result[1] - i16::MIN as i32).abs() <= 1);
        // 0.5 · 2^31 = 2^30; (2^30) >> 16 = 2^14 = 16384. With TPDF dither at
        // ±1 LSB the result is in {16383, 16384, 16385}.
        assert!((result[2] - (1 << 14)).abs() <= 1);
        assert!(result[3].abs() <= 1);
    }

    #[test]
    fn test_f32_to_i32_clamps_above_one() {
        // WASAPI shared-mode mixing can produce intersample peaks > 1.0. The clamp
        // must map these to full-scale, not let them wrap or drift via SIMD saturation.
        // At 24-bit (no dither) the result is exact; at 16-bit dither could pull it
        // 1 LSB below i16::MAX so we assert the ±1 LSB band there.
        let arr = f32x4::new([1.05, 1.5, 100.0, f32::INFINITY]);
        let result_24 = f32_to_i32(BitDepth::Bits24, arr);
        for v in result_24 {
            assert_eq!(v, 0x7F_FFFF);
        }
        let result_16 = f32_to_i32(BitDepth::Bits16, arr);
        for v in result_16 {
            assert!((v - i16::MAX as i32).abs() <= 1);
        }
    }

    #[test]
    fn test_f32_to_i32_clamps_below_neg_one() {
        let arr = f32x4::new([-1.05, -1.5, -100.0, f32::NEG_INFINITY]);
        let result_24 = f32_to_i32(BitDepth::Bits24, arr);
        for v in result_24 {
            assert_eq!(v, -0x80_0000);
        }
        let result_16 = f32_to_i32(BitDepth::Bits16, arr);
        for v in result_16 {
            assert!((v - i16::MIN as i32).abs() <= 1);
        }
    }

    #[test]
    fn test_f32_to_i32_handles_nan() {
        // NaN cannot occur in practice (WASAPI does not produce NaN samples), but the
        // pipeline must not panic if it ever did. On x86, MINPS/MAXPS — which back
        // `fast_min`/`fast_max` — return the non-NaN operand when one input is NaN, so
        // our ±1.0 clamps coerce any NaN to a finite rail before quantization. The
        // exact rail it lands on is platform-/operand-order-dependent; we only assert
        // the output stays inside the legal i16 range and is finite.
        let arr = f32x4::new([f32::NAN, 0.0, 0.0, 0.0]);
        let result = f32_to_i32(BitDepth::Bits16, arr);
        assert!(
            result[0] >= i16::MIN as i32 && result[0] <= i16::MAX as i32,
            "NaN escaped clamp: {}",
            result[0]
        );
    }

    #[test]
    fn test_f32_to_i32_24bit_unchanged() {
        // 24-bit path is undithered and deterministic — known reference values must
        // continue to match exactly. This test guards against accidental dithering
        // creeping into the 24-bit path in the future.
        let arr = f32x4::new([1.0, -1.0, 0.5, 0.0]);
        let result = f32_to_i32(BitDepth::Bits24, arr);
        // 1.0 · 2^31 saturates to i32::MAX, then >> 8 = 0x7FFFFF
        assert_eq!(result[0], 0x7F_FFFF);
        // -1.0 · 2^31 = -2^31, >> 8 = -0x80_0000 (sign-extended arithmetic shift)
        assert_eq!(result[1], -0x80_0000);
        // 0.5 · 2^31 = 2^30, >> 8 = 2^22
        assert_eq!(result[2], 1 << 22);
        assert_eq!(result[3], 0);
    }

    #[test]
    fn test_tpdf_dither_zero_mean() {
        // 10 000 dither samples at 16-bit should have mean within ±0.1 LSB of zero.
        // (Standard error of mean for U(-1,1)+U(-1,1) over N samples is √(2/3 N) LSB,
        // ≈ 0.0082 LSB at N=10 000 — 0.1 LSB is a comfortable bound.)
        let mut rng = fastrand::Rng::with_seed(0xDEAD_BEEF);
        let mut sum = 0.0f64;
        let mut count = 0u32;
        for _ in 0..2500 {
            let lanes = tpdf_dither_lanes_16_seeded(&mut rng).to_array();
            for v in lanes {
                sum += v as f64;
                count += 1;
            }
        }
        let mean_lsb = (sum / count as f64) / LSB_AT_16BIT_POST_MULT as f64;
        assert!(
            mean_lsb.abs() < 0.1,
            "TPDF dither mean drifted: {mean_lsb} LSB"
        );
    }

    #[test]
    fn test_tpdf_dither_peak_within_two_lsb() {
        // TPDF on (u1 - u2) has support (-1, 1) LSB. Allow a small epsilon for
        // floating-point boundary effects.
        let mut rng = fastrand::Rng::with_seed(42);
        for _ in 0..2500 {
            let lanes = tpdf_dither_lanes_16_seeded(&mut rng).to_array();
            for v in lanes {
                let abs_lsb = v.abs() / LSB_AT_16BIT_POST_MULT;
                assert!(
                    abs_lsb < 1.0 + 1e-6,
                    "TPDF dither out of range: {abs_lsb} LSB"
                );
            }
        }
    }

    #[test]
    fn test_tpdf_dither_seeded_deterministic() {
        // Regression guard: with a fixed seed the dither sequence must be reproducible.
        let mut rng_a = fastrand::Rng::with_seed(123);
        let mut rng_b = fastrand::Rng::with_seed(123);
        for _ in 0..16 {
            let a = tpdf_dither_lanes_16_seeded(&mut rng_a).to_array();
            let b = tpdf_dither_lanes_16_seeded(&mut rng_b).to_array();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn test_f32_to_i32_16bit_dither_bounded_and_unbiased() {
        // The 16-bit dithered output is a random variable. Two checks:
        //  (a) The spread across many trials must be ≤ 3 LSBs — TPDF dither has open
        //      support (-1, 1) LSB, and after round_int the output lands in one of at
        //      most 3 adjacent integer values.
        //  (b) The mean across many trials must be close to the analytical
        //      round-to-nearest value `round(f · 2^15)`, because TPDF is zero-mean.
        //      With N=1000 the standard error of the mean is well below 0.1 LSB.
        let inputs = [0.1f32, -0.3, 0.5, -0.7];
        let arr = f32x4::new(inputs);
        let mut lane_samples: [Vec<i32>; 4] = Default::default();
        for _ in 0..1000 {
            let r = f32_to_i32(BitDepth::Bits16, arr);
            for i in 0..4 {
                lane_samples[i].push(r[i]);
            }
        }
        for i in 0..4 {
            let max = *lane_samples[i].iter().max().unwrap();
            let min = *lane_samples[i].iter().min().unwrap();
            assert!(
                max - min <= 3,
                "lane {i} spread too large: {min}..{max} (range {})",
                max - min
            );
            let mean: f64 = lane_samples[i].iter().map(|&v| v as f64).sum::<f64>()
                / lane_samples[i].len() as f64;
            let ref_val = (inputs[i] * 32_768.0_f32).round() as i32;
            assert!(
                (mean - ref_val as f64).abs() < 0.5,
                "lane {i} mean {mean} drifted from analytical reference {ref_val}"
            );
        }
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
        // 3 samples: all go through the scalar remainder path (< 4 samples per chunk)
        let input = vec![0.5, -0.5, 0.25];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 1, &mut out);
        assert_eq!(out, vec![0.5, 0.5, -0.5, -0.5, 0.25, 0.25]);
    }

    #[test]
    fn test_downmix_mono_to_stereo_simd_exact() {
        // 4 samples: exercises exactly one SIMD chunk, no remainder
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 1, &mut out);
        assert_eq!(out, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4]);
    }

    #[test]
    fn test_downmix_mono_to_stereo_simd_with_remainder() {
        // 6 samples: one SIMD chunk (4) + scalar remainder (2)
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 1, &mut out);
        assert_eq!(
            out,
            vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4, 0.5, 0.5, 0.6, 0.6]
        );
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

    #[test]
    fn test_downmix_7_1_bs775() {
        // One frame: FL=0.1, FR=0.2, FC=0.1, LFE=0.0 (dropped), BL=0.1, BR=0.1, SL=0.1, SR=0.1
        // Values kept small to stay below the ±1.0 clamp.
        let input = vec![0.1f32, 0.2, 0.1, 0.0, 0.1, 0.1, 0.1, 0.1];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 8, &mut out);
        assert_eq!(out.len(), 2);
        let expected_l = 0.1 + DOWNMIX_ATTEN * (0.1 + 0.1 + 0.1);
        let expected_r = 0.2 + DOWNMIX_ATTEN * (0.1 + 0.1 + 0.1);
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
    fn test_downmix_beyond_8_channels() {
        // 9-channel frame: standard 7.1 + one extra channel at index 8.
        // Extra channel (0.5) is mixed equally into both sides with DOWNMIX_ATTEN.
        // FL=0.1, FR=0.2, FC=0.0, LFE=0.0, BL=0.0, BR=0.0, SL=0.0, SR=0.0, extra=0.5
        let input = vec![0.1f32, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5];
        let mut out = Vec::new();
        downmix_to_stereo(&input, 9, &mut out);
        assert_eq!(out.len(), 2);
        let expected_l = 0.1 + DOWNMIX_ATTEN * 0.5;
        let expected_r = 0.2 + DOWNMIX_ATTEN * 0.5;
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
    fn test_downmix_zero_channels() {
        // 0-channel input: stereo output must be empty.
        let mut out = vec![1.0, 2.0];
        downmix_to_stereo(&[], 0, &mut out);
        assert!(out.is_empty());
    }
}
