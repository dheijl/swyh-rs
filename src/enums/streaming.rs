use ecow::EcoString;
use serde::{Deserialize, Serialize};
use std::{convert::From, fmt, str::FromStr};
use tiny_http::Request;

use crate::{
    globals::statics::get_config, openhome::rendercontrol::WavData,
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
    pub fn needs_wav_hdr(self) -> bool {
        self == StreamingFormat::Wav || self == StreamingFormat::Rf64
    }
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
/// it gathers all the information needed for HTTP streaming and
/// starts out with the default values from the config
/// it is then updated as needed before streaming starts
#[derive(Debug)]
pub struct StreamingContext {
    pub sample_rate: u32,
    pub sample_format: cpal::SampleFormat,
    pub bits_per_sample: BitDepth,
    pub streaming_format: StreamingFormat,
    pub lpcm_streamsize: StreamSize,
    pub wav_streamsize: StreamSize,
    pub flac_streamsize: StreamSize,
    pub rf64_streamsize: StreamSize,
    pub buffering_delay_msec: u32,
    pub remote_addr: EcoString, // ip:port
    pub remote_ip: EcoString,   // ip only
    pub chunksize: usize,
    pub streamsize: Option<usize>,
    pub url: EcoString,
}

impl StreamingContext {
    /// initialize default values from config where possible
    pub fn from_config() -> StreamingContext {
        let cfg = get_config();
        StreamingContext {
            sample_rate: 44100,
            sample_format: cpal::SampleFormat::F32,
            bits_per_sample: BitDepth::from(cfg.bits_per_sample.unwrap_or(16)),
            streaming_format: cfg.streaming_format.unwrap_or(StreamingFormat::Flac),
            lpcm_streamsize: cfg.lpcm_stream_size.unwrap(),
            wav_streamsize: cfg.wav_stream_size.unwrap(),
            flac_streamsize: cfg.flac_stream_size.unwrap(),
            rf64_streamsize: cfg.rf64_stream_size.unwrap(),
            buffering_delay_msec: cfg.buffering_delay_msec.unwrap_or(0),
            remote_addr: EcoString::new(),
            remote_ip: EcoString::new(),
            chunksize: 0,
            streamsize: None,
            url: EcoString::new(),
        }
    }
    /// initialize `remote_addr` and `remote_ip`
    pub fn set_remote_addr(&mut self, rq: &Request) {
        self.url = EcoString::from(rq.url());
        self.remote_addr = EcoString::from(rq.remote_addr().unwrap().to_string());
        self.remote_ip = self.remote_addr.clone();
        if let Some(i) = self.remote_addr.find(':') {
            self.remote_ip.truncate(i);
        }
    }
    /// intialize sample rate and format from `WavData`
    pub fn set_sample_data(&mut self, wd: WavData) {
        self.sample_rate = wd.sample_rate.0;
        self.sample_format = wd.sample_format;
    }
    /// update values from query parameters if present
    pub fn update_format(&mut self, query_params: &StreamingParams) {
        // streaming format
        if let Some(fmt) = query_params.fmt {
            self.streaming_format = fmt;
        }
        // bit depth
        if let Some(bd) = query_params.bd {
            self.bits_per_sample = bd;
        }
        // get default streamsize/chunksize
        let (mut streamsize, mut chunksize) = match self.streaming_format {
            StreamingFormat::Lpcm => self.lpcm_streamsize.values(),
            StreamingFormat::Wav => self.wav_streamsize.values(),
            StreamingFormat::Rf64 => self.rf64_streamsize.values(),
            StreamingFormat::Flac => self.flac_streamsize.values(),
        };
        // unless overridden in query params
        if let Some(ss) = query_params.ss {
            (streamsize, chunksize) = ss.values();
        }
        // update streamsize/chunksize accordingly
        self.streamsize = streamsize;
        self.chunksize = chunksize;
    }
    /// do we need a WAV/RF64 header for this streaming format ?
    /// don't call before all fields are properly initialized
    #[inline]
    pub fn needs_wav_hdr(&self) -> bool {
        self.streaming_format.needs_wav_hdr()
    }
    /// return the dlna string for this stream
    /// don't call before all fields are properly initialized
    pub fn dlna_string(&self) -> String {
        self.streaming_format.dlna_string(self.bits_per_sample)
    }
}
