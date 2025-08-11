use std::sync::{LazyLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicBool};

use crate::{
    enums::messages::MessageType,
    openhome::rendercontrol::Renderer,
    utils::{configuration::Configuration, rwstream::ChannelStream},
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use ecow::EcoString;
use hashbrown::HashMap;
use std::sync::RwLock;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_DATE: Option<&str> = option_env!("BUILD_DAY");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

/// one_minute duration for sleep
pub const ONE_MINUTE: f64 = 60.0 * 1000.0;

/// the thread stack size
pub const THREAD_STACK: usize = 4 * 1024 * 1024;

/// the list of known fltk theme names
pub static THEMES: &[&str] = &["Shake", "Gray", "Tan", "Dark", "Black", "None"];

/// the global "enable rms monitor" flag
pub static RUN_RMS_MONITOR: AtomicBool = AtomicBool::new(false);

/// streaming clients of the webserver
static CLIENTS: LazyLock<RwLock<HashMap<EcoString, ChannelStream>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub fn get_clients() -> RwLockReadGuard<'static, HashMap<EcoString, ChannelStream>> {
    CLIENTS.read().expect("CLIENTS read lock poisoned")
}
pub fn get_clients_mut() -> RwLockWriteGuard<'static, HashMap<EcoString, ChannelStream>> {
    CLIENTS.write().expect("CLIENTS write lock poisoned")
}

/// all currentlyknown renderers as discovered by SSDP
static RENDERERS: LazyLock<RwLock<Vec<Renderer>>> =
    LazyLock::new(|| RwLock::new(Vec::<Renderer>::new()));
pub fn get_renderers() -> RwLockReadGuard<'static, Vec<Renderer>> {
    RENDERERS.read().expect("RENDERERS read lock poisened")
}
pub fn get_renderers_mut() -> RwLockWriteGuard<'static, Vec<Renderer>> {
    RENDERERS.write().expect("RENDERERS write lock poisened")
}

/// the global GUI logger textbox channel used by all threads
static MSGCHANNEL: LazyLock<RwLock<(Sender<MessageType>, Receiver<MessageType>)>> =
    LazyLock::new(|| RwLock::new(unbounded()));
pub fn get_msgchannel() -> RwLockReadGuard<'static, (Sender<MessageType>, Receiver<MessageType>)> {
    MSGCHANNEL.read().expect("MSGCHANNEL read lock poisoned")
}

/// the global configuration state
static CONFIG: LazyLock<RwLock<Configuration>> =
    LazyLock::new(|| RwLock::new(Configuration::read_config()));
pub fn get_config() -> RwLockReadGuard<'static, Configuration> {
    CONFIG.read().expect("CONFIG read lock poisoned")
}
pub fn get_config_mut() -> RwLockWriteGuard<'static, Configuration> {
    CONFIG.write().expect("CONFIG write lock poisoned")
}
