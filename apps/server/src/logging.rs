use std::fs::{File, OpenOptions};
use std::io::IsTerminal;
use std::path::Path;
use std::sync::Mutex;

use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

/// Start a new log file once the previous run's log is larger than this.
const ROTATE_BYTES: u64 = 10 * 1024 * 1024;

/// Initialise tracing. `filter` uses `EnvFilter` syntax; a bare level such as
/// `info` keeps noisy dependencies at `warn`. With `file`, logs also go to that
/// file (the server's, so they survive without a console window).
pub fn init(filter: &str, file: Option<&Path>) {
    let filter = if filter.contains('=') || filter.contains(',') {
        filter.to_owned()
    } else {
        format!("{filter},sqlx=warn,hyper=warn,tower_http=warn,reqwest=warn,zbus=warn")
    };
    let env_filter = EnvFilter::try_new(&filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = fmt::layer()
        .with_target(false)
        .with_ansi(std::io::stderr().is_terminal())
        .log_internal_errors(false)
        .with_writer(std::io::stderr)
        .compact();
    let file = file.and_then(open_log).map(|f| {
        fmt::layer().with_target(false).with_ansi(false).log_internal_errors(false).with_writer(Mutex::new(f)).compact()
    });
    let _ = tracing_subscriber::registry().with(env_filter).with(stderr).with(file).try_init();
}

/// Append to `path`, keeping the previous log as `<name>.1` when it got large.
fn open_log(path: &Path) -> Option<File> {
    std::fs::create_dir_all(path.parent()?).ok()?;
    if std::fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        let mut previous = path.as_os_str().to_owned();
        previous.push(".1");
        let _ = std::fs::rename(path, previous);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}
