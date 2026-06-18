#[cfg(feature = "gui")]
use std::{thread, time::Duration};

/// Show a fatal error dialog and exit the process.
#[cfg(feature = "gui")]
pub fn fatal_error(msg: String) -> ! {
    fltk::dialog::message_default(&msg);
    while fltk::app::wait() {
        thread::sleep(Duration::from_millis(250));
        if fltk::app::event_is_click() {
            break;
        }
    }
    std::process::exit(-1);
}
