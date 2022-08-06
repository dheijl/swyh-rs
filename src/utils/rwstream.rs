use crate::StreamingFormat;
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
    silence_period: Duration,
    sending_silence: bool,
    wav_hdr: Vec<u8>,
    use_wave_format: bool,
    bits_per_sample: u16,
    flac_channel: Option<FlacChannel>,
}

const CAPTURE_TIMEOUT: u64 = 2; // seconds
const SILENCE_PERIOD: u64 = 250; // milliseconds

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
        let chs = ChannelStream {
            s: tx,
            r: rx,
            fifo: VecDeque::with_capacity(16384),
            flac_fifo: VecDeque::with_capacity(16384),
            silence: get_silence_buffer(sample_rate),
            capture_timeout: Duration::new(CAPTURE_TIMEOUT, 0), // silence kicks in after CAPTURE_TIMEOUT seconds
            silence_period: Duration::from_millis(SILENCE_PERIOD), // send SILENCE_PERIOD msec of silence every SILENCE_PERIOD msec
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
        if self.flac_channel.is_some() {
            self.flac_channel.as_ref().unwrap().run();
        }
    }

    // stop the flac encoder thread
    pub fn stop_flac_encoder(&self) {
        if self.flac_channel.is_some() {
            self.flac_channel.as_ref().unwrap().stop();
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
            let bytes_per_sample = (self.bits_per_sample / 8) as usize;
            while i < buf.len() - bytes_per_sample {
                if let Some(f32sample) = self.fifo.pop_front() {
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
                } else if let Ok(chunk) = self.r.recv_timeout(time_out) {
                    self.fifo.extend(chunk);
                    self.sending_silence = false;
                    time_out = self.capture_timeout;
                } else {
                    self.fifo.extend(self.silence.clone());
                    self.sending_silence = true;
                    time_out = self.silence_period;
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

pub fn get_silence_buffer(sample_rate: u32) -> Vec<f32> {
    const DIVISOR: u64 = 1000 / SILENCE_PERIOD;
    let size = ((sample_rate * 2) / DIVISOR as u32) as usize;
    let mut silence = Vec::with_capacity(size);
    silence.resize(size, 0f32);
    silence
}

#[cfg(test)]
mod tests {
    use crate::utils::rwstream::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
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
        let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
        let mut cs = ChannelStream::new(
            tx,
            rx,
            "192.168.0.254".to_string(),
            false,
            SAMPLE_RATE,
            16,
            None,
        );
        cs.create_silence(SAMPLE_RATE);
        assert_eq!(
            cs.silence.len(),
            ((SAMPLE_RATE * 2) / (1000 / SILENCE_PERIOD)) as usize
        );
    }
}
