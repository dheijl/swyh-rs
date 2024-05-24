use serde::{Deserialize, Serialize};
use std::{convert::From, fmt, str::FromStr};

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
        match s {
            "Lpcm" => Ok(StreamingFormat::Lpcm),
            "Wav" => Ok(StreamingFormat::Wav),
            "Flac" => Ok(StreamingFormat::Flac),
            "Rf64" => Ok(StreamingFormat::Rf64),
            _ => Err(()),
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
        let s = s.to_lowercase();
        match s.as_str() {
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
