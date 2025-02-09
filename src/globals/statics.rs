use std::sync::{atomic::AtomicBool, LazyLock};

use crate::{
    enums::messages::MessageType,
    utils::{configuration::Configuration, rwstream::ChannelStream},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use hashbrown::HashMap;
use std::sync::RwLock;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

// streaming clients of the webserver
pub static CLIENTS: LazyLock<RwLock<HashMap<String, ChannelStream>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
// the global GUI logger textbox channel used by all threads
pub static MSGCHANNEL: LazyLock<RwLock<(Sender<MessageType>, Receiver<MessageType>)>> =
    LazyLock::new(|| RwLock::new(unbounded()));
// the global configuration state
pub static CONFIG: LazyLock<RwLock<Configuration>> =
    LazyLock::new(|| RwLock::new(Configuration::read_config()));
// the list of known fltk theme naes
pub static THEMES: [&str; 6] = ["Shake", "Gray", "Tan", "Dark", "Black", "None"];
// the global "enable rms monitor" flag
pub static RUN_RMS_MONITOR: AtomicBool = AtomicBool::new(false);
