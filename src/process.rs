//! Process queries used by `capture status` (what v1 got from psutil)

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub uptime: Duration,
    pub memory_bytes: u64,
    pub cpu_percent: f64,
}

/// Python's str(timedelta) without microseconds: "H:MM:SS" or "N day(s), H:MM:SS"
pub fn format_uptime(uptime: Duration) -> String {
    let secs = uptime.as_secs();
    let (days, rest) = (secs / 86400, secs % 86400);
    let hms = format!("{}:{:02}:{:02}", rest / 3600, (rest % 3600) / 60, rest % 60);
    match days {
        0 => hms,
        1 => format!("1 day, {hms}"),
        d => format!("{d} days, {hms}"),
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, STILL_ACTIVE};
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn open(pid: u32, access: u32) -> Option<Handle> {
        let h = unsafe { OpenProcess(access, 0, pid) };
        (!h.is_null()).then_some(Handle(h))
    }

    fn ticks(ft: &FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    }

    /// (creation time, kernel + user CPU time), in 100 ns units
    fn times(h: &Handle) -> Option<(u64, u64)> {
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let ok = unsafe { GetProcessTimes(h.0, &mut created, &mut exited, &mut kernel, &mut user) };
        (ok != 0).then(|| (ticks(&created), ticks(&kernel) + ticks(&user)))
    }

    pub fn pid_exists(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        let Some(h) = open(pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
            // Access denied still means the process exists
            return std::io::Error::last_os_error().raw_os_error() == Some(5);
        };
        let mut code: u32 = 0;
        unsafe { GetExitCodeProcess(h.0, &mut code) != 0 && code == STILL_ACTIVE as u32 }
    }

    /// Full path of the process executable, when it can be queried
    pub fn image_path(pid: u32) -> Option<std::path::PathBuf> {
        let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
        let mut buf = vec![0u16; 32768];
        let mut size = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size) };
        (ok != 0).then(|| std::path::PathBuf::from(String::from_utf16_lossy(&buf[..size as usize])))
    }

    /// Uptime, working set and CPU usage measured over `interval` (like psutil's cpu_percent)
    pub fn process_info(pid: u32, interval: Duration) -> Option<ProcessInfo> {
        let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ)
            .or_else(|| open(pid, PROCESS_QUERY_LIMITED_INFORMATION))?;
        let (created, cpu1) = times(&h)?;
        let start = Instant::now();
        std::thread::sleep(interval);
        let (_, cpu2) = times(&h)?;
        let elapsed = start.elapsed().as_secs_f64();
        let cpu_percent = if elapsed > 0.0 { (cpu2 - cpu1) as f64 / 1e7 / elapsed * 100.0 } else { 0.0 };

        let mut now = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        unsafe { GetSystemTimeAsFileTime(&mut now) };
        let uptime = Duration::from_nanos(ticks(&now).saturating_sub(created) * 100);

        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory_bytes = if unsafe { K32GetProcessMemoryInfo(h.0, &mut counters, counters.cb) } != 0 {
            counters.WorkingSetSize as u64
        } else {
            0
        };
        Some(ProcessInfo { uptime, memory_bytes, cpu_percent: (cpu_percent * 10.0).round() / 10.0 })
    }
}

#[cfg(windows)]
pub use win::{image_path, pid_exists, process_info};

#[cfg(not(windows))]
pub fn image_path(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// A running process that is a WinLLDP executable (a stale PID file may point to
/// a PID that Windows has since given to another program)
pub fn is_winlldp(pid: u32) -> bool {
    if !pid_exists(pid) {
        return false;
    }
    match image_path(pid) {
        Some(path) => path.file_stem().is_some_and(|stem| stem.to_string_lossy().to_lowercase().starts_with("winlldp")),
        // Not allowed to query it: trust the PID file, like v1
        None => true,
    }
}

#[cfg(not(windows))]
pub fn pid_exists(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(windows))]
pub fn process_info(_pid: u32, _interval: Duration) -> Option<ProcessInfo> {
    let _ = Instant::now();
    None
}

/// Forcefully end a process (and its children), like v1's `taskkill /F /PID`
pub fn kill(pid: u32) {
    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_like_python_timedelta() {
        assert_eq!(format_uptime(Duration::from_secs(5)), "0:00:05");
        assert_eq!(format_uptime(Duration::from_secs(3 * 3600 + 7 * 60 + 9)), "3:07:09");
        assert_eq!(format_uptime(Duration::from_secs(86400 + 61)), "1 day, 0:01:01");
        assert_eq!(format_uptime(Duration::from_secs(2 * 86400)), "2 days, 0:00:00");
    }

    #[test]
    fn only_winlldp_processes_are_the_capture() {
        // The test binary is not called winlldp*: a PID file pointing to it is stale
        let me = std::process::id();
        let stem = std::env::current_exe().unwrap().file_stem().unwrap().to_string_lossy().to_lowercase();
        assert_eq!(is_winlldp(me), stem.starts_with("winlldp"));
        assert!(!is_winlldp(0));
    }

    #[test]
    fn current_process_exists() {
        assert!(pid_exists(std::process::id()));
        assert!(!pid_exists(0));
    }

    #[cfg(windows)]
    #[test]
    fn current_process_info() {
        let info = process_info(std::process::id(), Duration::from_millis(50)).unwrap();
        assert!(info.memory_bytes > 0);
        assert!(info.uptime < Duration::from_secs(3600));
    }
}
