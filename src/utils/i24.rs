pub struct I24 {
    pub b1: u8,
    pub b2: u8,
    pub b3: u8,
}
#[allow(dead_code)]
impl I24 {
    const MAX: i32 = 8_388_607;
    const MIN: i32 = -8_388_608;
}
pub trait I24Sample: Copy + Clone {
    fn to_i24(&self) -> I24;
}
impl I24Sample for f32 {
    fn to_i24(&self) -> I24 {
        let mut s32 = *self;
        if s32 > 1.0 {
            s32 = 1.0;
        } else {
            if s32 < -1.0 {
                s32 = -1.0;
            }
        }
        let tmp = ((s32 as f64 * I24::MAX as f64) as i32).to_be_bytes();
        I24 {
            b1: tmp[1],
            b2: tmp[2],
            b3: tmp[3],
        }
    }
}
impl I24Sample for i32 {
    fn to_i24(&self) -> I24 {
        let tmp = (*self).to_be_bytes();
        I24 {
            b1: tmp[1],
            b2: tmp[2],
            b3: tmp[3],
        }
    }
}
