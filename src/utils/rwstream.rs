mod rwstream {

    use crossbeam_channel::*;
    use std::io::Read;
    use std::io::Result as IoResult;

    pub struct ChannelStream {
        r: Receiver<u16>,
        s: Sender<u16>,
    }

    impl ChannelStream {
        pub fn new(r: Receiver<u16>, s: Sender<u16>) -> ChannelStream {
            ChannelStream { r, s }
        }
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
}
