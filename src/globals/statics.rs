//! Process-wide static singletons: configuration, active streaming clients,
//! discovered renderers, the inter-thread message channel, and misc constants.

use std::sync::{LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicBool};

use crate::{
    audio::rwstream::ChannelStream, enums::messages::MessageType,
    renderers::rendercontrol::Renderer, utils::configuration::Configuration,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use ecow::EcoString;
use hashbrown::HashMap;

/// app version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_DATE: Option<&str> = option_env!("BUILD_DAY");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

/// `one_minute` duration for sleep
pub const ONE_MINUTE: f64 = 60.0 * 1000.0;

/// the thread stack size
pub const THREAD_STACK: usize = 4 * 1024 * 1024;

/// the global "enable rms monitor" flag
pub static RUN_RMS_MONITOR: AtomicBool = AtomicBool::new(false);

/// the list of supported audio sample rates
pub const SAMPLE_RATES: &[u32] = &[44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];

/// the list of known fltk theme names
pub static THEMES: &[&str] = &[
    "Shake",
    "Gray",
    "Tan",
    "Dark",
    "Black",
    "Nord",
    "Dracula",
    "Gruvbox Dark",
    "Solarized Light",
    "Monokai",
    "Solarized Dark",
    "Oceanic Next",
    "Minimalist",
    "None",
];
/// number of available themes (excluding the last dummy one, "None")
pub const NTHEMES: usize = THEMES.len() - 1;
/// default color theme index for new configs (Solarized Light) - keep in sync with THEMES array
pub const DEFAULT_COLOR_THEME: u8 = 8;

/// the list of known fltk widget style (`WidgetScheme`) names
pub static STYLES: &[&str] = &["Fleet1", "Fleet2", "None"];
/// number of available styles (excluding the last dummy one, "None")
pub const NSTYLES: usize = STYLES.len() - 1;
/// default widget style index for new configs (Fleet2) - keep in sync with STYLES array
pub const DEFAULT_WIDGET_SCHEME: u8 = 1;

/// streaming clients of the webserver
static CLIENTS: LazyLock<RwLock<HashMap<EcoString, ChannelStream>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub fn get_clients() -> RwLockReadGuard<'static, HashMap<EcoString, ChannelStream>> {
    CLIENTS.read().expect("CLIENTS read lock poisoned")
}
pub fn get_clients_mut() -> RwLockWriteGuard<'static, HashMap<EcoString, ChannelStream>> {
    CLIENTS.write().expect("CLIENTS write lock poisoned")
}

/// all currently known renderers as discovered by SSDP
static RENDERERS: LazyLock<RwLock<Vec<Renderer>>> = LazyLock::new(|| RwLock::new(Vec::new()));
pub fn get_renderers() -> RwLockReadGuard<'static, Vec<Renderer>> {
    RENDERERS.read().expect("RENDERERS read lock poisoned")
}
pub fn get_renderers_mut() -> RwLockWriteGuard<'static, Vec<Renderer>> {
    RENDERERS.write().expect("RENDERERS write lock poisoned")
}

/// the global GUI logger textbox channel used by all threads
/// no `RwLock` needed: `Sender`/`Receiver` are already `Send + Sync` on their own
/// (crossbeam-channel does its own internal synchronization), and this is set once
/// at startup and never mutated afterwards
static MSGCHANNEL: LazyLock<(Sender<MessageType>, Receiver<MessageType>)> =
    LazyLock::new(unbounded);
pub fn get_msgchannel() -> &'static (Sender<MessageType>, Receiver<MessageType>) {
    &MSGCHANNEL
}

/// the global configuration state
static CONFIG: LazyLock<RwLock<Configuration>> = LazyLock::new(|| {
    RwLock::new(Configuration::read_config().unwrap_or_else(|e| {
        eprintln!("Fatal: failed to read config: {e:#}");
        std::process::exit(1);
    }))
});
pub fn get_config() -> RwLockReadGuard<'static, Configuration> {
    CONFIG.read().expect("CONFIG read lock poisoned")
}
pub fn get_config_mut() -> RwLockWriteGuard<'static, Configuration> {
    CONFIG.write().expect("CONFIG write lock poisoned")
}
