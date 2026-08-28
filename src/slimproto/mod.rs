//! SlimProto (squeezelite/Squeezebox) renderer support: UDP discovery, the
//! TCP control connection, and the `strm`/`audg` command frame builders.

pub mod audg;
pub mod discovery;
pub mod frame_envelope;
pub mod parse_frames;
pub mod server;
pub mod strm;
pub mod types;

/// UDP/TCP port used by SlimProto (squeezelite / Squeezebox) clients.
pub const SLIM_PORT: u16 = 3483;
