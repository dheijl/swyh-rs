#[allow(dead_code)]
pub struct I24 {
    b1: u8,
    b2: u8,
    b3: u8,
}
impl I24 {
    const MAX: i32 = 8_388_607;
    const MIN: i32 = -8_388_608;
}
pub trait I24Sample: Copy + Clone {
    fn to_i24(&self) -> I24;
}
impl I24Sample for f32 {
    fn to_i24(&self) -> I24 {
        let tmp: i32;
        if *self > 0.0 {
            tmp = (*self * I24::MAX as f32) as i32;
        } else {
            tmp = (-*self * I24::MIN as f32) as i32;
        }
        I24 {
            b1: (tmp & 0xff0000 >> 16) as u8,
            b2: (tmp & 0xff00 >> 8) as u8,
            b3: (tmp & 0xff) as u8,
        }
    }
}
impl I24Sample for i32 {
    fn to_i24(&self) -> I24 {
        let tmp = *self;
        I24 {
            b1: (tmp & 0xff0000 >> 16) as u8,
            b2: (tmp & 0xff00 >> 8) as u8,
            b3: (tmp & 0xff) as u8,
        }
    }
}
/*
let mut i24_samples: Vec<I24> = Vec::with_capacity(samples.len());
i24_samples.extend(samples.iter().map(|x| x.to_i24()));
*/
