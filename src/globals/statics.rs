//! Process-wide static singletons: configuration, active streaming clients,
//! discovered renderers, the inter-thread message channel, and misc constants.

use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard, atomic::AtomicBool};

use crate::{
    audio::rwstream::ChannelStream, enums::messages::MessageType, rendercontrol::Renderer,
    utils::configuration::Configuration,
};

use arc_swap::{ArcSwap, Guard};
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
pub static SAMPLE_RATES: &[u32] = &[44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];

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

/// streaming clients of the webserver.
///
/// `ArcSwap` rather than `RwLock`: this is read on every CPAL capture callback
/// (`distribute_samples`, every ~10-20ms) but only written on client connect/
/// disconnect (human timescale), so reads should never block on a writer.
/// A snapshot from `get_clients()` keeps its map (and every entry's
/// `Sender`/`Receiver`) alive via its own `Arc` for as long as it's held, so a
/// concurrent removal can never yank an entry out from under an in-progress
/// iteration — worst case, one extra sample is queued into (or a send fails
/// harmlessly on, see `ChannelStream::write`) a channel whose receiving end is
/// already tearing down.
static CLIENTS: LazyLock<ArcSwap<HashMap<EcoString, ChannelStream>>> =
    LazyLock::new(|| ArcSwap::from_pointee(HashMap::new()));

/// Wait-free snapshot of the current streaming clients.
pub fn get_clients() -> Arc<HashMap<EcoString, ChannelStream>> {
    CLIENTS.load_full()
}

/// Wait-free snapshot for the CPAL capture callback: `load()` avoids the
/// atomic refcount bump that `load_full()` pays on every call, at the cost
/// of a borrowed `Guard` instead of an owned `Arc`. Only use this where the
/// guard's scope is short and never held across a blocking call, or it
/// delays the writer's reclamation of the previous snapshot.
pub fn get_clients_fast() -> Guard<Arc<HashMap<EcoString, ChannelStream>>> {
    CLIENTS.load()
}

/// Insert a new streaming client (on connect), returning the resulting client count.
pub fn insert_client(remote_addr: EcoString, stream: ChannelStream) -> usize {
    CLIENTS.rcu(|clients| {
        let mut clients = (**clients).clone();
        clients.insert(remote_addr.clone(), stream.clone());
        clients
    });
    CLIENTS.load().len()
}

/// Remove a streaming client (on disconnect), returning the removed client
/// (if it was still present) and the resulting client count.
pub fn remove_client(remote_addr: &str) -> (Option<ChannelStream>, usize) {
    let mut removed = None;
    CLIENTS.rcu(|clients| {
        let mut clients = (**clients).clone();
        removed = clients.remove(remote_addr);
        clients
    });
    (removed, CLIENTS.load().len())
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
