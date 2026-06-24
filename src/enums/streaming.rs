//! Streaming-related enumerations and the [`StreamingContext`] helper struct.
//!
//! Covers [`StreamingFormat`] (LPCM/WAV/RF64/FLAC), [`BitDepth`], [`StreamSize`],
//! [`Endian`], and [`StreamingState`], plus [`StreamingContext`] which aggregates
//! all per-connection streaming parameters.

use ecow::EcoString;
use serde::{Deserialize, Serialize};
use std::{convert::From, fmt, str::FromStr};
use tiny_http::Request;

use crate::{
    globals::statics::get_config, renderers::rendercontrol::WavData,
    server::query_params::StreamingParams,
};

/// streaming state
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StreamingState {
    Started,
    Ended,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum StreamingFormat {
    Lpcm,
    Wav,
    Flac,
    Rf64,
}

impl fmt::Display for StreamingFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingFormat::Lpcm => f.write_str("lpcm"),
            StreamingFormat::Wav => f.write_str("wav"),
            StreamingFormat::Flac => f.write_str("flac"),
            StreamingFormat::Rf64 => f.write_str("rf64"),
        }
    }
}

impl FromStr for StreamingFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "lpcm" | "raw" => Ok(StreamingFormat::Lpcm),
            "wav" => Ok(StreamingFormat::Wav),
            "flac" => Ok(StreamingFormat::Flac),
            "rf64" => Ok(StreamingFormat::Rf64),
            _ => Err(()),
        }
    }
}

impl StreamingFormat {
    pub fn needs_wav_hdr(self) -> bool {
        self == StreamingFormat::Wav || self == StreamingFormat::Rf64
    }

    pub fn dlna_audio_string(self, bps: BitDepth) -> String {
        match self {
            StreamingFormat::Flac => "audio/FLAC".to_string(),
            StreamingFormat::Wav | StreamingFormat::Rf64 => "audio/wave;codec=1 (WAV)".to_string(),
            StreamingFormat::Lpcm => {
                if bps == BitDepth::Bits16 {
                    "audio/L16 (LPCM)".to_string()
                } else {
                    "audio/L24 (LPCM)".to_string()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum StreamSize {
    NoneChunked,
    U32maxChunked,
    U32maxNotChunked,
    U64maxChunked,
    U64maxNotChunked,
}

// streamsize/chunkthreshold pairs for tiny-http response.
// The "NotChunked" variants set content-length to MAX-1 and chunk-threshold to MAX
// so tiny-http never triggers chunked transfer (threshold must exceed content-length).
pub(crate) const NONECHUNKED: (Option<usize>, usize) = (None, 8192);
pub(crate) const U32MAXCHUNKED: (Option<usize>, usize) = (Some(u32::MAX as usize), 8192);
pub(crate) const U32MAXNOTCHUNKED: (Option<usize>, usize) =
    (Some((u32::MAX - 1) as usize), u32::MAX as usize);
pub(crate) const U64MAXCHUNKED: (Option<usize>, usize) = (Some(u64::MAX as usize), 8192);
pub(crate) const U64MAXNOTCHUNKED: (Option<usize>, usize) =
    (Some((u64::MAX - 1) as usize), u64::MAX as usize);

impl StreamSize {
    #[must_use]
    pub fn values(&self) -> (Option<usize>, usize) {
        match self {
            StreamSize::NoneChunked => NONECHUNKED,
            StreamSize::U32maxChunked => U32MAXCHUNKED,
            StreamSize::U32maxNotChunked => U32MAXNOTCHUNKED,
            StreamSize::U64maxChunked => U64MAXCHUNKED,
            StreamSize::U64maxNotChunked => U64MAXNOTCHUNKED,
        }
    }
}

impl fmt::Display for StreamSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamSize::NoneChunked => f.write_str("NoneChunked"),
            StreamSize::U32maxChunked => f.write_str("U32maxChunked"),
            StreamSize::U32maxNotChunked => f.write_str("U32maxNotChunked"),
            StreamSize::U64maxChunked => f.write_str("U64maxChunked"),
            StreamSize::U64maxNotChunked => f.write_str("U64maxNotChunked"),
        }
    }
}

impl FromStr for StreamSize {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "nonechunked" => Ok(StreamSize::NoneChunked),
            "u32maxchunked" => Ok(StreamSize::U32maxChunked),
            "u32maxnotchunked" => Ok(StreamSize::U32maxNotChunked),
            "u64maxchunked" => Ok(StreamSize::U64maxChunked),
            "u64maxnotchunked" => Ok(StreamSize::U64maxNotChunked),
            _ => Ok(StreamSize::NoneChunked),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum BitDepth {
    Bits24 = 24,
    Bits16 = 16,
}

impl BitDepth {
    /// Right-shift applied to an `i32` sample to extract the target bit depth.
    /// `i32 >> 8` yields 24 significant bits; `i32 >> 16` yields 16.
    pub fn shift_value(&self) -> u8 {
        match self {
            BitDepth::Bits24 => 8u8,
            BitDepth::Bits16 => 16u8,
        }
    }
}

impl From<u16> for BitDepth {
    fn from(bps: u16) -> Self {
        match bps {
            16 => BitDepth::Bits16,
            24 => BitDepth::Bits24,
            _ => BitDepth::Bits16,
        }
    }
}

impl fmt::Display for BitDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitDepth::Bits16 => f.write_str("16"),
            BitDepth::Bits24 => f.write_str("24"),
        }
    }
}

impl FromStr for BitDepth {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "16" => Ok(BitDepth::Bits16),
            "24" => Ok(BitDepth::Bits24),
            _ => Ok(BitDepth::Bits16),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum Endian {
    Little,
    Big,
}

/// All per-connection streaming parameters, fully initialised in one shot by [`StreamingContext::new`].
#[derive(Debug)]
pub struct StreamingContext {
    pub sample_rate: u32,
    pub sample_format: cpal::SampleFormat,
    pub bits_per_sample: BitDepth,
    pub streaming_format: StreamingFormat,
    pub buffering_delay_msec: u32,
    pub remote_addr: EcoString, // ip:port
    pub remote_ip: EcoString,   // ip only
    pub chunksize: usize,
    pub streamsize: Option<usize>,
    pub url: EcoString,
    pub use_dither: bool,
}

impl StreamingContext {
    /// Build a fully-initialised `StreamingContext` from config, a live request, and its parsed URL params.
    pub fn new(wd: WavData, rq: &Request, params: &StreamingParams) -> StreamingContext {
        let cfg = get_config();

        let url = EcoString::from(rq.url());
        let remote_addr = EcoString::from(rq.remote_addr().unwrap().to_string());
        let mut remote_ip = remote_addr.clone();
        if let Some(i) = remote_addr.find(':') {
            remote_ip.truncate(i);
        }

        let streaming_format = params
            .fmt
            .unwrap_or_else(|| cfg.streaming_format.unwrap_or(StreamingFormat::Flac));
        let bits_per_sample = params
            .bd
            .unwrap_or_else(|| BitDepth::from(cfg.bits_per_sample.unwrap_or(16)));

        let (streamsize, chunksize) =
            params
                .ss
                .map(|ss| ss.values())
                .unwrap_or_else(|| match streaming_format {
                    StreamingFormat::Lpcm => cfg.lpcm_stream_size.unwrap().values(),
                    StreamingFormat::Wav => cfg.wav_stream_size.unwrap().values(),
                    StreamingFormat::Rf64 => cfg.rf64_stream_size.unwrap().values(),
                    StreamingFormat::Flac => cfg.flac_stream_size.unwrap().values(),
                });

        StreamingContext {
            sample_rate: wd.sample_rate,
            sample_format: wd.sample_format,
            bits_per_sample,
            streaming_format,
            buffering_delay_msec: cfg.buffering_delay_msec.unwrap_or(0),
            remote_addr,
            remote_ip,
            chunksize,
            streamsize,
            url,
            use_dither: cfg.use_dither.unwrap_or(true),
        }
    }

    #[inline]
    pub fn needs_wav_hdr(&self) -> bool {
        self.streaming_format.needs_wav_hdr()
    }

    pub fn dlna_audio_string(&self) -> String {
        self.streaming_format
            .dlna_audio_string(self.bits_per_sample)
    }
}
