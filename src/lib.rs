//! swyh-rs — Stream What You Hear for Rust.
//!
//! Root crate that re-exports the audio capture, HTTP streaming server,
//! DLNA/OpenHome renderer control, FLTK GUI, and shared utility modules.

#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "en");

pub mod enums;
pub mod globals;
pub mod openhome;
pub mod server;
pub mod ui;
pub mod utils;