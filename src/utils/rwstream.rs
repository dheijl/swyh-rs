use std::io::Read;
use std::io::Result as IoResult;
//use std::sync::mpsc::{Receiver, Sender};
use crossbeam_channel::{Receiver, Sender};

pub struct ChannelStream {
    pub s: Sender<i16>,
    pub r: Receiver<i16>,
}

impl ChannelStream {
    pub fn write(&self, samples: &[i16]) {
        for sam in samples {
            self.s.send(*sam).unwrap();
        }
    }
}

impl Read for ChannelStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut i = 0;
        while i < buf.len() - 2 {
            match self.r.recv() {
                Ok(sample) => {
                    buf[i] = ((sample >> 8) & 0xff) as u8;
                    buf[i + 1] = (sample & 0xff) as u8;
                    i = i + 2;
                }
                Err(_) => {
                    break;
                }
            }
        }
        Ok(i)
    }
}
