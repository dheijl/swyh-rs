use std::sync::{LazyLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicBool};

use crate::{
    enums::messages::MessageType,
    openhome::rendercontrol::Renderer,
    utils::{configuration::Configuration, rwstream::ChannelStream},
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use hashbrown::HashMap;
use std::sync::RwLock;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

// streaming clients of the webserver
pub static CLIENTS: LazyLock<RwLock<HashMap<String, ChannelStream>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub fn get_clients() -> RwLockReadGuard<'static, HashMap<String, ChannelStream>> {
    CLIENTS.read().expect("CLIENTS read lock poisoned")
}
pub fn get_clients_mut() -> RwLockWriteGuard<'static, HashMap<String, ChannelStream>> {
    CLIENTS.write().expect("CLIENTS write lock poisoned")
}

// all currentlyknown renderers as discovered by SSDP
pub static RENDERERS: LazyLock<RwLock<Vec<Renderer>>> =
    LazyLock::new(|| RwLock::new(Vec::<Renderer>::new()));
pub fn get_renderers() -> RwLockReadGuard<'static, Vec<Renderer>> {
    RENDERERS.read().expect("RENDERERS read lock poisened")
}
pub fn get_renderers_mut() -> RwLockWriteGuard<'static, Vec<Renderer>> {
    RENDERERS.write().expect("RENDERERS write lock poisened")
}

// the global GUI logger textbox channel used by all threads
pub static MSGCHANNEL: LazyLock<RwLock<(Sender<MessageType>, Receiver<MessageType>)>> =
    LazyLock::new(|| RwLock::new(unbounded()));
pub fn get_msgchannel() -> RwLockReadGuard<'static, (Sender<MessageType>, Receiver<MessageType>)> {
    MSGCHANNEL.read().expect("MSGCHANNEL read lock poisoned")
}

// the global configuration state
pub static CONFIG: LazyLock<RwLock<Configuration>> =
    LazyLock::new(|| RwLock::new(Configuration::read_config()));
pub fn get_config() -> RwLockReadGuard<'static, Configuration> {
    CONFIG.read().expect("CONFIG read lock poisoned")
}
pub fn get_config_mut() -> RwLockWriteGuard<'static, Configuration> {
    CONFIG.write().expect("CONFIG write lock poisoned")
}

// the list of known fltk theme names
pub static THEMES: [&str; 6] = ["Shake", "Gray", "Tan", "Dark", "Black", "None"];

// the global "enable rms monitor" flag
pub static RUN_RMS_MONITOR: AtomicBool = AtomicBool::new(false);
