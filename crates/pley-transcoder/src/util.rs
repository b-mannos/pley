// SPDX-License-Identifier: LicenseRef-Restricted
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

/// Stable cache key for a file: hash of path, size and modification time, so a
/// replaced file never hits stale caches.
pub fn file_key(path: &Path) -> std::io::Result<String> {
    let meta = std::fs::metadata(path)?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(meta.len().to_le_bytes());
    hasher.update(mtime.to_le_bytes());
    Ok(hex::encode(&hasher.finalize()[..16]))
}

/// Seconds with millisecond precision, the way ffmpeg expects time arguments.
pub fn secs(ms: i64) -> String {
    let ms = ms.max(0);
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// `file:` prefixed path so ffmpeg never interprets it as a protocol or option.
pub fn input_arg(path: &Path) -> OsString {
    let mut arg = OsString::from("file:");
    arg.push(path.as_os_str());
    arg
}

/// Resolve a program name through `PATH` (and `PATHEXT` on Windows).
pub fn resolve_executable(program: &Path) -> Option<PathBuf> {
    if program.components().count() > 1 || program.is_absolute() {
        return program.is_file().then(|| program.to_path_buf());
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(program);
        if candidate.is_file() && (!cfg!(windows) || candidate.extension().is_some()) {
            return Some(candidate);
        }
        for ext in &exts {
            let mut name = program.as_os_str().to_owned();
            name.push(ext);
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Apply platform tweaks to child processes: no console window on Windows.
pub fn configure_child(cmd: &mut tokio::process::Command) {
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Total size of all files below `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(_) => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                Err(_) => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_seconds() {
        assert_eq!(secs(0), "0.000");
        assert_eq!(secs(4000), "4.000");
        assert_eq!(secs(123_456), "123.456");
        assert_eq!(secs(-5), "0.000");
    }

    #[test]
    fn input_arg_is_prefixed() {
        assert_eq!(input_arg(Path::new("/m/a.mkv")), OsString::from("file:/m/a.mkv"));
    }
}
