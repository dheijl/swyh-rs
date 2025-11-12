use std::ops::Shr;

use wide::f32x4;

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
        let i_array = f32_to_i32(shift, f32_array);
        i32_samples.extend(&i_array);
    });
    if remainder.len() == 2 {
        f32_array = [remainder[0], remainder[1], 0.0, 0.0];
        let i_array = f32_to_i32(shift, f32_array);
        i32_samples.extend(&i_array[0..2]);
    }
}

/// convert 4 f32 samples to 4 i32 samples using SSE2
#[inline(always)]
pub fn f32_to_i32(shift: u8, f32_array: [f32; 4]) -> [i32; 4] {
    const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
    let imax = f32x4::splat(I32_MAX);
    let fchunk = f32x4::from(f32_array);
    let fchunk_i32 = fchunk * imax;
    let s4i = fchunk_i32.trunc_int().shr(shift);
    s4i.to_array()
}
