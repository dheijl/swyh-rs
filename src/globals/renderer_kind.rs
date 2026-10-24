//! [`RendererKind`]: a discovered renderer of either supported protocol,
//! letting code that only cares about "all known renderers" (shutdown,
//! player-index bookkeeping) walk one list instead of two. Protocol-specific
//! control (play/stop/volume mechanics) still goes through the wrapped
//! [`Renderer`]/[`SlimRenderer`] directly via [`RendererKind::as_upnp`]/
//! [`RendererKind::as_slim`] (or their `_mut` counterparts).

use crate::rendercontrol::Renderer;
use crate::slimproto::types::SlimRenderer;

#[derive(Debug, Clone)]
pub enum RendererKind {
    Upnp(Renderer),
    Slim(SlimRenderer),
}

impl RendererKind {
    pub fn as_upnp(&self) -> Option<&Renderer> {
        match self {
            Self::Upnp(r) => Some(r),
            Self::Slim(_) => None,
        }
    }

    pub fn as_upnp_mut(&mut self) -> Option<&mut Renderer> {
        match self {
            Self::Upnp(r) => Some(r),
            Self::Slim(_) => None,
        }
    }

    pub fn as_slim(&self) -> Option<&SlimRenderer> {
        match self {
            Self::Slim(r) => Some(r),
            Self::Upnp(_) => None,
        }
    }

    pub fn as_slim_mut(&mut self) -> Option<&mut SlimRenderer> {
        match self {
            Self::Slim(r) => Some(r),
            Self::Upnp(_) => None,
        }
    }
}

impl From<Renderer> for RendererKind {
    fn from(r: Renderer) -> Self {
        Self::Upnp(r)
    }
}

impl From<SlimRenderer> for RendererKind {
    fn from(r: SlimRenderer) -> Self {
        Self::Slim(r)
    }
}
