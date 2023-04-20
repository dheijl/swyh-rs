use crate::utils::{configuration::Configuration, rwstream::ChannelStream};

use crossbeam_channel::{unbounded, Receiver, Sender};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = env!("CARGO_BIN_NAME");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

// streaming clients of the webserver
pub static CLIENTS: Lazy<RwLock<HashMap<String, ChannelStream>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
// the global GUI logger textbox channel used by all threads
pub static LOGCHANNEL: Lazy<RwLock<(Sender<String>, Receiver<String>)>> =
    Lazy::new(|| RwLock::new(unbounded()));
// the global configuration state
pub static CONFIG: Lazy<RwLock<Configuration>> =
    Lazy::new(|| RwLock::new(Configuration::read_config()));
// UI or CLI
pub static HAVE_UI: Lazy<bool> = Lazy::new(|| APP_NAME == "swyh-rs");
