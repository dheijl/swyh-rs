use crossbeam_channel::{Receiver, Sender, unbounded};
use fastrand::Rng;
use flac_bound::{FlacEncoder, WriteWrapper};
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
    },
    time::Duration,
};
use wide::f32x4;

use crate::{
    enums::streaming::BitDepth,
    globals::statics::THREAD_STACK,
    utils::{
        samples_conv::{f32_to_i32, samples_to_i32},
        ui_logger::{LogCategory, ui_log},
    },
};

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
        if self.active.load(Acquire) {
            let msg = "Flac encoder already running!";
            ui_log(LogCategory::Error, msg);
            panic!("{msg}");
        }
        let samples_rdr = self.samples_rcvr.clone();
        let mut writer = self.writer.clone();
        let ch = self.channels;
        let bps = self.bits_per_sample;
        let sr = self.sample_rate;
        let l_active = self.active.clone();
        // fire up thread
        self.active.store(true, Release);
        let _thr = std::thread::Builder::new()
            .name("flac_encoder".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                // we're running
                // setup the encoder
                let mut outw = WriteWrapper(&mut writer);
                let mut enc = FlacEncoder::new()
                    .unwrap_or_else(|| {
                        let msg = "Can't start FLAC encoder";
                        ui_log(LogCategory::Error, msg);
                        panic!("{msg}");
                    })
                    .channels(ch)
                    .bits_per_sample(bps)
                    .sample_rate(sr)
                    .compression_level(1)
                    .set_limit_min_bitrate(true)
                    .init_write(&mut outw)
                    .unwrap_or_else(|e| {
                        let msg = format!("Flac encoder start error {e:?}");
                        ui_log(LogCategory::Error, &msg);
                        panic!("{msg}");
                    });
                // read captured samples and encode
                let bd = BitDepth::from(bps as u16);
                // create the random generator for the white noise
                let mut rng = fastrand::Rng::with_seed(79);
                // init NOISE feature and preallocate the noise buffer
                const DIVISOR: u64 = 1000 / NOISE_PERIOD_MS;
                let noise_bufsize = ((sr * ch) / DIVISOR as u32) as usize;
                let mut noise_buf: Vec<i32> = vec![0; noise_bufsize];
                // read and FLAC encode samples
                let mut time_out = Duration::from_millis(NOISE_PERIOD_MS);
                let mut i32_samples = Vec::<i32>::with_capacity(16384);
                while l_active.load(Acquire) {
                    if let Ok(f32_samples) = samples_rdr.recv_timeout(time_out) {
                        time_out = Duration::from_millis(NOISE_PERIOD_MS);
                        samples_to_i32(&f32_samples, &mut i32_samples, bd);
                        if enc
                            .process_interleaved(
                                &i32_samples,
                                (i32_samples.len() / ch as usize) as u32,
                            )
                            .is_err()
                        {
                            ui_log(LogCategory::Warning, "Flac encoder: stopped.");
                            break;
                        }
                    } else {
                        if l_active.load(Acquire) {
                            time_out = Duration::from_millis(NOISE_PERIOD_MS * 2);
                            // if no samples for a certain time: send very faint near silence bursts
                            fill_noise_buffer(&mut rng, bd, &mut noise_buf);
                            if enc
                                .process_interleaved(
                                    &noise_buf,
                                    (noise_buf.len() / ch as usize) as u32,
                                )
                                .is_err()
                            {
                                ui_log(LogCategory::Warning, "Flac inject near silence: stopped.");
                                break;
                            }
                        }
                    }
                }
                ui_log(LogCategory::Info, "Flac encoder thread exit.");
                let _ = enc.finish(); // for whatever reason
            })
            .unwrap_or_else(|e| {
                let msg = format!("Failed to spawn Flac encoder thread: {e:?}.");
                ui_log(LogCategory::Error, &msg);
                panic!("{msg}");
            });
    }

    pub fn stop(&self) {
        self.active.store(false, Release);
    }
}

///
/// fill the pre-allocated noise buffer with white noise
///
fn fill_noise_buffer(rng: &mut Rng, bd: BitDepth, noise_buf: &mut [i32]) {
    let mut f32_array = [0f32; 4];
    for samples in noise_buf.chunks_mut(4) {
        // prepare 4 samples, possibly wasting 2 if last chunk is only 2 samples
        f32_array[0] = (rng.f32() * 2.0) - 1.0;
        f32_array[1] = (rng.f32() * 2.0) - 1.0;
        f32_array[2] = (rng.f32() * 2.0) - 1.0;
        f32_array[3] = (rng.f32() * 2.0) - 1.0;
        let f32_sse = f32x4::new(f32_array);
        let i32_array = f32_to_i32(bd, f32_sse);
        samples
            .iter_mut()
            .zip(i32_array)
            .for_each(|s| *s.0 = (s.1 >> bd.shift_value()) & 0x03);
    }
}
