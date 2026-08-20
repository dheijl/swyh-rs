pub mod discovery;
pub mod frames;
pub mod server;
pub mod strm;
pub mod types;

/// UDP/TCP port used by SlimProto (squeezelite / Squeezebox) clients.
pub const SLIM_PORT: u16 = 3483;
