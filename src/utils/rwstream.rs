/*
///
/// rwstream.rs
///
/// ChannelStream: the write method sends the received samples on the CrssBeam channel
/// for the Read trait to read them back
///
/// the Read trait implementation is used by the HTTP response to send the response PCM/L16 stream
/// to the media Renderer
///
*/
use crate::utils::i24::I24Sample;
use cpal::Sample;
use crossbeam_channel::{Receiver, Sender};
use log::debug;
use std::collections::VecDeque;
use std::io::Read;
use std::io::Result as IoResult;
use std::time::Duration;

/// Channelstream - used to transport the samples from the wave_reader to the http output wav stream
#[derive(Clone)]
pub struct ChannelStream {
    pub s: Sender<Vec<f32>>,
    pub r: Receiver<Vec<f32>>,
    pub remote_ip: String,
    fifo: VecDeque<f32>,
    silence: Vec<f32>,
    capture_timeout: Duration,
    silence_period: Duration,
    sending_silence: bool,
    wav_hdr: Vec<u8>,
    use_wave_format: bool,
    bits_per_sample: u16,
}

const CAPTURE_TIMEOUT: u32 = 2; // seconds
const SILENCE_PERIOD: u32 = 250; // milliseconds

impl ChannelStream {
    pub fn new(
        tx: Sender<Vec<f32>>,
        rx: Receiver<Vec<f32>>,
        remote_ip_addr: String,
        use_wave_format: bool,
        sample_rate: u32,
        bits_per_sample: u16,
    ) -> ChannelStream {
        ChannelStream {
            s: tx,
            r: rx,
            fifo: VecDeque::with_capacity(16384),
            silence: Vec::new(),
            capture_timeout: Duration::new(CAPTURE_TIMEOUT as u64, 0), // silence kicks in after CAPTURE_TIMEOUT seconds
            silence_period: Duration::from_millis(SILENCE_PERIOD as u64), // send SILENCE_PERIOD msec of silence every SILENCE_PERIOD msec
            sending_silence: false,
            remote_ip: remote_ip_addr,
            wav_hdr: if !use_wave_format {
                Vec::new()
            } else {
                create_wav_hdr(sample_rate, bits_per_sample)
            },
            use_wave_format,
            bits_per_sample,
        }
    }

    // create a 250 msec silence
    pub fn create_silence(&mut self, sample_rate: u32) {
        const DIVISOR: u32 = 1000 / SILENCE_PERIOD;
        let size = ((sample_rate * 2) / DIVISOR) as usize;
        self.silence = Vec::with_capacity(size);
        self.silence.resize(size, 0f32);
        // Hack for Bubble with Chromecast/Nest
        //self.fifo.extend(self.silence.clone());
    }

    pub fn write(&self, samples: &[f32]) {
        self.s.send(samples.to_vec()).unwrap();
    }
}

impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut time_out = if self.sending_silence {
            self.silence_period
        } else {
            self.capture_timeout
        };
        let mut i = 0;
        if self.use_wave_format && !self.wav_hdr.is_empty() {
            i = self.wav_hdr.len();
            buf[..i].copy_from_slice(&self.wav_hdr);
            self.wav_hdr.clear();
        }
        while i < buf.len() - (self.bits_per_sample / 8) as usize {
            match self.fifo.pop_front() {
                Some(f32sample) => {
                    if self.bits_per_sample == 16 {
                        let sample = f32sample.to_i16();
                        let b = if self.use_wave_format {
                            sample.to_le_bytes()
                        } else {
                            sample.to_be_bytes()
                        };
                        buf[i] = b[0];
                        buf[i + 1] = b[1];
                        i += 2;
                    } else {
                        let sample = f32sample.to_i24();
                        if self.use_wave_format {
                            buf[i] = sample.b3;
                            buf[i + 1] = sample.b2;
                            buf[i + 2] = sample.b1;
                        } else {
                            buf[i] = sample.b1;
                            buf[i + 1] = sample.b2;
                            buf[i + 2] = sample.b3;
                        }
                        i += 3;
                    }
                }
                None => match self.r.recv_timeout(time_out) {
                    Ok(chunk) => {
                        self.fifo.extend(chunk);
                        self.sending_silence = false;
                        time_out = self.capture_timeout;
                    }
                    Err(_) => {
                        self.fifo.extend(self.silence.clone());
                        self.sending_silence = true;
                        time_out = self.silence_period;
                    }
                },
            }
        }
        Ok(i)
    }
}

// create an "infinite size" wav hdr
// note this may not work when streaming to a "libsndfile" based renderer
// as libsndfile insists on a seekable WAV file depending on the open mode used
fn create_wav_hdr(sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
    let mut hdr = [0u8; 44];
    let channels: u16 = 2;
    let bytes_per_sample: u16 = bits_per_sample / 8;
    let block_align: u16 = channels * bytes_per_sample;
    let byte_rate: u32 = sample_rate * block_align as u32;
    hdr[0..4].copy_from_slice(b"RIFF"); //ChunkId, little endian WAV
    let subchunksize: u32 = std::u32::MAX; // "infinite" data chunksize signal value
    let chunksize: u32 = subchunksize; // "infinite" RIFF chunksize signal value
    hdr[4..8].copy_from_slice(&chunksize.to_le_bytes()); // ChunkSize
    hdr[8..12].copy_from_slice(b"WAVE"); // File Format
    hdr[12..16].copy_from_slice(b"fmt "); // SubChunk = Format
    hdr[16..20].copy_from_slice(&16u32.to_le_bytes()); // SubChunk1Size for PCM
    hdr[20..22].copy_from_slice(&1u16.to_le_bytes()); // AudioFormat: uncompressed PCM
    hdr[22..24].copy_from_slice(&channels.to_le_bytes()); // numchannels 2
    hdr[24..28].copy_from_slice(&sample_rate.to_le_bytes()); // SampleRate
    hdr[28..32].copy_from_slice(&byte_rate.to_le_bytes()); // ByteRate (Bps)
    hdr[32..34].copy_from_slice(&block_align.to_le_bytes()); // BlockAlign
    hdr[34..36].copy_from_slice(&bits_per_sample.to_le_bytes()); // BitsPerSample
    hdr[36..40].copy_from_slice(b"data"); // SubChunk2Id
    hdr[40..44].copy_from_slice(&subchunksize.to_le_bytes()); // SubChunk2Size
    debug!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
    hdr.to_vec()
}

#[cfg(test)]
mod tests {
    use crate::utils::rwstream::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    #[test]

    fn test_wav_hdr() {
        let hdr = create_wav_hdr(44100, 24);
        eprintln!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
        let hdr = create_wav_hdr(44100, 16);
        eprintln!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
    }

    #[test]
    fn test_silence() {
        const SAMPLE_RATE: u32 = 44100;
        let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
        let mut cs =
            ChannelStream::new(tx, rx, "192.168.0.254".to_string(), false, SAMPLE_RATE, 16);
        cs.create_silence(SAMPLE_RATE);
        assert_eq!(
            cs.silence.len(),
            ((SAMPLE_RATE * 2) / (1000 / SILENCE_PERIOD)) as usize
        );
        /*
        let mut i = 0;
        for sample in cs.silence {
            if i == 15 {
                eprint!("{}\r\n", sample);
                i = 0;
            } else {
                eprint!("{} ", sample);
                i += 1;
            }
        }
        */
    }

    /*
    // near-silent 440 Hz tone (attenuation 32)
    use std::f64::consts::PI;
    self.silence = Vec::with_capacity((sample_rate * 2) as usize);
    let incr = (2.0 * PI) / (sample_rate as f64 / 440.0);
    let min_value: f64 = 0.0;
    let max_value: f64 = 2.0 * PI;
    let mut i = 0;
    loop {
        let mut input_value = min_value;
        while input_value <= max_value {
            let sample = (32.0 * input_value.sin()) as i16;
            self.silence.push(sample); // left channel
            self.silence.push(sample); // right channel
            input_value += incr;
            i += 1;
            if i == sample_rate {
                return;
            }
        }
    }
    */
}
