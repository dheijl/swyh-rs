use crate::enums::streaming::{BitDepth, StreamSize, StreamingFormat};
use std::str::FromStr;

const VALID_URLS: [&str; 4] = [
    "/stream/swyh.wav",
    "/stream/swyh.raw",
    "/stream/swyh.flac",
    "/stream/swyh.rf64",
];

#[derive(Debug, Clone)]
pub struct StreamingParams {
    pub path: Option<String>,
    pub bd: Option<BitDepth>,
    pub ss: Option<StreamSize>,
    pub fmt: Option<StreamingFormat>,
}

impl StreamingParams {
    #[must_use]
    pub fn from_query_string(url: &str) -> StreamingParams {
        let mut result = StreamingParams {
            path: None,
            bd: None,
            ss: None,
            fmt: None,
        };
        if !url.contains('/') {
            return result;
        }
        let parts: Vec<&str> = url.split('?').collect();
        if parts.is_empty() {
            return result;
        }
        let path = parts[0];
        if path.is_empty() {
            return result;
        }
        let lc_path = path.to_lowercase();
        if VALID_URLS.contains(&lc_path.as_str()) {
            result.path = Some(lc_path.clone());
        }
        let fmt = {
            if let Some(format_start) = lc_path.find("/stream/swyh.") {
                match lc_path.to_lowercase().get(format_start + 13..) {
                    Some("flac") => Some(StreamingFormat::Flac),
                    Some("wav") => Some(StreamingFormat::Wav),
                    Some("rf64") => Some(StreamingFormat::Rf64),
                    Some("raw") => Some(StreamingFormat::Lpcm),
                    None | Some(&_) => None,
                }
            } else {
                None
            }
        };
        result.fmt = fmt;
        if fmt.is_none() {
            return result;
        }
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
        let sp = StreamingParams::from_query_string("/stream/Swyh.wav?bd=24&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Wav));
        let sp = StreamingParams::from_query_string("/stream/swyh.Flac?ss=u32maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.flac".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Flac));
        let sp = StreamingParams::from_query_string("/stream/swyh.rf64?bd=24&ss=u32maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.rf64".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Rf64));
        let sp = StreamingParams::from_query_string("/stream/swyh.RAW");
        assert_eq!(sp.path, Some("/stream/swyh.raw".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Lpcm));
        let sp = StreamingParams::from_query_string("/stream/swyh.waf?");
        assert_eq!(sp.path, None);
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, None);
        let sp = StreamingParams::from_query_string("/stream/swyh.wav?&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Wav));
        let sp = StreamingParams::from_query_string("/stream/swyh.rf65?bd=24&ss=u32maxchunked");
        assert_eq!(sp.path, None);
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, None);
    }
}
