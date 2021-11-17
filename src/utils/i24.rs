#[derive(Debug, PartialEq)]
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
        let mut sample = *self;
        if sample > 1.0 {
            sample = 1.0;
        } else if sample < -1.0 {
            sample = -1.0;
        }
        let tmp = (((sample as f64 * i32::MAX as f64) + 0.5) as i32).to_be_bytes();
        I24 {
            b1: tmp[0],
            b2: tmp[1],
            b3: tmp[2],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::i24::*;

    #[test]
    fn test_i24() {
        let i32_sample: i32 = 0x0a0b0c00 as i32;
        let f32_sample: f32 = (i32_sample as f32 / i32::MAX as f32) as f32;
        let i24_sample = f32_sample.to_i24();
        assert_eq!(i24_sample.b1, 0x0a, "f32: msb1 fails");
        assert_eq!(i24_sample.b2, 0x0b, "f32: lsb2 fails");
        assert_eq!(i24_sample.b3, 0x0c, "f32: lsb1 fails");
        let i32_sample: i32 = 0x0a0b0d00 as i32;
        let f32_sample: f32 = (i32_sample as f32 / i32::MAX as f32) as f32;
        let i24_sample = f32_sample.to_i24();
        assert_eq!(i24_sample.b1, 0x0a, "f32: msb1 fails");
        assert_eq!(i24_sample.b2, 0x0b, "f32: lsb2 fails");
        assert_eq!(i24_sample.b3, 0x0d, "f32: lsb1 fails");
        let i32_sample: i32 = 0x0a0b0e00 as i32;
        let f32_sample: f32 = (i32_sample as f32 / i32::MAX as f32) as f32;
        let i24_sample = f32_sample.to_i24();
        assert_eq!(i24_sample.b1, 0x0a, "f32: msb1 fails");
        assert_eq!(i24_sample.b2, 0x0b, "f32: lsb2 fails");
        assert_eq!(i24_sample.b3, 0x0e, "f32: lsb1 fails");
        let i32_sample: i32 = 0x0a0b0f00 as i32;
        let f32_sample: f32 = (i32_sample as f32 / i32::MAX as f32) as f32;
        let i24_sample = f32_sample.to_i24();
        assert_eq!(i24_sample.b1, 0x0a, "f32: msb1 fails");
        assert_eq!(i24_sample.b2, 0x0b, "f32: lsb2 fails");
        assert_eq!(i24_sample.b3, 0x0f, "f32: lsb1 fails");
    }
}
