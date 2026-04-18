use crate::utils::ui_logger::ui_log;
use rust_i18n::t;

#[cfg(target_os = "windows")]
pub fn raise_priority() {
    use windows::Win32::{
        Foundation::GetLastError,
        System::Threading::{
            GetCurrentProcess, GetCurrentProcessId, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
        },
    };
    unsafe {
        let id = GetCurrentProcess();
        if SetPriorityClass(id, ABOVE_NORMAL_PRIORITY_CLASS).is_err() {
            let e = GetLastError();
            let p = GetCurrentProcessId();
            ui_log(&format!(
                "*E*E*>{} id={p}, error={e:?}",
                t!("failed_set_process_priority")
            ));
        }
    }
    ui_log(&*t!("running_high_priority"));
}

#[cfg(target_os = "linux")]
pub fn raise_priority() {
    use libc::{getpriority, setpriority, PRIO_PROCESS};
    unsafe {
        let pri = getpriority(PRIO_PROCESS, 0);
        let newpri = pri - 5;
        let rc = setpriority(PRIO_PROCESS, 0, newpri);
        if rc != 0 {
            ui_log(&*t!("no_permission_raise_priority"));
        } else {
            ui_log(&format!("{} {newpri}", t!("now_running_nice_value")));
        }
    }
}

#[cfg(target_os = "macos")]
pub fn raise_priority() {}