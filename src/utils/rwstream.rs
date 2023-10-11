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
use crate::{enums::streaming::StreamingFormat, globals::statics::CONFIG, utils::i24::I24Sample};
use crossbeam_channel::{Receiver, Sender};
use dasp_sample::Sample;
use log::debug;
use rand::{distributions::Uniform, rngs::StdRng, Rng, SeedableRng};
use std::{
    collections::VecDeque,
    io::{Read, Result as IoResult},
    time::Duration,
};

use super::flacstream::FlacChannel;

/// Channelstream - used to transport the f32 samples from the wave_reader
/// to the http output stream in LPCM/WAV/FLAC format
#[derive(Clone)]
pub struct ChannelStream {
    pub s: Sender<Vec<f32>>,
    pub r: Receiver<Vec<f32>>,
    pub remote_ip: String,
    pub streaming_format: StreamingFormat,
    fifo: VecDeque<f32>,
    flac_fifo: VecDeque<u8>,
    silence: Vec<f32>,
    capture_timeout: Duration,
    sending_silence: bool,
    wav_hdr: Vec<u8>,
    use_wave_format: bool,
    bits_per_sample: u16,
    flac_channel: Option<FlacChannel>,
}

impl ChannelStream {
    pub fn new(
        tx: Sender<Vec<f32>>,
        rx: Receiver<Vec<f32>>,
        remote_ip_addr: String,
        use_wave_format: bool,
        sample_rate: u32,
        bits_per_sample: u16,
        streaming_format: StreamingFormat,
    ) -> ChannelStream {
        let flac_channel = if streaming_format == StreamingFormat::Flac {
            Some(FlacChannel::new(
                rx.clone(),
                sample_rate,
                bits_per_sample as u32,
                2,
            ))
        } else {
            None
        };
        let capture_timout = CONFIG.read().capture_timeout.unwrap() as u64;
        let chs = ChannelStream {
            s: tx,
            r: rx,
            fifo: VecDeque::with_capacity(16384),
            flac_fifo: VecDeque::with_capacity(16384),
            silence: get_silence_buffer(sample_rate, capture_timout / 4),
            capture_timeout: Duration::from_millis(capture_timout), // silence kicks in after CAPTURE_TIMEOUT seconds
            sending_silence: false,
            remote_ip: remote_ip_addr,
            wav_hdr: if !use_wave_format {
                Vec::new()
            } else {
                create_wav_hdr(sample_rate, bits_per_sample)
            },
            use_wave_format,
            bits_per_sample,
            streaming_format,
            flac_channel,
        };
        if chs.streaming_format == StreamingFormat::Flac {
            chs.start_flac_encoder();
        }
        chs
    }

    // the flac encoder runs in a seperate thread
    fn start_flac_encoder(&self) {
        if let Some(flac_channel) = &self.flac_channel {
            flac_channel.run();
        }
    }

    // stop the flac encoder thread
    pub fn stop_flac_encoder(&self) {
        if let Some(flac_channel) = &self.flac_channel {
            flac_channel.stop();
        }
    }

    // called by the wave_reader to write the f32 samples to the input channel
    pub fn write(&self, samples: &[f32]) {
        self.s.send(samples.to_vec()).unwrap();
    }
}

/// implement the Read trait for the HTTP writer
///
/// for LPCM/WAV the f32 samples are read from the f32 input channel and pushed
/// on the fifo VecDeque that is then read for conversion to LPCM and transmission
///
/// for FLAC the f32 samples have already been encoded to FLAC and written to the
/// flac_out channel of the FlacChannel encoder.
/// the flac_in channel of the FlacChannel is read here and pushed on the flac_fifo VecDeque
/// for transmission  
impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.flac_channel.is_none() {
            // naked LPCM or WAV LPCM
            let time_out = self.capture_timeout;
            let mut i = 0;
            if self.use_wave_format && !self.wav_hdr.is_empty() {
                i = self.wav_hdr.len();
                buf[..i].copy_from_slice(&self.wav_hdr);
                self.wav_hdr.clear();
            }
            let bytes_per_sample = (self.bits_per_sample / 8) as usize;
            while i < buf.len() - bytes_per_sample {
                if let Some(f32sample) = self.fifo.pop_front() {
                    if self.bits_per_sample == 16 {
                        //let sample: i16 = f32sample.to_i16();
                        let sample = i16::from_sample(f32sample);
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
                } else if let Ok(chunk) = self.r.recv_timeout(time_out) {
                    self.fifo.extend(chunk);
                    self.sending_silence = false;
                } else {
                    self.fifo.extend(self.silence.clone());
                    self.sending_silence = true;
                }
            }
            Ok(i)
        } else {
            // FLAC
            let flac_in = self.flac_channel.as_ref().unwrap().flac_in.clone();
            let mut i: usize = 0;
            while i < buf.len() {
                if let Some(flacbyte) = self.flac_fifo.pop_front() {
                    buf[i] = flacbyte;
                    i += 1;
                } else if let Ok(chunk) = flac_in.recv() {
                    self.flac_fifo.extend(chunk);
                }
            }
            Ok(i)
        }
    }
}

// create an "infinite size" wav hdr
// note this may not work when streaming to an older "libsndfile" based renderer
// as it insists on a seekable WAV file depending on the open mode used
/*
PCM Data (s16le)
Field	        Length	Contents
ckID	        4	    Chunk ID: 'RIFF'
cksize	        4	    Chunk size: 4 + 24 + (8 + M*Nc*Ns + (0 or 1)
WAVEID	        4	    WAVE ID: 'WAVE'
ckID	        4	    Chunk ID: 'fmt '
cksize	        4	    Chunk size: 16
wFormatTag	    2	    WAVE_FORMAT_PCM (0001)
nChannels	    2	    Nc
nSamplesPerSec	4	    F
nAvgBytesPerSec	4	    F*M*Nc
nBlockAlign	    2	    M*Nc
wBitsPerSample	2	    rounds up to 8*M
ckID	        4	    Chunk ID: 'data'
cksize	        4	    Chunk size: M*Nc*Ns
sampled data	M*Nc*Ns	Nc*Ns channel-interleaved M-byte samples
pad byte	    0 or 1	Padding byte if M*Nc*Ns is odd
*/
fn create_wav_hdr(sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
    let mut hdr = [0u8; 44];
    let channels: u16 = 2;
    let bytes_per_sample: u16 = bits_per_sample / 8;
    let block_align: u16 = channels * bytes_per_sample;
    let byte_rate: u32 = sample_rate * block_align as u32;
    hdr[0..4].copy_from_slice(b"RIFF"); //ChunkId, little endian WAV
    let chunksize: u32 = 4294967284; // max RIFF chunksize
    let subchunksize: u32 = 4294967248; // max data chunksize signal value
    hdr[4..8].copy_from_slice(&chunksize.to_le_bytes()); // RIFF ChunkSize
    hdr[8..12].copy_from_slice(b"WAVE"); // File Format
    hdr[12..16].copy_from_slice(b"fmt "); // SubChunk = Format
    hdr[16..20].copy_from_slice(&16u32.to_le_bytes()); // fmt chunksize for PCM
    hdr[20..22].copy_from_slice(&1u16.to_le_bytes()); // AudioFormat: uncompressed PCM
    hdr[22..24].copy_from_slice(&channels.to_le_bytes()); // numchannels 2
    hdr[24..28].copy_from_slice(&sample_rate.to_le_bytes()); // SampleRate
    hdr[28..32].copy_from_slice(&byte_rate.to_le_bytes()); // ByteRate (Bps)
    hdr[32..34].copy_from_slice(&block_align.to_le_bytes()); // BlockAlign
    hdr[34..36].copy_from_slice(&bits_per_sample.to_le_bytes()); // BitsPerSample
    hdr[36..40].copy_from_slice(b"data"); // SubChunk = "data"
    hdr[40..44].copy_from_slice(&subchunksize.to_le_bytes()); // data SubChunkSize
    debug!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
    hdr.to_vec()
}

//#[allow(dead_code)]
fn get_silence_buffer(sample_rate: u32, silence_period: u64) -> Vec<f32> {
    // silence_period is in msecs (capture_timeout / 4), sample rate is per second, 2 channels for stereo
    let size = ((sample_rate * 2 * silence_period as u32) / 1000) as usize;
    let mut silence = Vec::with_capacity(size);
    silence.resize(size, 0f32);
    silence
}

///
/// fille the pre-allocated noise buffer with a very faint white noise (-60db)
///
#[allow(dead_code)]
fn get_noise_buffer(sample_rate: u32, silence_period: u64) -> Vec<f32> {
    // create the random generator for the white noise
    let mut rng = StdRng::seed_from_u64(79);
    let size = ((sample_rate * 2 * silence_period as u32) / 1000) as usize;
    let mut noise = Vec::with_capacity(size);
    noise.resize(size, 0.0);
    let amplitude: f32 = 0.001;
    for sample in noise.iter_mut() {
        *sample = ((rng.sample(Uniform::new(0.0, 1.0)) * 2.0) - 1.0) * amplitude;
    }
    noise
}

#[cfg(test)]
mod tests {
    use crate::utils::rwstream::*;
    #[test]

    fn test_wav_hdr() {
        let _hdr = create_wav_hdr(44100, 24);
        //eprintln!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
        let _hdr = create_wav_hdr(44100, 16);
        //eprintln!("WAV Header (l={}): \r\n{:02x?}", hdr.len(), hdr);
    }

    #[test]
    fn test_silence() {
        const SAMPLE_RATE: u32 = 44100;
        let sb = get_silence_buffer(SAMPLE_RATE, 250);
        assert_eq!(sb.len(), ((SAMPLE_RATE * 2) as u64 / (1000 / 250)) as usize);
    }
}
