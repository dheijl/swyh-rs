use std::{collections::VecDeque, ops::Shr};

use wide::f32x4;

/// convert f32 samples to i32 for flac encoding
/// but scaled to i16 or i24 according to shift (8 or 16)
/// using SIMD SSE xmm registers (with the wide crate)
pub fn samples_to_i32(f32_samples: &[f32], i32_samples: &mut Vec<i32>, shift: u8) {
    i32_samples.clear();
    const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
    let imax = f32x4::splat(I32_MAX);
    let mut f32_array = [0.0; 4];
    let chunks = f32_samples.chunks_exact(4);
    let remainder = chunks.remainder();
    chunks.into_iter().for_each(|chunk| {
        f32_array.copy_from_slice(chunk); // the array forces a MOVUPS of all 4 f32 values 
        let fchunk = f32x4::from(f32_array); // into the xmm reg without using the array at all
        let fchunk_i32 = fchunk * imax;
        let s4i = fchunk_i32.trunc_int().shr(shift);
        let i_array = s4i.to_array();
        i32_samples.extend(&i_array);
    });
    if remainder.len() == 2 {
        f32_array = [remainder[0], remainder[1], 0.0, 0.0];
        let fchunk = f32x4::from(f32_array);
        let fchunk_i32 = fchunk * imax;
        let s4i = fchunk_i32.trunc_int().shr(shift);
        let i_array = s4i.to_array();
        i32_samples.extend(&i_array[0..2]);
    }
}

// convert f32samples in chunks of 4 samples to lpcm
// in the HTTP streaming buffer using SIMD SSE2
pub fn samples_to_lpcm(
    f32_samples: &mut VecDeque<f32>,
    bytes_per_sample: usize,
    little_endian: bool,
    buf: &mut [u8],
) {
    let samples_needed = buf.len() / bytes_per_sample;
    let (s1, s2) = f32_samples.as_slices();
    let (l1, l2) = {
        if s1.len() >= samples_needed {
            (samples_needed, 0)
        } else {
            (s1.len(), samples_needed - s1.len())
        }
    };
    debug_assert!(l1 + l2 == samples_needed);
    let s1_chunks = s1.chunks_exact(4);
    let s1_rem = s1_chunks.remainder();
    let mut buf_index = 0usize;
    s1_chunks.into_iter().for_each(|chunk| {
        buf_index += chunk_to_buf(bytes_per_sample, little_endian, buf, buf_index, chunk, 4);
    });
    if s1_rem.len() == 2 {
        let chunk = &[s1_rem[0], s1_rem[1], 0.0, 0.0];
        buf_index += chunk_to_buf(bytes_per_sample, little_endian, buf, buf_index, chunk, 2);
    }
    if !s2.is_empty() {
        let s2_chunks = s2.chunks_exact(4);
        let s2_rem = s2_chunks.remainder();
        s2_chunks.into_iter().for_each(|chunk| {
            buf_index += chunk_to_buf(bytes_per_sample, little_endian, buf, buf_index, chunk, 4);
        });
        if s2_rem.len() == 2 {
            let chunk = &[s1_rem[0], s1_rem[1], 0.0, 0.0];
            buf_index += chunk_to_buf(bytes_per_sample, little_endian, buf, buf_index, chunk, 2);
        }
    }
    debug_assert_eq!(buf_index, samples_needed * bytes_per_sample);
    f32_samples.drain(0..samples_needed);
}

fn chunk_to_buf(
    bytes_per_sample: usize,
    little_endian: bool,
    buf: &mut [u8],
    buf_index: usize,
    chunk: &[f32],
    nsamples: usize,
) -> usize {
    const I32_MAX: f32 = (i32::MAX as f32) + 1.0;
    let imax = f32x4::splat(I32_MAX);
    let shift = if bytes_per_sample == 2 { 16u8 } else { 8u8 };
    let mut f32_array = [0f32; 4];
    f32_array.copy_from_slice(chunk);
    let fchunk = f32x4::from(f32_array);
    let fchunk_i32 = fchunk * imax;
    let ichunk = fchunk_i32.trunc_int().shr(shift);
    let i_array = ichunk.to_array();
    match (nsamples, bytes_per_sample) {
        (2, 2) => copy_2bps(&[0, 2], little_endian, buf, buf_index, i_array),
        (2, 3) => copy_3bps(&[0, 3], little_endian, buf, buf_index, i_array),
        (4, 2) => copy_2bps(&[0, 2, 4, 6], little_endian, buf, buf_index, i_array),
        (4, 3) => copy_3bps(&[0, 3, 6, 9], little_endian, buf, buf_index, i_array),
        _ => panic!(
            "chunk_to_buf invalid combination nsamples/bytes_per_sample: {nsamples} {bytes_per_sample}"
        ),
    }
}

fn copy_2bps(
    range: &[usize],
    little_endian: bool,
    buf: &mut [u8],
    bufindex: usize,
    i_array: [i32; 4],
) -> usize {
    let mut l = bufindex;
    for i in range {
        if little_endian {
            buf[l..l + 2].copy_from_slice(&i_array[*i].to_le_bytes()[2..4]);
        } else {
            buf[l..l + 2].copy_from_slice(&i_array[*i].to_be_bytes()[2..4]);
        }
        l += 2;
    }
    l - bufindex
}

fn copy_3bps(
    range: &[usize],
    little_endian: bool,
    buf: &mut [u8],
    bufindex: usize,
    i_array: [i32; 4],
) -> usize {
    let mut l = bufindex;
    for i in range {
        if little_endian {
            buf[l..l + 3].copy_from_slice(&i_array[*i].to_le_bytes()[1..4]);
        } else {
            buf[l..l + 3].copy_from_slice(&i_array[*i].to_be_bytes()[1..4]);
        }
        l += 3;
    }
    l - bufindex
}
