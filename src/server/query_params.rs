use crate::enums::streaming::{BitDepth, StreamSize};
use std::str::FromStr;

pub struct StreamingParams {
    bd: Option<BitDepth>,
    ss: Option<StreamSize>,
}

impl StreamingParams {
    pub fn from_query_string(url: &str) -> StreamingParams {
        let mut result = StreamingParams { bd: None, ss: None };
        if !url.contains('?') {
            return result;
        }
        let parts: Vec<&str> = url.split('?').collect();
        if parts.len() < 2 {
            return result;
        }
        let query_string = parts[1];
        if !query_string.is_empty() {
            let parts: Vec<&str> = query_string.split('&').collect();
            for p in parts {
                if !p.is_empty() {
                    let kv: Vec<&str> = p.split('=').collect();
                    if kv.len() == 2 {
                        let k = kv[0];
                        let v = kv[1];
                        match k {
                            "bd" => result.bd = Some(BitDepth::from_str(v).unwrap()),
                            "ss" => result.ss = Some(StreamSize::from_str(v).unwrap()),
                            _ => (),
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::server::query_params::*;
    #[test]
    fn test_parse() {
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?bd=24&");
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, None);
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?ss=u32maxchunked");
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?bd=24&ss=u32maxchunked");
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        let sp = StreamingParams::from_query_string("/stream/swyh.wav");
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?");
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?&");
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
    }
}
