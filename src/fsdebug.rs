//! File operations that log what they open in verbose mode (v1's file_debug)

use crate::logger;
use std::io;
use std::panic::Location;
use std::path::{Path, PathBuf};

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn traced<T>(path: &Path, mode: &str, caller: &Location, op: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    if !logger::verbose() {
        return op();
    }
    let abs = absolute(path);
    let file = Path::new(caller.file()).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    logger::debug(&format!("FILE_OPEN: {mode} '{}' from {file}:{}", abs.display(), caller.line()));
    let result = op();
    match &result {
        Ok(_) => logger::debug(&format!("FILE_OPEN_SUCCESS: {mode} '{}'", abs.display())),
        Err(e) => logger::debug(&format!("FILE_OPEN_ERROR: {mode} '{}' - {e}", abs.display())),
    }
    logger::debug(&format!("FILE_CLOSE: '{}'", abs.display()));
    result
}

#[track_caller]
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let path = path.as_ref();
    traced(path, "READ", Location::caller(), || std::fs::read_to_string(path))
}

#[track_caller]
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    traced(path, "WRITE", Location::caller(), || std::fs::write(path, contents))
}
