use log::{error, info, warn};
use std::fmt::Display;

pub enum LogCategory {
    Error,
    Warning,
    Info,
}

impl Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogCategory::Error => write!(f, "*E "),
            LogCategory::Warning => write!(f, "*W "),
            LogCategory::Info => write!(f, ""),
        }
    }
}

/// `ui_log`
/// - log a messgae to the terminal and the logfile
/// - send a logmessage to the textbox on the Crossbeam LOGCHANNEL when runing the GUI
pub fn ui_log(cat: LogCategory, s: &str) {
    match cat {
        LogCategory::Warning => warn!("tb_log: {s}"),
        LogCategory::Error => error!("tb_log: {s}"),
        LogCategory::Info => info!("tb_log: {s}"),
    };
    #[cfg(feature = "gui")]
    {
        use crate::enums::messages::MessageType;
        use crate::globals::statics::get_msgchannel;
        use fltk::app;
        get_msgchannel()
            .0
            .send(MessageType::LogMessage(cat.to_string() + s))
            .unwrap();
        app::awake();
    }
}
