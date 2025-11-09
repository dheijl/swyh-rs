use wide::f32x4;

/// convert f32 samples to i32
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate)
pub fn samples_to_i32(f32_samples: &[f32], i32_samples: &mut Vec<i32>, shift: u8) {
    i32_samples.clear();
    const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
    let imax = f32x4::from(I32_MAX);
    let mut f32_array = [0.0; 4];
    let chunks = f32_samples.chunks_exact(4);
    let remainder = chunks.remainder();
    chunks.into_iter().for_each(|chunk| {
        f32_array.copy_from_slice(chunk); // the array forces a MOVUPS of all 4 f32 values 
        let fchunk = f32x4::from(f32_array); // into the xmm reg without using the array at all
        let fchunk_i32 = fchunk * imax;
        let mut i_array = fchunk_i32.trunc_int().to_array();
        // missing the xmm integer shifts here
        for s in i_array.iter_mut() {
            *s >>= shift
        }
        i32_samples.extend(&i_array);
    });
    if remainder.len() == 2 {
        f32_array = [remainder[0], remainder[1], 0.0, 0.0];
        let fchunk = f32x4::from(f32_array);
        let fchunk_i32 = fchunk * imax;
        let mut i_array = fchunk_i32.trunc_int().to_array();
        for s in i_array[0..2].iter_mut() {
            *s >>= shift
        }
        i32_samples.extend(&i_array[0..2]);
    }
}
