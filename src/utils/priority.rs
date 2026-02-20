use crate::utils::ui_logger::{LogCategory, ui_log};

#[cfg(target_os = "windows")]
pub fn raise_priority() {
    use windows::Win32::{
        Foundation::GetLastError,
        System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, GetCurrentProcess, GetCurrentProcessId, SetPriorityClass,
        },
    };
    unsafe {
        let id = GetCurrentProcess();
        if SetPriorityClass(id, ABOVE_NORMAL_PRIORITY_CLASS).is_err() {
            let e = GetLastError();
            let p = GetCurrentProcessId();
            ui_log(
                LogCategory::Error,
                &format!("Failed to set process priority id={p}, error={e:?}"),
            );
            return;
        }
    }
    ui_log(
        LogCategory::Info,
        "Now running at ABOVE_NORMAL_PRIORITY_CLASS",
    );
}

#[cfg(target_os = "linux")]
pub fn raise_priority() {
    // the following only works when you're root on Linux
    // or if you give the program CAP_SYS_NICE (cf. setcap)
    // or are a user of the pipewire group
    use rustix::process::{getpriority_process, setpriority_process};
    if let Ok(pri) = getpriority_process(None) {
        if pri >= 0 {
            if setpriority_process(None, -10).is_err() {
                ui_log(
                    LogCategory::Warning,
                    "Sorry, but you don't have permissions to raise priority...",
                );
            } else {
                ui_log(LogCategory::Info, "Now running at nice value -10");
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn raise_priority() {}
