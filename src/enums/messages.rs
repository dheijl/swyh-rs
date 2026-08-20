//! Inter-thread message types carried over the application's crossbeam channel.

use crate::{
    rendercontrol::{PlayOutcome, Renderer},
    server::streaming_server::StreamerFeedBack,
    slimproto::types::SlimRenderer,
};
use ecow::EcoString;

#[derive(Debug, Clone)]
pub enum MessageType {
    SsdpMessage(Box<Renderer>), // boxed to reduce enum size
    PlayerMessage(StreamerFeedBack),
    /// outcome of a `Renderer::spawn_play()` attempt, see [`PlayOutcome`]
    PlayResult(PlayOutcome),
    LogMessage(String),
    CaptureAborted,
    /// a SlimProto (squeezelite) client sent its `HELO` handshake
    SlimHelo(Box<SlimRenderer>), // boxed to reduce enum size
    /// a SlimProto client's TCP connection dropped; `remote_addr` identifies
    /// which `SLIM_RENDERERS` entry to mark not-playing / turn its button
    /// off. `peer_port` is that connection's source port: a reconnect from
    /// the same IP refreshes the renderer's `peer_port` in place before this
    /// message is (necessarily racily) processed, so the handler must check
    /// `peer_port` still matches before clearing state — otherwise a stale
    /// disconnect notification for an already-superseded connection can
    /// clobber a newer one's `playing`/button state.
    SlimDisconnected {
        remote_addr: EcoString,
        peer_port: u16,
    },
}
