//! Criterion benchmarks comparing the SIMD (SSSE3/NEON) byte-pack implementations
//! in `simd_impl` / `neon_impl` against the scalar fallback, and measuring
//! `samples_to_i32` throughput at 16-bit and 24-bit depth.
//!
//! Run with:
//!   cargo bench --bench samples_conv
//!
//! The scalar reference functions below are verbatim copies of the `#[cfg(not(...))]`
//! arms in `samples_conv.rs`.  They are defined here because on x86_64+ssse3 / aarch64
//! the scalar path is not compiled into the crate binary, so we cannot call it directly.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use swyh_rs::audio::samples_conv::{
    i32_to_i16be, i32_to_i16le, i32_to_i24be, i32_to_i24le, samples_to_i32,
};
use swyh_rs::enums::streaming::BitDepth;

// ---------------------------------------------------------------------------
// Scalar reference implementations (always compiled regardless of CPU target)
// ---------------------------------------------------------------------------

#[inline(never)]
fn scalar_i32_to_i16le(i32_array: &[i32; 4], buf: &mut [u8]) {
    buf[0..=1].copy_from_slice(&(i32_array[0] as i16).to_le_bytes());
    buf[2..=3].copy_from_slice(&(i32_array[1] as i16).to_le_bytes());
    buf[4..=5].copy_from_slice(&(i32_array[2] as i16).to_le_bytes());
    buf[6..=7].copy_from_slice(&(i32_array[3] as i16).to_le_bytes());
}

#[inline(never)]
fn scalar_i32_to_i16be(i32_array: &[i32; 4], buf: &mut [u8]) {
    buf[0..=1].copy_from_slice(&(i32_array[0] as i16).to_be_bytes());
    buf[2..=3].copy_from_slice(&(i32_array[1] as i16).to_be_bytes());
    buf[4..=5].copy_from_slice(&(i32_array[2] as i16).to_be_bytes());
    buf[6..=7].copy_from_slice(&(i32_array[3] as i16).to_be_bytes());
}

#[inline(never)]
fn scalar_i32_to_i24le(i32_array: &[i32; 4], buf: &mut [u8]) {
    buf[0..=2].copy_from_slice(&i32_array[0].to_le_bytes()[..=2]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_le_bytes()[..=2]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_le_bytes()[..=2]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_le_bytes()[..=2]);
}

#[inline(never)]
fn scalar_i32_to_i24be(i32_array: &[i32; 4], buf: &mut [u8]) {
    buf[0..=2].copy_from_slice(&i32_array[0].to_be_bytes()[1..]);
    buf[3..=5].copy_from_slice(&i32_array[1].to_be_bytes()[1..]);
    buf[6..=8].copy_from_slice(&i32_array[2].to_be_bytes()[1..]);
    buf[9..=11].copy_from_slice(&i32_array[3].to_be_bytes()[1..]);
}

/// Scalar f32→i32 conversion matching the SIMD path exactly: clamp, scale by 2³¹,
/// optional TPDF dither at 16-bit (2× fastrand per sample), half-LSB nudge, round, arithmetic shift.
#[inline(never)]
fn scalar_samples_to_i32(
    f32_samples: &[f32],
    i32_samples: &mut Vec<i32>,
    bd: BitDepth,
    use_dither: bool,
) {
    i32_samples.clear();
    const F32_TO_I32: f32 = (i32::MAX as f32) + 1.0; // 2^31
    const LSB_AT_16BIT: f32 = 65536.0; // 2^31 / 2^15
    let half_lsb = if bd == BitDepth::Bits16 {
        32_768.0_f32
    } else {
        128.0_f32
    };
    let shift = bd.shift_value();
    for &s in f32_samples {
        let scaled = s.clamp(-1.0, 1.0) * F32_TO_I32;
        let pre_quant = if bd == BitDepth::Bits16 {
            let dither = if use_dither {
                (fastrand::f32() - fastrand::f32()) * LSB_AT_16BIT
            } else {
                0.0
            };
            scaled + dither + half_lsb
        } else {
            scaled + half_lsb
        };
        i32_samples.push((pre_quant.round() as i32) >> shift);
    }
}

// ---------------------------------------------------------------------------
// byte-pack benchmarks: one 4-sample chunk, SIMD vs scalar
// ---------------------------------------------------------------------------

fn bench_byte_pack(c: &mut Criterion) {
    let arr: [i32; 4] = [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F];
    let mut buf16 = [0u8; 8];
    let mut buf24 = [0u8; 12];

    macro_rules! pack_group {
        ($name:expr, $simd_fn:expr, $scalar_fn:expr, $buf:expr) => {{
            let mut g = c.benchmark_group($name);
            g.bench_function("simd", |b| {
                b.iter(|| $simd_fn(black_box(&arr), black_box(&mut $buf)))
            });
            g.bench_function("scalar", |b| {
                b.iter(|| $scalar_fn(black_box(&arr), black_box(&mut $buf)))
            });
            g.finish();
        }};
    }

    pack_group!("i32_to_i16le", i32_to_i16le, scalar_i32_to_i16le, buf16);
    pack_group!("i32_to_i16be", i32_to_i16be, scalar_i32_to_i16be, buf16);
    pack_group!("i32_to_i24le", i32_to_i24le, scalar_i32_to_i24le, buf24);
    pack_group!("i32_to_i24be", i32_to_i24be, scalar_i32_to_i24be, buf24);
}

// ---------------------------------------------------------------------------
// byte-pack throughput: process a large buffer of N chunks
// ---------------------------------------------------------------------------

fn bench_byte_pack_buffer(c: &mut Criterion) {
    const CHUNKS: usize = 1024; // 4096 samples
    let i32_data: Vec<[i32; 4]> = (0..CHUNKS)
        .map(|i| {
            let b = (i * 4) as i32;
            [b, b + 1, b + 2, b + 3]
        })
        .collect();

    let mut buf16 = vec![0u8; CHUNKS * 8];
    let mut buf24 = vec![0u8; CHUNKS * 12];

    let mut g = c.benchmark_group("buffer_i16le");
    g.throughput(Throughput::Elements((CHUNKS * 4) as u64));
    g.bench_function("simd", |b| {
        b.iter(|| {
            for (chunk, out) in i32_data.iter().zip(buf16.chunks_exact_mut(8)) {
                i32_to_i16le(black_box(chunk), black_box(out));
            }
        })
    });
    g.bench_function("scalar", |b| {
        b.iter(|| {
            for (chunk, out) in i32_data.iter().zip(buf16.chunks_exact_mut(8)) {
                scalar_i32_to_i16le(black_box(chunk), black_box(out));
            }
        })
    });
    g.finish();

    let mut g = c.benchmark_group("buffer_i24le");
    g.throughput(Throughput::Elements((CHUNKS * 4) as u64));
    g.bench_function("simd", |b| {
        b.iter(|| {
            for (chunk, out) in i32_data.iter().zip(buf24.chunks_exact_mut(12)) {
                i32_to_i24le(black_box(chunk), black_box(out));
            }
        })
    });
    g.bench_function("scalar", |b| {
        b.iter(|| {
            for (chunk, out) in i32_data.iter().zip(buf24.chunks_exact_mut(12)) {
                scalar_i32_to_i24le(black_box(chunk), black_box(out));
            }
        })
    });
    g.finish();
}

// ---------------------------------------------------------------------------
// samples_to_i32: SIMD (wide f32x4) vs scalar loop
//   Cases: 16-bit with dither, 16-bit without dither, 24-bit
// ---------------------------------------------------------------------------

fn bench_samples_to_i32(c: &mut Criterion) {
    const N: usize = 4096;
    let samples: Vec<f32> = (0..N).map(|i| (i as f32 / N as f32) * 2.0 - 1.0).collect();
    let mut out = Vec::with_capacity(N);

    let mut g = c.benchmark_group("samples_to_i32");
    g.throughput(Throughput::Elements(N as u64));

    // (bit_depth, use_dither, bench label)
    let cases: &[(BitDepth, bool, &str)] = &[
        (BitDepth::Bits16, true, "16bit_dither"),
        (BitDepth::Bits16, false, "16bit_nodither"),
        (BitDepth::Bits24, false, "24bit"),
    ];

    for &(bd, use_dither, label) in cases {
        g.bench_with_input(BenchmarkId::new("simd", label), label, |b, _| {
            b.iter(|| samples_to_i32(black_box(&samples), black_box(&mut out), bd, use_dither))
        });
        g.bench_with_input(BenchmarkId::new("scalar", label), label, |b, _| {
            b.iter(|| {
                scalar_samples_to_i32(black_box(&samples), black_box(&mut out), bd, use_dither)
            })
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_byte_pack,
    bench_byte_pack_buffer,
    bench_samples_to_i32,
);
criterion_main!(benches);
