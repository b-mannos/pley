use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current unix time in seconds.
/// When this server process started (unix ms): part of tags that a newer
/// version may answer differently.
pub fn process_epoch() -> i64 {
    static EPOCH: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(now_ms)
}

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

/// Lossy UTF-8 string of a path (paths are stored as text in the database).
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
        if let Some(rest) = lower.strip_prefix(article) {
            if !rest.trim().is_empty() {
                return rest.trim_start().to_owned();
            }
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
