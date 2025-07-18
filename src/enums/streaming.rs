use serde::{Deserialize, Serialize};
use std::{convert::From, fmt, str::FromStr};

use crate::globals::statics::get_config;

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
            StreamingFormat::Lpcm => write!(f, "Lpcm"),
            StreamingFormat::Wav => write!(f, "Wav"),
            StreamingFormat::Flac => write!(f, "Flac"),
            StreamingFormat::Rf64 => write!(f, "Rf64"),
        }
    }
}

impl FromStr for StreamingFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "lpcm" => Ok(StreamingFormat::Lpcm),
            "wav" => Ok(StreamingFormat::Wav),
            "flac" => Ok(StreamingFormat::Flac),
            "rf64" => Ok(StreamingFormat::Rf64),
            _ => Err(()),
        }
    }
}

impl StreamingFormat {
    pub fn dlna_string(self, bps: BitDepth) -> String {
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
    pub fn get_streaming_params(self, stream_config: &StreamConfig) -> (Option<usize>, usize) {
        match self {
            StreamingFormat::Lpcm => stream_config.lpcm_streamsize.values(),
            StreamingFormat::Wav => stream_config.wav_streamsize.values(),
            StreamingFormat::Rf64 => stream_config.rf64_streamsize.values(),
            StreamingFormat::Flac => stream_config.flac_streamsize.values(),
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

// streamsize/chunkthreshold pairs for tiny-http response
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
            StreamSize::NoneChunked => write!(f, "NoneChunked"),
            StreamSize::U32maxChunked => write!(f, "U32maxChunked"),
            StreamSize::U32maxNotChunked => write!(f, "U32maxNotChunked"),
            StreamSize::U64maxChunked => write!(f, "U64maxChunked"),
            StreamSize::U64maxNotChunked => write!(f, "U64maxNotChunked"),
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
            BitDepth::Bits16 => write!(f, "16"),
            BitDepth::Bits24 => write!(f, "24"),
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

/// helper holding struct to avoid repeatedly reading the config data
/// or cloning the large Configuration struct
#[derive(Copy, Clone)]
pub struct StreamConfig {
    pub bits_per_sample: u16,
    pub streaming_format: StreamingFormat,
    pub lpcm_streamsize: StreamSize,
    pub wav_streamsize: StreamSize,
    pub flac_streamsize: StreamSize,
    pub rf64_streamsize: StreamSize,
    pub buffering_delay_msec: u32,
}

impl StreamConfig {
    pub fn get() -> StreamConfig {
        let cfg = get_config();
        StreamConfig {
            bits_per_sample: cfg.bits_per_sample.unwrap_or(16),
            streaming_format: cfg.streaming_format.unwrap_or(StreamingFormat::Flac),
            lpcm_streamsize: cfg.lpcm_stream_size.unwrap(),
            wav_streamsize: cfg.wav_stream_size.unwrap(),
            flac_streamsize: cfg.flac_stream_size.unwrap(),
            rf64_streamsize: cfg.rf64_stream_size.unwrap(),
            buffering_delay_msec: cfg.buffering_delay_msec.unwrap_or(0),
        }
    }
}
