use log::{error, info, warn};

/// `ui_log`
/// - log a message to the terminal and the logfile
/// - send a logmessage to the textbox on the Crossbeam LOGCHANNEL when running the GUI
pub fn ui_log(s: &str) {
    if s.starts_with("*W") {
        warn!("tb_log: {}", s);
    } else if s.starts_with("*E") {
        error!("tb_log: {}", s);
    } else {
        info!("tb_log: {}", s);
    }

    #[cfg(feature = "gui")]
    {
        use crate::globals::statics::LOGCHANNEL;
        use fltk::app;
        let logger = &LOGCHANNEL.read().0;
        let _ = logger.send(s.to_string());
        app::awake();
        std::thread::yield_now();
    }
}