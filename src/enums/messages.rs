//! Inter-thread message types carried over the application's crossbeam channel.

use crate::{
    renderers::rendercontrol::{PlayOutcome, Renderer},
    server::streaming_server::StreamerFeedBack,
};

#[derive(Debug, Clone)]
pub enum MessageType {
    SsdpMessage(Box<Renderer>), // boxed to reduce enum size
    PlayerMessage(StreamerFeedBack),
    /// outcome of a `Renderer::spawn_play()` attempt, see [`PlayOutcome`]
    PlayResult(PlayOutcome),
    LogMessage(String),
    CaptureAborted,
}
