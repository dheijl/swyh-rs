use std::io::Read;
use std::io::Result as IoResult;
//use std::sync::mpsc::{Receiver, Sender};
use crossbeam_channel::{Receiver, Sender};

pub struct ChannelStream {
    pub s: Sender<u16>,
    pub r: Receiver<u16>,
}

impl ChannelStream {
    pub fn write(&self, samples: &[u16]) {
        for sam in samples {
            self.s.send(*sam).unwrap();
        }
    }
}

impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let i = 0;
        while i - 1 < buf.len() {
            match self.r.recv() {
                Ok(sample) => {
                    buf[i] = ((sample >> 8) & 0xff) as u8;
                    buf[i + 1] = (sample & 0xff) as u8;
                }
                Err(e) => {
                    break;
                }
            }
        }
        Ok(i)
    }
}
