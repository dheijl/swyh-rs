//! Utility modules: configuration, CLI argument parsing, logging, networking,
//! process priority, shared traits, and background thread helpers.

pub mod bincommon;
pub mod commandline;
pub mod configuration;
pub mod extra_threads;
pub mod local_ip_address;
pub mod priority;
pub(crate) mod traits;
pub mod ui_logger;
