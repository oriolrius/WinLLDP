//! Logging: %TEMP%\winlldp_service.log plus the console (like v1's logging setup)

use chrono::Local;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);
static CONSOLE: AtomicBool = AtomicBool::new(false);

/// `console` prints INFO and above to stderr (DEBUG too when `verbose`)
pub fn init(verbose: bool, console: bool) {
    VERBOSE.store(verbose, Ordering::Relaxed);
    CONSOLE.store(console, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn log_file() -> std::path::PathBuf {
    std::env::temp_dir().join("winlldp_service.log")
}

fn log(level: &str, message: &str) {
    let asctime = Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
    let line = format!("{asctime} - WinLLDP - {level} - {message}");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file()) {
        let _ = writeln!(f, "{line}");
    }
    if CONSOLE.load(Ordering::Relaxed) {
        if verbose() {
            eprintln!("{line}");
        } else if level != "DEBUG" {
            eprintln!("{level}:WinLLDP:{message}");
        }
    }
}

pub fn info(message: &str) {
    log("INFO", message);
}

pub fn warning(message: &str) {
    log("WARNING", message);
}

pub fn error(message: &str) {
    log("ERROR", message);
}

pub fn debug(message: &str) {
    if verbose() {
        log("DEBUG", message);
    }
}
