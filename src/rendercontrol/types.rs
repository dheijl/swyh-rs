//! Shared data types for renderer discovery and control: the [`Renderer`]
//! struct itself, its protocol/service description types, and the streaming
//! metadata types ([`WavData`], [`StreamInfo`]) passed between
//! [`super::discovery`] and [`super::control`]. `Renderer`'s `impl` block
//! lives entirely in [`super::control`], not here.

use crate::{
    enums::streaming::{BitDepth, StreamingFormat},
    globals::{
        RendererKind,
        statics::{SERVER_PORT, get_config},
    },
};
use ecow::EcoString;
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
    pub(super) service_id: EcoString,
    pub(super) service_type: EcoString,
    pub(super) control_url: EcoString,
}

impl AvService {
    pub(super) fn new() -> AvService {
        AvService {
            service_id: EcoString::new(),
            service_type: EcoString::new(),
            control_url: EcoString::new(),
        }
    }
}

/// supported UPNP/DLNA protocols
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SupportedProtocols {
    pub openhome: bool,
    pub avtransport: bool,
}

impl SupportedProtocols {
    pub fn is_valid(&self) -> bool {
        self.openhome || self.avtransport
    }
}

#[cfg(feature = "gui")]
#[derive(Debug, Clone, Default)]
/// The UI elements associated with a renderer
pub struct RendUI {
    pub slider: Option<HorNiceSlider>,
    pub button: Option<LightButton>,
}

/// The subset of a [`Renderer`]'s fields needed to drive play/stop/volume
/// over the network.
/// [`Renderer`] embeds this behind an `Arc` instead of duplicating these
/// fields itself, so cloning a `Renderer` (e.g. once per UI widget) or handing
/// a background thread a handle to drive play/stop/volume is a refcount bump
/// rather than a field-by-field copy.
/// Its `impl` block lives entirely in [`super::control`], not here.
#[derive(Debug, Clone)]
pub struct Controller {
    // public identity/state, read directly by the UI and CLI binaries
    pub dev_name: String,
    pub remote_addr: EcoString,
    pub supported_protocols: SupportedProtocols,

    // internal mod wiring, only ever touched from control.rs/discovery.rs
    pub(super) agent: ureq::Agent,
    pub(super) host: String,
    pub(super) port: u16,
    // these are computed in ssdp discovery for reuse later
    pub(super) oh_control_full_url: String,
    pub(super) av_control_full_url: String,
    pub(super) oh_volume_full_url: String,
    pub(super) av_volume_full_url: String,
}

/// Renderer struct describers a media renderer,
/// info is collected from the GetDescription.xml
/// if GUI is enabled, the renderer tracks it associated UI (a slider and a button)
#[derive(Debug, Clone)]
pub struct Renderer {
    // public identity/state, read directly by the UI and CLI binaries
    pub player_index: usize,
    pub dev_model: EcoString,
    pub dev_url: EcoString,
    pub volume: i32,
    pub location: EcoString,
    pub playing: bool,
    #[cfg(feature = "gui")]
    /// the GUI fields associated with this renderer neede when the streaming
    /// sends a feedback message to the UI or when syncing volume sliders
    pub rend_ui: RendUI,
    /// the renderer fields needed by the GUI play/stop/volume background threads
    /// grouped in an `Arc`` so very cheap to clone when spawning
    pub controller: Arc<Controller>,

    // internal mod wiring, only ever touched from control.rs/discovery.rs
    pub(super) dev_type: EcoString,
    pub(super) oh_control_url: EcoString,
    pub(super) av_control_url: EcoString,
    pub(super) oh_volume_url: EcoString,
    pub(super) av_volume_url: EcoString,
    pub(super) services: Vec<AvService>,
    /// guards against overlapping `spawn_play()` calls for this renderer;
    /// shared (via `Arc`) across every clone made from the same discovered
    /// instance, so a click on any clone sees an in-flight play started from
    /// another clone (e.g. the button callback vs. auto-resume)
    pub(super) play_pending: Arc<AtomicBool>,
}

/// Decide which (if any) UPnP renderer a `StreamerFeedBack`/`ChannelStream`
/// belongs to, given its remote IP and its `is_slim` marker (see
/// `StreamingContext::slim`), against the IPs of the currently-known UPnP
/// renderers. `upnp_renderer_ips` must already be filtered down to UPnP
/// renderers only — the returned index is a position in that filtered
/// sequence, not in a merged UPnP+SlimProto list. Most callers should use
/// [`find_matching_upnp_renderer`] instead, which does that filtering (and
/// the index resolution back to a renderer) for them.
///
/// Returns `None` — a deliberate no-op, never a panic/unwrap — when:
/// * the feedback is SlimProto's own (`is_slim == true`): it was never a UPnP
///   renderer's traffic in the first place, no matter what shares its IP;
/// * no UPnP renderer's IP matches at all (e.g. a manually-entered stream URL
///   fetched by something that isn't one of our discovered renderers
/// * more than one UPnP renderer shares this IP (ambiguous — matches the
///   pre-existing "Bubble UPnP same-IP" bail-out behaviour).
pub fn match_upnp_renderer_index<'a>(
    is_slim: bool,
    feedback_ip: &str,
    upnp_renderer_ips: impl Iterator<Item = &'a str>,
) -> Option<usize> {
    if is_slim {
        return None;
    }
    let mut found: Option<usize> = None;
    for (i, ip) in upnp_renderer_ips.enumerate() {
        if ip == feedback_ip {
            if found.is_some() {
                return None;
            }
            found = Some(i);
        }
    }
    found
}

/// [`match_upnp_renderer_index`], but operating directly on the merged
/// UPnP+SlimProto `renderers` list: filters it down to the UPnP entries
/// (in order), resolves the match against that filtered sequence, then
/// indexes back into it — so the caller never has to juggle a separate
/// "filtered index vs. merged-list index" distinction itself.
pub fn find_matching_upnp_renderer<'a>(
    renderers: &'a [RendererKind],
    is_slim: bool,
    feedback_ip: &str,
) -> Option<&'a Renderer> {
    let upnp_renderers: Vec<&Renderer> =
        renderers.iter().filter_map(RendererKind::as_upnp).collect();
    let ips = upnp_renderers
        .iter()
        .map(|r| r.controller.remote_addr.as_str());
    let idx = match_upnp_renderer_index(is_slim, feedback_ip, ips)?;
    Some(upnp_renderers[idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slimproto::{parse_frames::SlimHelo, types::SlimRenderer};
    use std::net::TcpListener;
    use std::sync::Mutex;

    fn upnp_renderer(remote_addr: &str) -> RendererKind {
        let mut r = Renderer::new(&ureq::agent());
        Arc::make_mut(&mut r.controller).remote_addr = remote_addr.into();
        RendererKind::Upnp(r)
    }

    fn slim_renderer(remote_addr: &str) -> RendererKind {
        let helo = SlimHelo {
            device_id: 12,
            revision: 0,
            mac: [0, 1, 2, 3, 4, 5],
            uuid: None,
            wlan_channel_list: 0,
            bytes_received: 0,
            capabilities: "".into(),
            lang: None,
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
        let addr = listener.local_addr().expect("local_addr");
        let stream = std::net::TcpStream::connect(addr).expect("connect loopback stream");
        RendererKind::Slim(SlimRenderer::from_helo(
            &helo,
            remote_addr.into(),
            12345,
            Arc::new(Mutex::new(stream)),
        ))
    }

    #[test]
    fn find_matching_upnp_renderer_single_match() {
        let renderers = [upnp_renderer("192.168.1.10"), upnp_renderer("192.168.1.20")];
        let found = find_matching_upnp_renderer(&renderers, false, "192.168.1.20")
            .expect("expected a match");
        assert_eq!(found.controller.remote_addr, "192.168.1.20");
    }

    /// The riskiest part of merging the UPnP and SlimProto renderer lists:
    /// SlimProto entries interleaved among the UPnP ones must not shift which
    /// UPnP renderer a given filtered-sequence index resolves back to.
    #[test]
    fn find_matching_upnp_renderer_skips_interleaved_slim_renderers() {
        let renderers = [
            slim_renderer("192.168.1.5"),
            upnp_renderer("192.168.1.10"),
            slim_renderer("192.168.1.6"),
            upnp_renderer("192.168.1.20"),
        ];
        let found = find_matching_upnp_renderer(&renderers, false, "192.168.1.10")
            .expect("expected a match");
        assert_eq!(found.controller.remote_addr, "192.168.1.10");
        let found = find_matching_upnp_renderer(&renderers, false, "192.168.1.20")
            .expect("expected a match");
        assert_eq!(found.controller.remote_addr, "192.168.1.20");
    }

    #[test]
    fn find_matching_upnp_renderer_ambiguous_same_ip() {
        let renderers = [upnp_renderer("192.168.1.10"), upnp_renderer("192.168.1.10")];
        assert!(find_matching_upnp_renderer(&renderers, false, "192.168.1.10").is_none());
    }

    #[test]
    fn find_matching_upnp_renderer_slim_feedback_ignored() {
        let renderers = [upnp_renderer("192.168.1.10")];
        assert!(find_matching_upnp_renderer(&renderers, true, "192.168.1.10").is_none());
    }

    #[test]
    fn find_matching_upnp_renderer_no_match() {
        let renderers = [upnp_renderer("192.168.1.10")];
        assert!(find_matching_upnp_renderer(&renderers, false, "192.168.1.99").is_none());
    }

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
    fn match_upnp_renderer_index_single_match() {
        let ips = ["192.168.1.10", "192.168.1.20"];
        assert_eq!(
            match_upnp_renderer_index(false, "192.168.1.10", ips.into_iter()),
            Some(0)
        );
        assert_eq!(
            match_upnp_renderer_index(false, "192.168.1.20", ips.into_iter()),
            Some(1)
        );
    }

    #[test]
    fn match_upnp_renderer_index_ambiguous_same_ip() {
        let ips = ["192.168.1.10", "192.168.1.10"];
        assert_eq!(
            match_upnp_renderer_index(false, "192.168.1.10", ips.into_iter()),
            None
        );
    }

    #[test]
    fn match_upnp_renderer_index_slim_feedback_ignored() {
        let ips = ["192.168.1.10"];
        assert_eq!(
            match_upnp_renderer_index(true, "192.168.1.10", ips.into_iter()),
            None
        );
    }

    #[test]
    fn match_upnp_renderer_index_no_match() {
        let ips = ["192.168.1.10"];
        assert_eq!(
            match_upnp_renderer_index(false, "192.168.1.99", ips.into_iter()),
            None
        );
    }

    #[test]
    fn match_upnp_renderer_index_empty_list() {
        let ips: [&str; 0] = [];
        assert_eq!(
            match_upnp_renderer_index(false, "192.168.1.10", ips.into_iter()),
            None
        );
    }

    #[test]
    fn test_supported_protocols_is_valid() {
        assert!(!SupportedProtocols::default().is_valid());
        assert!(
            SupportedProtocols {
                openhome: true,
                avtransport: false,
            }
            .is_valid()
        );
        assert!(
            SupportedProtocols {
                openhome: false,
                avtransport: true,
            }
            .is_valid()
        );
        assert!(
            SupportedProtocols {
                openhome: true,
                avtransport: true,
            }
            .is_valid()
        );
    }
}
