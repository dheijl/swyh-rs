#[derive(Debug, Eq, PartialEq)]
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
    #[inline]
    fn to_i24(&self) -> I24 {
        let mut sample = *self;
        if sample > 1.0 {
            sample = 1.0;
        } else if sample < -1.0 {
            sample = -1.0;
        }
        let tmp_i32 = {
            if sample >= 0.0 {
                ((sample as f64 * i32::MAX as f64) + 0.5) as i32
            } else {
                ((-sample as f64 * i32::MIN as f64) - 0.5) as i32
            }
        };
        let [a, b, c, _d] = tmp_i32.to_be_bytes();
        I24 {
            b1: a,
            b2: b,
            b3: c,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::i24::*;

    fn to_f32_to_i24(s: i32) -> I24 {
        eprintln!("i32_sample: {} {:0x}", s, s);
        let f32_sample: f32 = {
            if s < 0 {
                (s as f64 / -(i32::MIN as f64)) as f32
            } else {
                (s as f64 / i32::MAX as f64) as f32
            }
        };
        //(s as f32 / i32::MAX as f32) as f32;
        eprintln!("f32_sample: {}", f32_sample);
        let i24_sample = f32_sample.to_i24();
        eprintln!("i24_sample: {:?}", i24_sample);
        i24_sample
    }

    fn check_i24(s: I24, check: [u8; 3]) {
        assert_eq!(s.b1, check[0], "f32: msb1 fails");
        assert_eq!(s.b2, check[1], "f32: lsb2 fails");
        assert_eq!(s.b3, check[2], "f32: lsb1 fails");
    }

    #[test]
    fn test_i24() {
        let i24_sample = to_f32_to_i24(0x0a0b0c00);
        check_i24(i24_sample, [0x0a, 0x0b, 0x0c]);
        let i24_sample = to_f32_to_i24(0x0a0b0d00);
        check_i24(i24_sample, [0x0a, 0x0b, 0x0d]);
        let i24_sample = to_f32_to_i24(0x0a0b0e00);
        check_i24(i24_sample, [0x0a, 0x0b, 0x0e]);
        let i24_sample = to_f32_to_i24(0x0a0b0f00);
        check_i24(i24_sample, [0x0a, 0x0b, 0x0f]);
        let i24_sample = to_f32_to_i24(0 - 0x0a0b0c00); // F5F4F400
        check_i24(i24_sample, [0xF5, 0xF4, 0xF4]);
    }
}
