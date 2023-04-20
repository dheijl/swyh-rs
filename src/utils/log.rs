use fltk::app;
use log::{error, info, warn};

use crate::globals::statics::{HAVE_UI, LOGCHANNEL};

/// ui_log - send a logmessage to the textbox on the Crossbeam LOGCHANNEL
pub fn ui_log(s: String) {
    let cat: &str = &s[..2];
    if *HAVE_UI {
        match cat {
            "*W" => warn!("tb_log: {}", s),
            "*E" => error!("tb_log: {}", s),
            _ => info!("tb_log: {}", s),
        };
        let logger = &LOGCHANNEL.read().0;
        logger.send(s).unwrap();
        app::awake();
    } else {
        match cat {
            "*W" => warn!("{s}"),
            "*E" => error!("{s}"),
            _ => info!("{s}"),
        };
    }
}
