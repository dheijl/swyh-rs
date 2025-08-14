use crossbeam_channel::{Receiver, Sender, unbounded};
use dasp_sample::Sample;
use fastrand::Rng;
use flac_bound::{FlacEncoder, WriteWrapper};
#[cfg(feature = "trace_samples")]
use log::debug;
use log::info;
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    time::Duration,
};

use crate::globals::statics::THREAD_STACK;

const NOISE_PERIOD_MS: u64 = 250; // milliseconds

// the flacwriter receives the data from the encoder
// and writes them to the flac output channel
#[derive(Clone)]
pub struct FlacWriter {
    flac_out: Sender<Vec<u8>>,
}

impl FlacWriter {
    #[must_use]
    pub fn new(flac_out: Sender<Vec<u8>>) -> FlacWriter {
        FlacWriter { flac_out }
    }
}

impl Write for FlacWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.flac_out.send(buf.to_vec()) {
            Ok(()) => Ok(buf.len()),
            Err(_e) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "FlacWriter channel SendError",
            )),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// a FlacChannel is set up by the channelstream
// the ChannelStream writes the captured f32 samples
// to the samples_in channel for encoding
#[derive(Clone)]
pub struct FlacChannel {
    samples_rcvr: Receiver<Vec<f32>>,
    pub flac_in: Receiver<Vec<u8>>,
    active: Arc<AtomicBool>,
    writer: FlacWriter,
    sample_rate: u32,
    bits_per_sample: u32,
    channels: u32,
}

impl FlacChannel {
    #[must_use]
    pub fn new(
        samples_chan: Receiver<Vec<f32>>,
        sample_rate: u32,
        bits_per_sample: u32,
        channels: u32,
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
        }
    }

    pub fn run(&self) {
        // copy instance data for thread
        let samples_rdr = self.samples_rcvr.clone();
        let mut writer = self.writer.clone();
        let ch = self.channels;
        let bps = self.bits_per_sample;
        let sr = self.sample_rate;
        let l_active = self.active.clone();
        // fire up thread
        self.active.store(true, Relaxed);
        let _thr = std::thread::Builder::new()
            .name("flac_encoder".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                // we're running
                // setup the encoder
                let mut outw = WriteWrapper(&mut writer);
                let mut enc = FlacEncoder::new()
                    .unwrap()
                    .channels(ch)
                    .bits_per_sample(bps)
                    .sample_rate(sr)
                    .compression_level(0)
                    .set_limit_min_bitrate(true)
                    .init_write(&mut outw)
                    .unwrap();
                // read captured samples and encode
                let shift = if bps == 24 { 8u8 } else { 16u8 };
                // create the random generator for the white noise
                let mut rng = fastrand::Rng::with_seed(79);
                // init NOISE feature and preallocate the noise buffer
                const DIVISOR: u64 = 1000 / NOISE_PERIOD_MS;
                let noise_bufsize = ((sr * 2) / DIVISOR as u32) as usize;
                let mut noise_buf: Vec<f32> = Vec::with_capacity(noise_bufsize);
                noise_buf.resize(noise_bufsize, 0.0);
                // read and FLAC encode samples
                let mut time_out = Duration::from_millis(NOISE_PERIOD_MS);
                while l_active.load(Relaxed) {
                    if let Ok(f32_samples) = samples_rdr.recv_timeout(time_out) {
                        #[cfg(feature = "trace_samples")]
                        {
                            let zs = if f32_samples.iter().any(|&s| s != 0.0) {
                                "nonero"
                            } else {
                                "nonzero"
                            };
                            debug!("Encoding {} flac {zs} samples", f32_samples.len());
                        }
                        time_out = Duration::from_millis(NOISE_PERIOD_MS);
                        let samples = f32_samples
                            .iter()
                            .map(|s| s.to_sample::<i32>() >> shift)
                            .collect::<Vec<i32>>();
                        if enc
                            .process_interleaved(samples.as_slice(), (samples.len() / 2) as u32)
                            .is_err()
                        {
                            info!("Flac encoding interrupted.");
                            break;
                        }
                    } else {
                        time_out = Duration::from_millis(NOISE_PERIOD_MS * 2);
                        // if no samples for a certain time: send very faint near silence bursts
                        if l_active.load(Relaxed) {
                            fill_noise_buffer(&mut rng, &mut noise_buf);
                            let samples = noise_buf
                                .iter()
                                .map(|s| (s.to_sample::<i32>() >> shift) & 0x3)
                                .collect::<Vec<i32>>();
                            #[cfg(feature = "trace_samples")]
                            {
                                debug!("Encoding FLAC silence");
                            }
                            if enc
                                .process_interleaved(samples.as_slice(), (samples.len() / 2) as u32)
                                .is_err()
                            {
                                info!("Flac inject near silence interrupted.");
                                break;
                            }
                        }
                    }
                }
                let _ = enc.finish();
            })
            .unwrap();
    }

    pub fn stop(&self) {
        self.active.store(false, Relaxed);
    }
}

///
/// fill the pre-allocated noise buffer with white noise
///
fn fill_noise_buffer(rng: &mut Rng, noise_buf: &mut [f32]) {
    for sample in noise_buf.iter_mut() {
        *sample = (rng.f32() * 2.0) - 1.0
    }
}
