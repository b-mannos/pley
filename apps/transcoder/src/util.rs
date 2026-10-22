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

/// Apply platform tweaks to child processes: no console window on Windows; on
/// Linux the child is killed when the transcoder dies without cleaning up.
pub fn configure_child(cmd: &mut tokio::process::Command) {
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as libc::pid_t;
        // SAFETY: the closure only makes async-signal-safe calls (prctl,
        // getppid, _exit) between fork and exec.
        unsafe {
            cmd.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // The transcoder may have exited before the signal was armed.
                if libc::getppid() != parent {
                    libc::_exit(1);
                }
                Ok(())
            });
        }
    }
}

/// Run a child at reduced CPU priority (background analysis that must never
/// slow down playback). Call after [`configure_child`].
pub fn background_priority(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: setpriority is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                Ok(())
            });
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = cmd;
}

/// Make every child process die with the transcoder, whatever ends it.
///
/// Windows: the transcoder joins a job object that kills all of its members
/// when its last handle closes, which happens when this process exits for any
/// reason (including a crash or a forced kill). ffmpeg and ffprobe inherit the
/// job. Linux uses a parent-death signal per child instead (see
/// [`configure_child`]).
pub fn kill_children_on_exit() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        // SAFETY: plain Win32 calls on a handle owned here. The job handle is
        // deliberately never closed: it must live as long as the process.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                tracing::warn!("could not create a job object: {}", std::io::Error::last_os_error());
                return;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0;
            if !configured || AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
                tracing::warn!(
                    "could not tie ffmpeg processes to the transcoder's lifetime: {}",
                    std::io::Error::last_os_error()
                );
                CloseHandle(job);
            }
        }
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
