use crate::{openhome::rendercontrol::Renderer, server::streaming_server::StreamerFeedBack};
#[derive(Debug, Clone)]
pub enum MessageType {
    SsdpMessage(Box<Renderer>), // boxed to reduce enum size
    PlayerMessage(StreamerFeedBack),
    LogMessage(String),
}
