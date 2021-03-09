use crate::ui_log;

#[cfg(target_os = "windows")]
pub fn raise_priority() {
    use std::os::windows::raw::HANDLE;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::processthreadsapi::{GetCurrentProcess, GetCurrentProcessId, SetPriorityClass};
    unsafe {
        const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 32768;
        let id = GetCurrentProcess() as HANDLE;
        if SetPriorityClass(id, ABOVE_NORMAL_PRIORITY_CLASS) == 0 {
            let e = GetLastError();
            ui_log(format!(
                "*E*E*>Failed to set process priority id={}, error={}",
                GetCurrentProcessId(),
                e
            ));
        }
    }
    ui_log("Now running at ABOVE_NORMAL_PRIORITY_CLASS".to_string());
}

#[cfg(target_os = "linux")]
pub fn raise_priority() {
    // the following only works when you're root on Linux
    // or if you give the program CAP_SYS_NICE (cf. setcap)
    use libc::{getpriority, setpriority, PRIO_PROCESS};
    unsafe {
        let pri = getpriority(PRIO_PROCESS, 0);
        let newpri = pri - 5;
        let rc = setpriority(PRIO_PROCESS, 0, newpri);
        if rc != 0 {
            log("Sorry, but you don't have permissions to raise priority...".to_string());
        } else {
            log(format!("Now running at nice value {}", newpri));
        }
    }
}

#[cfg(target_os = "macos")]
pub fn raise_priority() {}
