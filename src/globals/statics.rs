use crate::{
    enums::messages::MessageType,
    utils::{configuration::Configuration, rwstream::ChannelStream},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use hashbrown::HashMap;
use once_cell::sync::Lazy;
use parking_lot::RwLock;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

// streaming clients of the webserver
pub static CLIENTS: Lazy<RwLock<HashMap<String, ChannelStream>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
// the global GUI logger textbox channel used by all threads
pub static MSGCHANNEL: Lazy<RwLock<(Sender<MessageType>, Receiver<MessageType>)>> =
    Lazy::new(|| RwLock::new(unbounded()));
// the global configuration state
pub static CONFIG: Lazy<RwLock<Configuration>> =
    Lazy::new(|| RwLock::new(Configuration::read_config()));
// the list of knowb fltk theme naes
pub static THEMES: [&str; 6] = ["Shake", "Gray", "Tan", "Dark", "Black", "None"];
