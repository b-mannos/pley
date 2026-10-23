use std::fs::{File, OpenOptions};
use std::io::IsTerminal;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use tracing::Metadata;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::writer::EitherWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

/// Start a new log file once the previous run's log is larger than this.
const ROTATE_BYTES: u64 = 10 * 1024 * 1024;

/// Target of the events that report panics (see [`install_panic_hook`]).
const PANIC_TARGET: &str = "pley::panic";

/// Cleared when the process leaves its console window (the Windows tray
/// does); log lines then only go to the log file.
static CONSOLE: AtomicBool = AtomicBool::new(true);

/// Stop writing log lines to the console. Call before detaching from it.
pub fn console_detached() {
    CONSOLE.store(false, Ordering::SeqCst);
}

/// Standard error while there is a console, nothing after.
struct Console;

impl<'a> MakeWriter<'a> for Console {
    type Writer = EitherWriter<std::io::Stderr, std::io::Sink>;

    fn make_writer(&'a self) -> Self::Writer {
        if CONSOLE.load(Ordering::Relaxed) {
            EitherWriter::A(std::io::stderr())
        } else {
            EitherWriter::B(std::io::sink())
        }
    }

    /// Panics are left out: Rust's own message is printed there already.
    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        if meta.target() == PANIC_TARGET { EitherWriter::B(std::io::sink()) } else { self.make_writer() }
    }
}

/// Initialise tracing and log panics (see [`install_panic_hook`]). `filter`
/// uses `EnvFilter` syntax; a bare level such as `info` keeps noisy
/// dependencies at `warn`. With `file`, logs also go to that file (the
/// server's, so they survive without a console window).
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
        .with_writer(Console)
        .compact();
    let file = file.and_then(open_log).map(|f| {
        fmt::layer().with_target(false).with_ansi(false).log_internal_errors(false).with_writer(Mutex::new(f)).compact()
    });
    let _ = tracing_subscriber::registry().with(env_filter).with(stderr).with(file).try_init();
    install_panic_hook();
}

/// Log panics through tracing: the log file gets the message, where it
/// happened and a backtrace, then the previous hook prints Rust's usual
/// message to the console. Once the tray has detached from the console,
/// standard error leads nowhere and the log file is the only trace.
fn install_panic_hook() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = info.payload();
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("Box<dyn Any>");
            let location = info.location().map_or_else(|| "an unknown place".to_owned(), ToString::to_string);
            let thread = std::thread::current();
            let backtrace = std::backtrace::Backtrace::force_capture();
            tracing::error!(
                target: PANIC_TARGET,
                "thread '{}' panicked at {location}: {message}\nbacktrace:\n{backtrace}",
                thread.name().unwrap_or("<unnamed>")
            );
            previous(info);
        }));
    });
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
