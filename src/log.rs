use crate::config::data_dir;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 256 * 1024;

/// Appends one line to %APPDATA%\Halo\halo.log, starting over once it grows past 256 KB.
pub fn line(msg: &str) {
    let path = data_dir().join("halo.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::remove_file(&path);
    }
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{secs}] {msg}");
    }
}
