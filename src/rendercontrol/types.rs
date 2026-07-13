//! Shared data types for renderer discovery and control: the [`Renderer`]
//! struct itself, its protocol/service description types, and the streaming
//! metadata types ([`WavData`], [`StreamInfo`]) passed between
//! [`super::discovery`] and [`super::control`]. `Renderer`'s `impl` block
//! lives entirely in [`super::control`], not here.

use crate::{
    enums::streaming::{BitDepth, StreamingFormat},
    globals::statics::{SERVER_PORT, get_config},
};
use bitflags::bitflags;
#[cfg(feature = "gui")]
use fltk::{button::LightButton, valuator::HorNiceSlider};
use std::sync::{Arc, atomic::AtomicBool};

/// some captured audio parameters (from CPAL)
#[derive(Debug, Clone, Copy)]
pub struct WavData {
    pub sample_format: cpal::SampleFormat,
    pub sample_rate: cpal::SampleRate,
    pub channels: u16,
    pub default_sample_rate: u32,
}

/// the parameters needed for streaming
#[derive(Debug, Clone, Copy)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub bits_per_sample: BitDepth,
    pub streaming_format: StreamingFormat,
    pub server_port: u16,
}

impl StreamInfo {
    pub fn new(sample_rate: u32) -> StreamInfo {
        let config = get_config();
        StreamInfo {
            sample_rate,
            bits_per_sample: BitDepth::from(config.bits_per_sample.unwrap_or(16)),
            streaming_format: config.streaming_format.unwrap_or(StreamingFormat::Flac),
            server_port: config.server_port.unwrap_or(SERVER_PORT),
        }
    }
}

/// An UPNP/DLNA service desciption
#[derive(Debug, Clone)]
pub struct AvService {
    pub(super) service_id: String,
    pub(super) service_type: String,
    pub(super) control_url: String,
}

impl AvService {
    pub(super) fn new() -> AvService {
        AvService {
            service_id: String::new(),
            service_type: String::new(),
            control_url: String::new(),
        }
    }
}

bitflags! {
/// supported UPNP/DLNA protocols
#[derive(Debug, Clone, Copy)]
pub struct SupportedProtocols: u32 {
        const NONE        = 0b0000;
        const OPENHOME    = 0b0001;
        const AVTRANSPORT = 0b0010;
        const ALL = Self::OPENHOME.bits() | Self::AVTRANSPORT.bits();
    }
}

impl SupportedProtocols {
    pub fn is_valid(&self) -> bool {
        (self.bits() & SupportedProtocols::ALL.bits()) != 0
    }
}

#[cfg(feature = "gui")]
#[derive(Debug, Clone, Default)]
/// The UI elements associated with a renderer
pub struct RendUI {
    pub slider: Option<HorNiceSlider>,
    pub button: Option<LightButton>,
}

/// Renderer struct describers a media renderer,
/// info is collected from the GetDescription.xml
/// if GUI is enabled, the renderer tracks it associated UI (a slider and a button)
#[derive(Debug, Clone)]
pub struct Renderer {
    pub player_index: usize,
    pub dev_name: String,
    pub dev_model: String,
    pub dev_type: String,
    pub dev_url: String,
    pub oh_control_url: String,
    pub av_control_url: String,
    pub oh_volume_url: String,
    pub av_volume_url: String,
    // absolute URLs (http://host:port + path), composed once in `parse_url()`
    // since host/port/path are all fixed after discovery
    pub(super) oh_control_full_url: String,
    pub(super) av_control_full_url: String,
    pub(super) oh_volume_full_url: String,
    pub(super) av_volume_full_url: String,
    pub volume: i32,
    pub supported_protocols: SupportedProtocols,
    pub remote_addr: String,
    pub location: String,
    pub services: Vec<AvService>,
    pub playing: bool,
    #[cfg(feature = "gui")]
    pub rend_ui: RendUI,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) agent: ureq::Agent,
    /// guards against overlapping `spawn_play()` calls for this renderer;
    /// shared (via `Arc`) across every clone made from the same discovered
    /// instance, so a click on any clone sees an in-flight play started from
    /// another clone (e.g. the button callback vs. auto-resume)
    pub(super) play_pending: Arc<AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_url_harman_kardon() {
        let mut url = "Avcontrol.url".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/Avcontrol.url");
        url = "/Avcontrol.url".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/Avcontrol.url");
        url = "".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "");
        url = "A/.url".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/A/.url");
    }

    #[test]
    fn test_supported_protocols_is_valid() {
        assert!(!SupportedProtocols::NONE.is_valid());
        assert!(SupportedProtocols::OPENHOME.is_valid());
        assert!(SupportedProtocols::AVTRANSPORT.is_valid());
        assert!((SupportedProtocols::OPENHOME | SupportedProtocols::AVTRANSPORT).is_valid());
        assert!(SupportedProtocols::ALL.is_valid());
    }
}
