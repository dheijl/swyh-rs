use crate::{openhome::rendercontrol::Renderer, server::streaming_server::StreamerFeedBack};
#[derive(Debug, Clone)]
pub enum MessageType {
    SsdpMessage(Box<Renderer>),
    PlayerMessage(StreamerFeedBack),
    LogMessage(String),
}
