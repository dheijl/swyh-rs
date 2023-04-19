use serde::{Deserialize, Serialize};
use std::fmt;

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
}

impl fmt::Display for StreamingFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingFormat::Lpcm => write!(f, "LPCM"),
            StreamingFormat::Wav => write!(f, "WAV"),
            StreamingFormat::Flac => write!(f, "FLAC"),
        }
    }
}
