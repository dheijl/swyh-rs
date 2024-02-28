use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

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

impl StreamSize {
    pub fn values(&self) -> (Option<usize>, usize) {
        match self {
            StreamSize::NoneChunked => (None, 8192),
            StreamSize::U32maxChunked => (Some(u32::MAX as usize), 8192),
            StreamSize::U32maxNotChunked => (Some((u32::MAX - 1) as usize), u32::MAX as usize),
            StreamSize::U64maxChunked => (Some(u64::MAX as usize), 8192),
            StreamSize::U64maxNotChunked => (Some((u64::MAX - 1) as usize), u64::MAX as usize),
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
        match s {
            "NoneChunked" => Ok(StreamSize::NoneChunked),
            "U32maxChunked" => Ok(StreamSize::U32maxChunked),
            "U32maxNotChunked" => Ok(StreamSize::U32maxNotChunked),
            "U64maxChunked" => Ok(StreamSize::U64maxChunked),
            "U64maxNotChunked" => Ok(StreamSize::U64maxNotChunked),
            _ => Err(()),
        }
    }
}
