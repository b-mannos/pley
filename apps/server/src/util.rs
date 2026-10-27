use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// When this server process started (unix ms): part of tags that a newer
/// version may answer differently.
pub fn process_epoch() -> i64 {
    static EPOCH: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(now_ms)
}

/// `%XX` escapes decoded (and `+` as a space when `plus`); anything else is
/// kept as it is.
pub fn percent_decode(s: &str, plus: bool) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            (b'+', ..) if plus => {
                out.push(b' ');
                i += 1;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `a` and `b` are equal, taking as long whichever byte differs
/// (secrets compared with what someone sent).
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Current unix time in seconds.
pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default()
}

/// Current unix time in milliseconds.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

pub fn system_time_secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default()
}

/// The UTC date and time of unix time `secs`: year, month, day, hour,
/// minute and second.
pub fn utc(secs: i64) -> [i64; 6] {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    [year, month, day, rem / 3600, rem % 3600 / 60, rem % 60]
}

/// Keeps the folder `dir` to the account pley runs as (on Unix, mode 0700
/// whatever the umask): pley's data holds password hashes, sessions,
/// backups and the log. On Windows, a user's own folders are private already.
pub fn keep_private(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let open = std::fs::metadata(dir).is_ok_and(|meta| meta.permissions().mode() & 0o077 != 0);
        if open {
            if let Err(err) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)) {
                tracing::warn!(
                    "other users of this computer can read {}, and pley can't change that: {err}",
                    dir.display()
                );
            }
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Lossy UTF-8 string of a path (paths are stored as text in the database).
/// Scans leave out names that are not valid UTF-8 (library/walk.rs), so the
/// paths of library files convert back exactly.
pub fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Canonical form used for identity keys: lowercase alphanumerics separated by
/// single spaces. `"The.Office (US)!"` → `"the office us"`.
pub fn normalize_key(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.extend(ch.to_lowercase());
        } else if ch == '\'' || ch == '\u{2019}' {
            // "Grey's" and "Greys" should match
        } else {
            pending_space = true;
        }
    }
    out
}

/// Sort key: lowercase without leading English articles, so "The Matrix" sorts under M.
pub fn sort_title(title: &str) -> String {
    let lower = title.trim().to_lowercase();
    for article in ["the ", "a ", "an "] {
        if let Some(rest) = lower.strip_prefix(article)
            && !rest.trim().is_empty()
        {
            return rest.trim_start().to_owned();
        }
    }
    lower
}

/// Run a blocking closure on the blocking pool and flatten the join error.
pub async fn blocking<T, F>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| anyhow::anyhow!("blocking task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn the_data_folder_is_kept_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        keep_private(dir.path());
        let mode = std::fs::metadata(dir.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn normalizes_keys() {
        assert_eq!(normalize_key("The.Office (US)!"), "the office us");
        assert_eq!(normalize_key("  Grey's Anatomy "), "greys anatomy");
        assert_eq!(normalize_key("Amélie"), "amélie");
    }

    #[test]
    fn sort_titles_drop_articles() {
        assert_eq!(sort_title("The Matrix"), "matrix");
        assert_eq!(sort_title("A Quiet Place"), "quiet place");
        assert_eq!(sort_title("The"), "the");
        assert_eq!(sort_title("Annie Hall"), "annie hall");
    }
}
