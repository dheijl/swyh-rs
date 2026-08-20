//! [`SlimRenderer`]: a connected SlimProto (squeezelite) client, built from
//! its `HELO` handshake.

use crate::slimproto::frames::SlimHelo;
use ecow::EcoString;
#[cfg(feature = "gui")]
use fltk::button::LightButton;

#[cfg(feature = "gui")]
#[derive(Debug, Clone, Default)]
/// The UI elements associated with a SlimProto renderer.
pub struct SlimRendUI {
    pub button: Option<LightButton>,
}

/// A connected SlimProto client. Unlike [`crate::rendercontrol::Renderer`],
/// there's no network client embedded here yet — sending playback commands
/// back to this specific client needs a per-connection outbound channel that
/// hasn't been added yet.
#[derive(Debug, Clone)]
pub struct SlimRenderer {
    pub player_index: usize,
    /// IP only, no port — same convention as `Controller::remote_addr`.
    pub remote_addr: EcoString,
    pub mac: [u8; 6],
    pub device_id: u8,
    pub capabilities: String,
    pub playing: bool,
    #[cfg(feature = "gui")]
    pub rend_ui: SlimRendUI,
}

impl SlimRenderer {
    pub fn from_helo(helo: &SlimHelo, remote_addr: EcoString) -> Self {
        Self {
            player_index: 0,
            remote_addr,
            mac: helo.mac,
            device_id: helo.device_id,
            capabilities: helo.capabilities.clone(),
            playing: false,
            #[cfg(feature = "gui")]
            rend_ui: SlimRendUI::default(),
        }
    }

    /// Best-effort display model, parsed out of the `Model=...` key in the
    /// `HELO` capabilities string (e.g. `Model=squeezelite,...`); falls back
    /// to a generic label if absent.
    pub fn model(&self) -> &str {
        self.capabilities
            .split(',')
            .find_map(|kv| kv.strip_prefix("Model="))
            .unwrap_or("SlimProto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helo(capabilities: &str) -> SlimHelo {
        SlimHelo {
            device_id: 12,
            revision: 0,
            mac: [0, 1, 2, 3, 4, 5],
            wlan_channel_list: 0,
            bytes_received: 0,
            capabilities: capabilities.to_string(),
        }
    }

    #[test]
    fn model_parses_model_capability() {
        let r = SlimRenderer::from_helo(
            &helo("Model=squeezelite,AccuratePlayPoints=1,Firmware=v1.0"),
            "192.168.1.50".into(),
        );
        assert_eq!(r.model(), "squeezelite");
    }

    #[test]
    fn model_falls_back_when_absent() {
        let r = SlimRenderer::from_helo(&helo("AccuratePlayPoints=1"), "192.168.1.50".into());
        assert_eq!(r.model(), "SlimProto");
    }
}
