use itertools::Itertools;
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

/// convert a 4 f32 samples itertools chunk to an i32 array (scaled to bitdepth)
#[inline(always)]
pub(crate) fn f32_chunk_to_i32(
    bd: BitDepth,
    sample_chunk: itertools::Chunk<'_, std::collections::vec_deque::Drain<'_, f32>>,
) -> [i32; 4] {
    let mut temp = [0f32; 4];
    temp.iter_mut().set_from(sample_chunk);
    let f32_array = f32x4::new(temp);
    f32_to_i32(bd, f32_array)
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
}
