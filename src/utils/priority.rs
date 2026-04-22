//! Process priority adjustment.
//!
//! [`raise_priority`] bumps the process scheduling priority on Windows (ABOVE_NORMAL)
//! and Linux (nice -10) to reduce audio capture stuttering under CPU load.

use crate::{
    fl,
    utils::ui_logger::{LogCategory, ui_log},
};

#[cfg(target_os = "windows")]
pub fn raise_priority() {
    use winapi_easy::process::{Process, ProcessPriority};
    let rc = Process::current().set_priority(ProcessPriority::AboveNormal);
    if rc.is_err() {
        ui_log(
            LogCategory::Error,
            &fl!("err-priority-windows", "error" = format!("{rc:?}")),
        );
        return;
    }
    ui_log(LogCategory::Info, &fl!("priority-above-normal"));
}

#[cfg(target_os = "linux")]
pub fn raise_priority() {
    // the following only works when you're root on Linux
    // or if you give the program CAP_SYS_NICE (cf. setcap)
    // or are a user of the pipewire group
    use rustix::process::{getpriority_process, setpriority_process};
    if let Ok(pri) = getpriority_process(None)
        && pri >= 0
    {
        if setpriority_process(None, -10).is_err() {
            ui_log(LogCategory::Warning, &fl!("err-priority-linux"));
        } else {
            ui_log(LogCategory::Info, &fl!("priority-nice"));
        }
    }
}

#[cfg(target_os = "macos")]
pub fn raise_priority() {}
