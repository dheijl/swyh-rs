//! FLAC encoding pipeline.
//!
//! [`FlacWriter`] implements [`std::io::Write`] and feeds encoded bytes into a crossbeam channel.
//! [`FlacChannel`] owns a background encoding thread that reads captured f32 samples,
//! encodes them with `flac-bound`, and injects near-silence bursts when no samples arrive.

use crate::{
    audio::{rwstream::AudioSamples, samples_conv::samples_to_i32},
    enums::streaming::{BitDepth, Dither},
    fl,
    globals::statics::THREAD_STACK,
    utils::ui_logger::{LogCategory, ui_log},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use fastrand::Rng;
use flac_bound::{FlacEncoder, WriteWrapper};
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    time::Duration,
};

const NOISE_PERIOD_MS: u64 = 250; // milliseconds

/// the flacwriter receives the data from the encoder
/// and writes them to the flac output channel
#[derive(Clone)]
pub(crate) struct FlacWriter {
    flac_out: Sender<Vec<u8>>,
}

impl FlacWriter {
    #[must_use]
    pub(crate) fn new(flac_out: Sender<Vec<u8>>) -> FlacWriter {
        FlacWriter { flac_out }
    }
}

impl Write for FlacWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.flac_out.send(buf.to_vec()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "FlacWriter channel SendError",
            )
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// a FlacChannel is set up by the channelstream
/// the ChannelStream writes the captured f32 samples
/// to the samples_in channel for encoding
#[derive(Clone)]
pub(crate) struct FlacChannel {
    samples_rcvr: Receiver<AudioSamples>,
    pub(crate) flac_in: Receiver<Vec<u8>>,
    active: Arc<AtomicBool>,
    writer: FlacWriter,
    sample_rate: u32,
    bits_per_sample: BitDepth,
    channels: u16,
    use_dither: Dither,
}

impl FlacChannel {
    #[must_use]
    pub(crate) fn new(
        samples_chan: Receiver<AudioSamples>,
        sample_rate: u32,
        bits_per_sample: BitDepth,
        channels: u16,
        use_dither: Dither,
    ) -> FlacChannel {
        let (flac_out, flac_in): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = unbounded();
        FlacChannel {
            samples_rcvr: samples_chan,
            flac_in,
            active: Arc::new(AtomicBool::new(false)),
            writer: FlacWriter::new(flac_out),
            sample_rate,
            bits_per_sample,
            channels,
            use_dither,
        }
    }

    pub(crate) fn run(&self) {
        // copy instance data for thread
        if self.active.load(Relaxed) {
            let msg = fl!("err-flac-already-running");
            ui_log(LogCategory::Error, &msg);
            panic!("{msg}");
        }
        let samples_rdr = self.samples_rcvr.clone();
        let mut writer = self.writer.clone();
        let ch = self.channels;
        let bd = self.bits_per_sample;
        let sr = self.sample_rate;
        let use_dither = self.use_dither;
        let l_active = self.active.clone();
        // fire up thread
        self.active.store(true, Relaxed);
        let _thr = std::thread::Builder::new()
            .name("flac_encoder".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                // we're running, setup the encoder
                let mut outw = WriteWrapper(&mut writer);
                let mut enc = FlacEncoder::new()
                    .unwrap_or_else(|| {
                        let msg = fl!("err-flac-cant-start");
                        ui_log(LogCategory::Error, &msg);
                        panic!("{msg}");
                    })
                    .channels(ch as u32)
                    .bits_per_sample(bd as u32)
                    .sample_rate(sr)
                    .compression_level(1)
                    .set_limit_min_bitrate(true)
                    .init_write(&mut outw)
                    .unwrap_or_else(|e| {
                        let msg = fl!("err-flac-start-error", "error" = format!("{e:?}"));
                        ui_log(LogCategory::Error, &msg);
                        panic!("{msg}");
                    });
                // create the random generator for the white noise
                let mut rng = fastrand::Rng::with_seed(79);
                // init NOISE feature and preallocate the noise buffer
                const DIVISOR: u64 = 1000 / NOISE_PERIOD_MS;
                let noise_bufsize = ((sr * u32::from(ch)) / DIVISOR as u32) as usize;
                let mut noise_buf: Vec<i32> = vec![0; noise_bufsize];
                // read and FLAC encode samples
                let mut time_out = Duration::from_millis(NOISE_PERIOD_MS);
                let mut i32_samples = Vec::<i32>::with_capacity(16384);
                while l_active.load(Relaxed) {
                    if let Ok(f32_samples) = samples_rdr.recv_timeout(time_out) {
                        time_out = Duration::from_millis(NOISE_PERIOD_MS);
                        samples_to_i32(&f32_samples, &mut i32_samples, bd, use_dither);
                        if enc
                            .process_interleaved(
                                &i32_samples,
                                (i32_samples.len() / ch as usize) as u32,
                            )
                            .is_err()
                        {
                            ui_log(LogCategory::Warning, &fl!("flac-encoder-end"));
                            break;
                        }
                    } else if l_active.load(Relaxed) {
                        time_out = Duration::from_millis(NOISE_PERIOD_MS * 2);
                        // if no samples for a certain time: send very faint near silence bursts
                        fill_noise_buffer(&mut rng, &mut noise_buf);
                        if enc
                            .process_interleaved(&noise_buf, (noise_buf.len() / ch as usize) as u32)
                            .is_err()
                        {
                            ui_log(LogCategory::Warning, &fl!("flac-encoder-silence-end"));
                            break;
                        }
                    }
                }
                let _ = enc.finish(); // thread stopped, for whatever reason
                ui_log(LogCategory::Info, &fl!("flac-encoder-exit"));
            })
            .unwrap_or_else(|e| {
                let msg = fl!("err-flac-spawn", "error" = format!("{e:?}"));
                ui_log(LogCategory::Error, &msg);
                panic!("{msg}");
            });
    }

    pub(crate) fn stop(&self) {
        self.active.store(false, Relaxed);
    }
}

/// Lowest noise sample value (inclusive), in LSB of the output bit depth.
const NOISE_MIN: i32 = -4;
/// Highest noise sample value (exclusive), in LSB of the output bit depth.
const NOISE_MAX: i32 = 4;

/// Fill the pre-allocated noise buffer with very faint white noise:
/// uniformly distributed integers in `[NOISE_MIN, NOISE_MAX)` LSB.
///
/// The samples are right-justified, so the amplitude in LSB is the same for
/// every bit depth (and thus ~48 dB quieter at 24 bit than at 16 bit).
/// The range size is a power of two, so `fastrand` never needs to reject a draw.
fn fill_noise_buffer(rng: &mut Rng, noise_buf: &mut [i32]) {
    noise_buf
        .iter_mut()
        .for_each(|s| *s = rng.i32(NOISE_MIN..NOISE_MAX));
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOISE_VALUES: usize = (NOISE_MAX - NOISE_MIN) as usize;

    /// one 250 ms burst of 48 kHz stereo, the typical real-world noise buffer
    fn noise_burst(rng: &mut Rng) -> Vec<i32> {
        let mut buf = vec![0; 48_000 * 2 / 4];
        fill_noise_buffer(rng, &mut buf);
        buf
    }

    #[test]
    fn noise_stays_within_range() {
        let mut rng = Rng::with_seed(79);
        let buf = noise_burst(&mut rng);
        assert!(buf.iter().all(|&s| (NOISE_MIN..NOISE_MAX).contains(&s)));
    }

    #[test]
    fn noise_is_not_silent() {
        let mut rng = Rng::with_seed(79);
        let buf = noise_burst(&mut rng);
        let nonzero = buf.iter().filter(|&&s| s != 0).count();
        // 7 of the 8 values are non-zero
        assert!(
            nonzero > buf.len() * 3 / 4,
            "only {nonzero} non-zero samples"
        );
    }

    #[test]
    fn noise_is_uniformly_distributed() {
        let mut rng = Rng::with_seed(79);
        let buf = noise_burst(&mut rng);
        let mut counts = [0usize; NOISE_VALUES];
        for &s in &buf {
            counts[(s - NOISE_MIN) as usize] += 1;
        }
        let expected = buf.len() / NOISE_VALUES; // 3000, std dev ~51
        for (i, &c) in counts.iter().enumerate() {
            let value = i as i32 + NOISE_MIN;
            assert!(
                c.abs_diff(expected) < expected / 10,
                "value {value} occurs {c} times, expected ~{expected}: {counts:?}"
            );
        }
    }

    #[test]
    fn noise_samples_are_uncorrelated() {
        let mut rng = Rng::with_seed(79);
        let buf = noise_burst(&mut rng);
        let n = buf.len() as f64;
        let mean = buf.iter().map(|&s| f64::from(s)).sum::<f64>() / n;
        let var = buf
            .iter()
            .map(|&s| (f64::from(s) - mean).powi(2))
            .sum::<f64>()
            / n;
        // lag-1 and lag-2 (same channel in stereo) autocorrelation should be ~0
        for lag in [1, 2] {
            let cov = buf
                .iter()
                .zip(&buf[lag..])
                .map(|(&a, &b)| (f64::from(a) - mean) * (f64::from(b) - mean))
                .sum::<f64>()
                / (n - lag as f64);
            let r = cov / var;
            // std error is 1/sqrt(n) ~ 0.0091
            assert!(r.abs() < 0.05, "lag-{lag} autocorrelation {r} too high");
        }
    }

    #[test]
    fn successive_bursts_differ() {
        let mut rng = Rng::with_seed(79);
        let first = noise_burst(&mut rng);
        let second = noise_burst(&mut rng);
        assert_ne!(first, second);
    }
}
