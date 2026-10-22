//! Centralized path resolution: runtime files live next to the executable

use std::path::PathBuf;

/// Directory where runtime files are stored (the executable's directory)
pub fn runtime_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn neighbors_file() -> PathBuf {
    runtime_directory().join("neighbors.json")
}

pub fn pid_file() -> PathBuf {
    runtime_directory().join("capture.pid")
}

pub fn log_file() -> PathBuf {
    runtime_directory().join("winlldp_capture.log")
}

/// The application log (v1 wrote it to %TEMP%)
pub fn app_log_file() -> PathBuf {
    runtime_directory().join("winlldp_service.log")
}

pub fn service_log_file() -> PathBuf {
    runtime_directory().join("nssm_service.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_live_in_the_executable_directory() {
        let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(runtime_directory(), dir);
        assert_eq!(neighbors_file(), dir.join("neighbors.json"));
        assert_eq!(pid_file(), dir.join("capture.pid"));
        assert_eq!(log_file(), dir.join("winlldp_capture.log"));
        assert_eq!(service_log_file(), dir.join("nssm_service.log"));
    }
}
