//! swyh-rs — Stream What You Hear for Rust.
//!
//! Root crate that re-exports the audio capture, HTTP streaming server,
//! DLNA/OpenHome renderer control, FLTK GUI, and shared utility modules.

pub mod audio;
pub mod enums;
pub mod globals;
pub mod renderers;
pub mod server;
pub mod ui;
pub mod utils;
