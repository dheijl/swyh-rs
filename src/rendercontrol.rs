//! DLNA/OpenHome renderer controller.
//!
//! Handles SSDP discovery ([`discover`], in [`discovery`]), and controls AV
//! renderers using both the OpenHome Playlist and UPnP AVTransport protocols
//! ([`control`]). [`Renderer`] and its supporting types are defined in
//! [`types`], but [`Renderer`]'s entire `impl` (construction, play/stop,
//! volume) lives in [`control`] alongside the SOAP mechanics it drives.
//! [`WavData`] carries the audio format metadata, and [`StreamInfo`] holds
//! the per-stream URL and bit-depth.

mod control;
mod discovery;
mod types;

pub use control::PlayOutcome;
pub use discovery::{discover, new_agent};
#[cfg(feature = "gui")]
pub use types::RendUI;
pub use types::{AvService, Renderer, StreamInfo, SupportedProtocols, WavData};
