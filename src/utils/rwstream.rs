///
/// rwstream.rs
///
/// ChannelStream: the write method sends the received samples on the CrssBeam channel
/// for the Read trait to read them back
///
/// the Read trait implementation is used by the HTTP response to send the response wav stream
/// to the media Renderer
///
/*
MIT License

Copyright (c) 2020 dheijl

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::io::Read;
use std::io::Result as IoResult;

/// Channelstream - used to transport the samples from the wave_reader to the http output wav stream
pub struct ChannelStream {
    pub s: Sender<Vec<i16>>,
    pub r: Receiver<Vec<i16>>,
    fifo: VecDeque<i16>,
}

impl ChannelStream {
    pub fn new(tx: Sender<Vec<i16>>, rx: Receiver<Vec<i16>>) -> ChannelStream {
        ChannelStream {
            s: tx,
            r: rx,
            fifo: VecDeque::with_capacity(16384),
        }
    }
    pub fn write(&self, samples: &[i16]) {
        let mut chunk: Vec<i16> = Vec::new();
        chunk.resize(samples.len(), 0);
        chunk.copy_from_slice(samples);
        self.s.send(chunk).unwrap();
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
                    i = i + 2;
                }
                None => match self.r.recv() {
                    Ok(chunk) => {
                        let mut new_samples: VecDeque<i16> = VecDeque::from(chunk);
                        self.fifo.append(&mut new_samples);
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                },
            }
        }
        Ok(i)
    }
}
