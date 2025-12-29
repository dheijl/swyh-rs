use std::ops::Shr;

use wide::f32x4;

/// conversion constant for f32 sample to i32
const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
/// XMM register constant
static I32_MAX_XMM: f32x4 = f32x4::splat(I32_MAX);

/// convert f32 samples to i32 for flac encoding
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate)
pub fn samples_to_i32(f32_samples: &[f32], i32_samples: &mut Vec<i32>, shift: u8) {
    i32_samples.clear();
    let mut f32_array = [0.0; 4];
    let chunks = f32_samples.chunks_exact(4);
    let remainder = chunks.remainder();
    chunks.into_iter().for_each(|chunk| {
        f32_array.copy_from_slice(chunk);
        let i_array = f32_to_i32(shift, &f32_array);
        i32_samples.extend(&i_array);
    });
    if remainder.len() == 2 {
        f32_array = [remainder[0], remainder[1], 0.0, 0.0];
        let i_array = f32_to_i32(shift, &f32_array);
        i32_samples.extend(&i_array[0..2]);
    }
}

/// convert 4 f32 samples to 4 i32 samples using SSE2
#[inline(always)]
pub fn f32_to_i32(shift: u8, f32_array: &[f32; 4]) -> [i32; 4] {
    let fchunk = f32x4::new(*f32_array);
    let fchunk_i32 = fchunk * I32_MAX_XMM;
    let s4i = fchunk_i32.trunc_int().shr(shift);
    s4i.to_array()
}

#[inline(always)]
pub fn i32_to_i16le(i32_array: &[i32; 4], buf: &mut [u8]) {
    // remove bounds checks
    assert!(buf.len() == 8);
    buf[0..=1].copy_from_slice(&i32_array[0].to_le_bytes()[..=1]);
    buf[2..=3].copy_from_slice(&i32_array[1].to_le_bytes()[..=1]);
    buf[4..=5].copy_from_slice(&i32_array[2].to_le_bytes()[..=1]);
    buf[6..=7].copy_from_slice(&i32_array[3].to_le_bytes()[..=1]);
}

#[inline(always)]
pub fn i32_to_i24le(i32_array: &[i32; 4], buf: &mut [u8]) {
    // remove bounds checks
    assert!(buf.len() == 12);
    buf[0..=2].copy_from_slice(&i32_array[0].to_le_bytes()[..=2]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_le_bytes()[..=2]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_le_bytes()[..=2]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_le_bytes()[..=2]);
}

#[inline(always)]
pub fn i32_to_i16be(i32_array: &[i32; 4], buf: &mut [u8]) {
    // remove bounds checks
    assert!(buf.len() == 8);
    buf[0..=1].copy_from_slice(&i32_array[0].to_be_bytes()[2..]);
    buf[2..=3].copy_from_slice(&i32_array[1].to_be_bytes()[2..]);
    buf[4..=5].copy_from_slice(&i32_array[2].to_be_bytes()[2..]);
    buf[6..=7].copy_from_slice(&i32_array[3].to_be_bytes()[2..]);
}

#[inline(always)]
pub fn i32_to_i24be(i32_array: &[i32; 4], buf: &mut [u8]) {
    // remove bounds checks
    assert!(buf.len() == 12);
    buf[0..=2].copy_from_slice(&i32_array[0].to_be_bytes()[1..]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_be_bytes()[1..]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_be_bytes()[1..]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_be_bytes()[1..]);
}
