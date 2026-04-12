//! HTTP query-string parsing for streaming requests.
//!
//! [`StreamingParams`] is populated from the URL path and optional `?bd=&ss=` query
//! parameters to control bit depth and stream size per connection.

use crate::enums::streaming::{BitDepth, StreamSize, StreamingFormat};
use faup_rs::Url;
use std::str::FromStr;

const VALID_URLS: [&str; 5] = [
    "/stream/swyh.wav",
    "/stream/swyh.raw",
    "/stream/swyh.lpcm",
    "/stream/swyh.flac",
    "/stream/swyh.rf64",
];

/// streaming parameters extracted from the streaming url
#[derive(Debug, Clone)]
pub struct StreamingParams {
    pub path: Option<String>,
    pub bd: Option<BitDepth>,
    pub ss: Option<StreamSize>,
    pub fmt: Option<StreamingFormat>,
}

impl StreamingParams {
    #[must_use]
    /// build streaming parameters from the url
    pub fn from_url(url: &str) -> StreamingParams {
        const PATH_PREFIX: &str = "/stream/swyh.";
        const PATH_PREFIX_LEN: usize = PATH_PREFIX.len();

        let mut params = StreamingParams {
            path: None,
            bd: None,
            ss: None,
            fmt: None,
        };
        // parse url, check path and querystring
        if let Ok(parsed_url) = Url::parse(url)
            && let Some(path) = parsed_url.path()
        {
            // validate path
            let lc_path = path.to_lowercase();
            if VALID_URLS.contains(&lc_path.as_str()) {
                params.path = Some(lc_path.clone());
                params.fmt = lc_path
                    .get(PATH_PREFIX_LEN..)
                    .and_then(|ext| StreamingFormat::from_str(ext).ok());
            }
            // validate query string if present
            if params.fmt.is_some() && parsed_url.query().is_some() {
                // parse key=value pairs from the querystring
                // extract bd (bit depth) and ss (streamsize) if found
                let query_string = parsed_url
                    .query()
                    .expect("querystring detected but not found.");
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
                            "bd" => {
                                params.bd = Some(BitDepth::from_str(v).unwrap_or(BitDepth::Bits16))
                            }
                            "ss" => {
                                params.ss =
                                    Some(StreamSize::from_str(v).unwrap_or(StreamSize::NoneChunked))
                            }
                            _ => (),
                        });
                }
            }
        }
        // return params
        params
    }
}

#[cfg(test)]
mod tests {
    use crate::server::query_params::*;
    #[test]
    fn test_parse() {
        let sp = StreamingParams::from_url("http://swyh.local/stream/Swyh.wav?bd=24&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Wav));
        let sp = StreamingParams::from_url("http://swyh.local/stream/Swyh.wav?bd=25&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits16));
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Wav));
        let sp = StreamingParams::from_url("http://swyh.local/stream/swyh.Flac?ss=u32maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.flac".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Flac));
        let sp =
            StreamingParams::from_url("http://swyh.local/stream/swyh.rf64?bd=24&ss=u32maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.rf64".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, Some(StreamSize::U32maxChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Rf64));
        let sp =
            StreamingParams::from_url("http://swyh.local/stream/swyh.rf64?bd=24&ss=u3maxchunked");
        assert_eq!(sp.path, Some("/stream/swyh.rf64".to_string()));
        assert_eq!(sp.bd, Some(BitDepth::Bits24));
        assert_eq!(sp.ss, Some(StreamSize::NoneChunked));
        assert_eq!(sp.fmt, Some(StreamingFormat::Rf64));
        let sp = StreamingParams::from_url("http://swyh.local/stream/swyh.RAW");
        assert_eq!(sp.path, Some("/stream/swyh.raw".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Lpcm));
        let sp = StreamingParams::from_url("http://swyh.local/stream/swyh.Lpcm");
        assert_eq!(sp.path, Some("/stream/swyh.lpcm".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Lpcm));
        let sp = StreamingParams::from_url("http://swyh.local/stream/swyh.waf?");
        assert_eq!(sp.path, None);
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, None);
        let sp = StreamingParams::from_url("http://swyh.local/stream/swyh.wav?&");
        assert_eq!(sp.path, Some("/stream/swyh.wav".to_string()));
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, Some(StreamingFormat::Wav));
        let sp =
            StreamingParams::from_url("http://swyh.local/stream/swyh.rf65?bd=24&ss=u32maxchunked");
        assert_eq!(sp.path, None);
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, None);
        let sp = StreamingParams::from_url("stream/swyh.wav");
        assert_eq!(sp.path, None);
        assert_eq!(sp.bd, None);
        assert_eq!(sp.ss, None);
        assert_eq!(sp.fmt, None);
    }
}
