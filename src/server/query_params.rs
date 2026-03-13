use crate::enums::streaming::{BitDepth, StreamSize, StreamingFormat};
use std::str::FromStr;

const VALID_URLS: [&str; 5] = [
    "/stream/swyh.wav",
    "/stream/swyh.raw",
    "/stream/swyh.lpcm",
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
        const PATH_PREFIX: &str = "/stream/swyh.";
        const PATH_PREFIX_LEN: usize = PATH_PREFIX.len();

        let mut params = StreamingParams {
            path: None,
            bd: None,
            ss: None,
            fmt: None,
        };
        let parts: Vec<&str> = url.split('?').collect();
        let path = parts[0];
        if path.is_empty() {
            return params;
        }
        let lc_path = path.to_lowercase();
        if VALID_URLS.contains(&lc_path.as_str()) {
            params.path = Some(lc_path.clone());
            params.fmt = lc_path
                .get(PATH_PREFIX_LEN..)
                .and_then(|ext| StreamingFormat::from_str(ext).ok());
        }
        if params.fmt.is_none() || parts.len() < 2 {
            return params;
        }
        // parse key=value pairs from querystring if present
        // extract bd (bit depth) and ss (streamsize) if found
        let query_string = parts[1];
        if !query_string.is_empty() {
            query_string
                .split('&')
                .filter_map(|part| {
                    let mut kv_pair = part.splitn(2, '=');
                    match (kv_pair.next(), kv_pair.next()) {
                        (Some(k), Some(v)) => Some((k, v)),
                        _ => None,
                    }
                })
                .for_each(|(k, v)| match k {
                    "bd" => params.bd = Some(BitDepth::from_str(v).unwrap_or(BitDepth::Bits16)),
                    "ss" => {
                        params.ss = Some(StreamSize::from_str(v).unwrap_or(StreamSize::NoneChunked))
                    }
                    _ => (),
                });
        }
        params
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
        let sp = StreamingParams::from_query_string("/stream/Swyh.wav?bd=25&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits16));
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
        let sp = StreamingParams::from_query_string("/stream/swyh.rf64?bd=24&ss=u3maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.rf64".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, Some(StreamSize::NoneChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Rf64));
        let sp = StreamingParams::from_query_string("/stream/swyh.RAW");
        assert_eq!(sp.path, Some("/stream/swyh.raw".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Lpcm));
        let sp = StreamingParams::from_query_string("/stream/swyh.Lpcm");
        assert_eq!(sp.path, Some("/stream/swyh.lpcm".to_string()));
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
