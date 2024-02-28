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
            StreamingFormat::Lpcm => write!(f, "LPCM"),
            StreamingFormat::Wav => write!(f, "WAV"),
            StreamingFormat::Flac => write!(f, "FLAC"),
            StreamingFormat::Rf64 => write!(f, "RF64"),
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
