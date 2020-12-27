use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
///
/// rwstream.rs
///
/// ChannelStream: the write method sends the received samples on the CrssBeam channel
/// for the Read trait to read them back
///
/// the Read trait implementation is used by the HTTP response to send the response wav stream
/// to the media Renderer
///
use std::f64::consts::PI;
use std::io::Read;
use std::io::Result as IoResult;
use std::time::Duration;

/// Channelstream - used to transport the samples from the wave_reader to the http output wav stream
pub struct ChannelStream {
    pub s: Sender<Vec<i16>>,
    pub r: Receiver<Vec<i16>>,
    fifo: VecDeque<i16>,
    silence: Vec<i16>,
    read_timeout: Duration,
}

impl ChannelStream {
    pub fn new(tx: Sender<Vec<i16>>, rx: Receiver<Vec<i16>>) -> ChannelStream {
        ChannelStream {
            s: tx,
            r: rx,
            fifo: VecDeque::with_capacity(16384),
            silence: Vec::new(),
            read_timeout: Duration::new(10, 0),
        }
    }

    // create a 1-second near-silent 440 Hz tone (attenuation 32)
    pub fn create_near_silence(&mut self, sample_rate: u32) {
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
    }

    pub fn write(&self, samples: &[i16]) {
        self.s.send(samples.to_vec()).unwrap();
    }
}

impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut i = 0;
        while i < buf.len() - 2 {
            match self.fifo.pop_front() {
                Some(sample) => {
                    buf[i] = ((sample >> 8) & 0xff) as u8;
                    buf[i + 1] = (sample & 0xff) as u8;
                    i += 2;
                }
                None => match self.r.recv_timeout(self.read_timeout) {
                    Ok(chunk) => {
                        self.fifo.extend(chunk);
                    }
                    Err(_) => {
                        self.fifo.extend(self.silence.clone());
                    }
                },
            }
        }
        Ok(i)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::rwstream::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    #[test]

    fn test_silence() {
        let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = unbounded();
        let mut cs = ChannelStream::new(tx, rx);
        cs.create_near_silence(44100);
        assert_eq!(cs.silence.len(), 44100 * 2);
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
}
