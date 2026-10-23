//! One server per data directory. The server holds an exclusive lock on
//! `<data-dir>/pley.lock` while it runs, so a second copy (started at login
//! next to the tray's, say) stops before it migrates the database, scans or
//! starts a transcoder, whose start-up empties the transcode jobs folder of
//! the copy already running. The system releases the lock when the process
//! ends, however it ends.
//!
//! Commands that work next to a running server (`reset-password`, `health`)
//! don't take it: SQLite lets them share the database.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::Context;

/// Held for as long as the server runs.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Lock `data_dir` (created if missing), or explain who holds it.
    pub fn acquire(data_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_dir).with_context(|| format!("cannot create {}", data_dir.display()))?;
        let path = data_dir.join("pley.lock");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        if !try_lock(&file).with_context(|| format!("cannot lock {}", path.display()))? {
            let holder = std::fs::read_to_string(&path).ok().and_then(|pid| pid.trim().parse::<u32>().ok());
            anyhow::bail!(
                "pley is already running{} with the data directory {}",
                holder.map(|pid| format!(" (process {pid})")).unwrap_or_default(),
                data_dir.display()
            );
        }
        // Who holds the lock, for the message above; only informative.
        let _ = file.set_len(0).and_then(|()| write!(file, "{}", std::process::id()));
        Ok(Self { _file: file })
    }
}

/// Take an exclusive lock on `file` without waiting: `false` when another
/// process (or another handle of this one) holds it.
#[cfg(unix)]
fn try_lock(file: &File) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;
    // SAFETY: `flock` only takes a descriptor, which `file` keeps open.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == std::io::ErrorKind::WouldBlock { Ok(false) } else { Err(err) }
}

#[cfg(windows)]
fn try_lock(file: &File) -> std::io::Result<bool> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx};
    use windows_sys::Win32::System::IO::{OVERLAPPED, OVERLAPPED_0, OVERLAPPED_0_0};

    // Other processes can't read locked bytes either: lock one byte well past
    // the process id at the start of the file (as SQLite does).
    const OFFSET: u64 = 1 << 30;
    let mut at = OVERLAPPED {
        Anonymous: OVERLAPPED_0 {
            Anonymous: OVERLAPPED_0_0 { Offset: OFFSET as u32, OffsetHigh: (OFFSET >> 32) as u32 },
        },
        ..Default::default()
    };
    // SAFETY: the handle stays open while `file` lives, and `at` outlives the
    // call, which completes before returning (the handle is synchronous).
    let locked = unsafe {
        LockFileEx(file.as_raw_handle(), LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, 1, 0, &mut at)
    } != 0;
    if locked {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) { Ok(false) } else { Err(err) }
}

#[cfg(not(any(unix, windows)))]
fn try_lock(_file: &File) -> std::io::Result<bool> {
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_holder_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let first = InstanceLock::acquire(&data).expect("a free data directory");
        let err = InstanceLock::acquire(&data).expect_err("already held");
        let message = format!("{err:#}");
        assert!(message.contains("already running"), "{message}");
        assert!(message.contains(&format!("(process {})", std::process::id())), "{message}");
        assert!(message.contains(&data.display().to_string()), "{message}");
        drop(first);
        let _again = InstanceLock::acquire(&data).expect("released with its holder");
    }
}
