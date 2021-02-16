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
use crossbeam_channel::{Receiver, Sender};
use log::{debug};
use std::collections::VecDeque;
use std::io::Read;
use std::io::Result as IoResult;
use std::time::Duration;

/// Channelstream - used to transport the samples from the wave_reader to the http output wav stream
pub struct ChannelStream {
    pub s: Sender<Vec<i16>>,
    pub r: Receiver<Vec<i16>>,
    pub remote_ip: String,
    fifo: VecDeque<i16>,
    silence: Vec<i16>,
    capture_timeout: Duration,
    silence_period: Duration,
    sending_silence: bool,
    wav_hdr: Vec<u8>,
    use_wave_format: bool,
}

const CAPTURE_TIMEOUT: u32 = 2; // seconds
const SILENCE_PERIOD: u32 = 250; // milliseconds

impl ChannelStream {
    pub fn new(
        tx: Sender<Vec<i16>>,
        rx: Receiver<Vec<i16>>,
        remote_ip_addr: String,
        use_wave_format: bool,
        sample_rate: u32,
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
                create_wav_hdr(sample_rate)
            },
            use_wave_format,
        }
    }

    // create a 250 msec silence
    pub fn create_silence(&mut self, sample_rate: u32) {
        const DIVISOR: u32 = 1000 / SILENCE_PERIOD;
        let size = ((sample_rate * 2) / DIVISOR) as usize;
        self.silence = Vec::with_capacity(size);
        self.silence.resize(size, 0i16);
        // Hack for Bubble with Chromecast/Nest
        self.fifo.extend(self.silence.clone());
    }

    pub fn write(&self, samples: &[i16]) {
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
        if !self.use_wave_format {
            while i < buf.len() - 2 {
                match self.fifo.pop_front() {
                    Some(sample) => {
                        let b = sample.to_be_bytes(); // audio/l16 = signed big endian format
                        buf[i] = b[0];
                        buf[i + 1] = b[1];
                        i += 2;
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
        } else {
            if !self.wav_hdr.is_empty() {
                debug!("Adding 'hound' WAV header for 16 bit PCM big endian format of infinite length");
                // should never happen as io::copy uses 8 KB chunks (DEFAULT_BUFSIZE)
                assert!(
                    buf.len() > self.wav_hdr.len(),
                    "ChannelStream::Read(): the read buffer is smaller than the WAV header!!"
                );
                i = self.wav_hdr.len(); 
                buf[..i - 1].copy_from_slice(&self.wav_hdr[..i - 1]);
                self.wav_hdr.clear();
            }
            while i < buf.len() - 2 {
                match self.fifo.pop_front() {
                    Some(sample) => {
                        let b = sample.to_be_bytes(); // audio/wma header of hound = signed big endian format
                        buf[i] = b[0];
                        buf[i + 1] = b[1];
                        i += 2;
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
}

// create an "infinite size" wav hdr
fn create_wav_hdr(sample_rate: u32) -> Vec<u8> {
    let spec = hound::WavSpec {
        bits_per_sample: 16,
        channels: 2,
        sample_format: hound::SampleFormat::Int,
        sample_rate,
    };
    spec.into_header_for_infinite_file()
}

#[cfg(test)]
mod tests {
    use crate::utils::rwstream::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    #[test]

    fn test_silence() {
        const SAMPLE_RATE: u32 = 44100;
        let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = unbounded();
        let mut cs = ChannelStream::new(tx, rx, "192.168.0.254".to_string(), false, SAMPLE_RATE);
        cs.create_silence(SAMPLE_RATE);
        assert_eq!(
            cs.silence.len(),
            ((SAMPLE_RATE * 2) / (1000 / SILENCE_PERIOD)) as usize
        );
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
