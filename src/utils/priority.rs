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
                Error,
                &format!("Failed to set process priority id={p}, error={e:?}"),
            );
        }
    }
    ui_log("Now running at ABOVE_NORMAL_PRIORITY_CLASS");
}

#[cfg(target_os = "linux")]
pub fn raise_priority() {
    // the following only works when you're root on Linux
    // or if you give the program CAP_SYS_NICE (cf. setcap)
    // or are a user of the pipewire group
    use libc::{PRIO_PROCESS, getpriority, setpriority};
    unsafe {
        let pri = getpriority(PRIO_PROCESS, 0);
        if pri >= 0 {
            let rc = setpriority(PRIO_PROCESS, 0, -10);
            if rc != 0 {
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
