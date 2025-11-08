use wide::f32x4;

/// convert f32 samples to i32
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate)
pub fn samples_to_i32(f32_samples: &[f32], i32_samples: &mut Vec<i32>, shift: u8) {
    i32_samples.clear();
    const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
    let imax = f32x4::from(I32_MAX);
    let chunks = f32_samples.chunks_exact(4);
    let remainder = chunks.remainder();
    chunks.into_iter().for_each(|chunk| {
        let fchunk = f32x4::from(chunk);
        let fchunk_ixx = fchunk * imax;
        let ichunk = fchunk_ixx.trunc_int();
        // missing the mmx integer shifts here
        let mut islice = ichunk.to_array();
        for s in islice.iter_mut() {
            *s >>= shift
        }
        i32_samples.extend_from_slice(&islice);
    });
    if remainder.len() == 2 {
        let fchunk = f32x4::from([remainder[0], remainder[1], 0.0, 0.0]);
        let fchunk_ixx = fchunk * imax;
        let ichunk = fchunk_ixx.trunc_int();
        let mut islice = ichunk.to_array();
        for s in islice[0..2].iter_mut() {
            *s >>= shift
        }
        i32_samples.extend_from_slice(&islice[0..2]);
    }
}
