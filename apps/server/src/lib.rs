//! pley — a fast, self-hosted media server.
//!
//! The crate is organised as a library so integration tests can build the full
//! application in-process; `main.rs` only parses the command line.

pub mod access;
pub mod accounts;
pub mod api;
pub mod auth;
pub mod backup;
pub mod catalog;
pub mod collections;
pub mod config;
pub mod db;
pub mod desktop;
pub mod discovery;
pub mod error;
pub mod events;
pub mod history;
pub mod http;
pub mod images;
pub mod instance;
pub mod labels;
pub mod library;
pub mod logging;
pub mod markers;
pub mod metadata;
pub mod oauth;
pub mod overrides;
pub mod people;
pub mod playback;
pub mod playlists;
pub mod plex;
pub mod providers;
pub mod ratings;
pub mod related;
pub mod settings;
pub mod setup;
pub mod state;
pub mod store;
pub mod streams;
pub mod transcoder;
pub mod trickplay;
pub mod two_step;
pub mod util;
pub mod web;

use std::net::SocketAddr;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

pub use config::Config;
pub use state::AppState;

/// Woken by [`request_shutdown`].
static SHUTDOWN: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// How long stopping the runtime waits for blocking work that is still
/// running, such as a library walk on a slow network share.
const RUNTIME_STOP: Duration = Duration::from_secs(3);

/// Ask the running server to shut down gracefully.
pub fn request_shutdown() {
    SHUTDOWN.notify_one();
}

/// How long a server that started shutting down may take before the process
/// ends anyway. Each step keeps to a limit of its own (connections,
/// background work, the transcoder, the database, the tray icon), so this
/// only catches one that doesn't: a lost message to the tray once kept pley
/// running, its icon still there, after Quit.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(45);

/// Whether the server started shutting down, for [`watch_shutdown`].
static SHUTTING_DOWN: parking_lot::Mutex<bool> = parking_lot::const_mutex(false);
static SHUTTING_DOWN_CHANGED: parking_lot::Condvar = parking_lot::Condvar::new();

fn set_shutting_down(shutting_down: bool) {
    *SHUTTING_DOWN.lock() = shutting_down;
    SHUTTING_DOWN_CHANGED.notify_all();
}

/// Ends the process when a shutdown isn't over [`SHUTDOWN_DEADLINE`] after
/// it started. For `pley serve` only: servers that tests run share their
/// process.
fn watch_shutdown() {
    let _ = std::thread::Builder::new().name("pley-shutdown-watch".into()).spawn(|| {
        let mut started = SHUTTING_DOWN.lock();
        while !*started {
            SHUTTING_DOWN_CHANGED.wait(&mut started);
        }
        drop(started);
        std::thread::sleep(SHUTDOWN_DEADLINE);
        tracing::error!("pley did not stop within {}s of being asked to: ending it", SHUTDOWN_DEADLINE.as_secs());
        std::process::exit(0);
    });
}

/// Whether the server finished shutting down (its database is closed), see
/// [`wait_until_stopped`].
static STOPPED: parking_lot::Mutex<bool> = parking_lot::const_mutex(false);
static STOPPED_CHANGED: parking_lot::Condvar = parking_lot::Condvar::new();

fn set_stopped(stopped: bool) {
    *STOPPED.lock() = stopped;
    STOPPED_CHANGED.notify_all();
}

/// Blocks the calling thread until the server has shut down, for at most
/// `timeout`; whether it has. For Windows, which may end the process as soon
/// as it has told it that the session ends.
#[cfg_attr(not(all(feature = "tray", windows)), allow(dead_code))]
pub(crate) fn wait_until_stopped(timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    let mut stopped = STOPPED.lock();
    while !*stopped {
        if STOPPED_CHANGED.wait_until(&mut stopped, deadline).timed_out() {
            break;
        }
    }
    *stopped
}

/// `pley serve`: run the server on a new runtime, with a tray icon when `tray`
/// is set and this is an interactive desktop session.
pub fn serve(config: Config, tray: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("pley-worker").build()?;
    watch_shutdown();
    let tray = tray && desktop::interactive_session();
    #[cfg(all(feature = "tray", target_os = "macos"))]
    if tray {
        // AppKit owns the main thread; the server runs on a helper thread.
        return desktop::macos::serve(runtime, config);
    }
    let launcher = tray.then(|| Box::new(desktop::start_tray) as desktop::TrayLauncher);
    run_on(runtime, config, launcher)
}

/// Run the server on `runtime`, then stop the runtime: tasks still running
/// (connections that outlived the grace period) are dropped, and blocking
/// work gets [`RUNTIME_STOP`]. Dropping the runtime instead would wait for
/// that work however long it takes.
pub(crate) fn run_on(
    runtime: tokio::runtime::Runtime,
    config: Config,
    tray: Option<desktop::TrayLauncher>,
) -> anyhow::Result<()> {
    let result = runtime.block_on(run_with_tray(config, tray));
    runtime.shutdown_timeout(RUNTIME_STOP);
    result
}

/// Build the application state, start background services and serve HTTP until
/// Ctrl+C, SIGTERM or [`request_shutdown`].
pub async fn run(config: Config) -> anyhow::Result<()> {
    run_with_tray(config, None).await
}

/// Like [`run`]; once the server listens, `tray` gets the context for the tray
/// icon (if any), which is removed again after shutdown. The tray's Quit stops
/// this server.
///
/// Before the database is opened, the data directory is locked (one server
/// per data directory, see [`instance`]) and both ports are bound: a second
/// copy, or one whose port is taken, stops without touching anything.
///
/// On shutdown the listeners close, the event streams end, and requests in
/// progress get [`Config::shutdown_grace`] to finish; connections still open
/// after that (a film playing directly) are left to the runtime's shutdown
/// (see [`serve`]).
pub async fn run_with_tray(config: Config, tray: Option<desktop::TrayLauncher>) -> anyhow::Result<()> {
    let _instance = instance::InstanceLock::acquire(&config.data_dir)?;
    set_stopped(false);
    set_shutting_down(false);
    let bind = config.bind;
    let listener =
        tokio::net::TcpListener::bind(bind).await.map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    let addr = listener.local_addr()?;
    let plex = plex::bind(config.plex_bind).await;
    let discovery = if config.discovery { discovery::bind(addr).await } else { None };

    let grace = config.shutdown_grace;
    let state = AppState::init(config).await?;
    state.start_background_tasks();
    let app = api::router(state.clone());
    tracing::info!("pley {} listening on http://{}", env!("CARGO_PKG_VERSION"), display_addr(addr));
    if let Some(code) = state.auth.setup.shown() {
        tracing::warn!(
            "pley isn't set up yet: open http://{}/setup#code={code} to create the administrator account \
             (the setup code is {code})",
            display_addr(addr)
        );
    }
    let tray = tray.and_then(|launch| launch(desktop::TrayContext::new(&state, addr)));

    let stopping = state.stopping.clone();
    let web = http::serve(listener, app, stopping.clone().cancelled_owned());
    let plex = plex.map(|listener| plex::serve(state.clone(), listener, stopping.clone().cancelled_owned()));
    if let Some(socket) = discovery {
        tokio::spawn(discovery::serve(state.clone(), socket, addr.port(), stopping.clone()));
    }
    let serving = async move {
        let plex = async move {
            if let Some(plex) = plex {
                plex.await;
            }
        };
        tokio::join!(web, plex);
    };
    tokio::pin!(serving);
    let stopped = tokio::select! {
        () = &mut serving => true,
        () = shutdown_signal(&stopping) => false,
    };
    tracing::info!("shutting down");
    set_shutting_down(true);
    stopping.cancel();
    if !stopped && tokio::time::timeout(grace, serving).await.is_err() {
        tracing::warn!(
            "not waiting any longer for connections still open {}s after the shutdown signal (a film playing?)",
            grace.as_secs_f32()
        );
    }
    if tokio::time::timeout(Duration::from_secs(25), state.shutdown()).await.is_err() {
        tracing::warn!("shutdown did not finish in time");
    }
    set_stopped(true);
    drop(tray);
    Ok(())
}

/// `pley reset-password <user> <password>`: recovery path for a lost admin password.
pub async fn reset_password(config: Config, username: &str, password: &str, admin: bool) -> anyhow::Result<()> {
    auth::validate_password(password, username).map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::create_dir_all(&config.data_dir)?;
    let db = db::Db::open(&config.db_path()).await?;
    let hash = auth::hash_password(password.to_owned()).await?;
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = ?").bind(username).fetch_optional(&db.read).await?;
    match existing {
        Some(id) => {
            sqlx::query("UPDATE users SET password_hash = ?, is_admin = MAX(is_admin, ?) WHERE id = ?")
                .bind(&hash)
                .bind(admin)
                .bind(id)
                .execute(&db.write)
                .await?;
            sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&db.write).await?;
            // Whoever runs this owns the server: an authenticator lost with
            // the password doesn't keep them out.
            let two_step = sqlx::query("DELETE FROM two_step WHERE user_id = ?")
                .bind(id)
                .execute(&db.write)
                .await?
                .rows_affected();
            sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?").bind(id).execute(&db.write).await?;
            let off = if two_step > 0 { "; two-step sign-in is off" } else { "" };
            println!("password updated for '{username}'{off}");
        }
        None => {
            sqlx::query("INSERT INTO users (username, password_hash, is_admin, created_at) VALUES (?, ?, ?, ?)")
                .bind(username)
                .bind(&hash)
                .bind(admin)
                .bind(util::now())
                .execute(&db.write)
                .await?;
            println!("created user '{username}'{}", if admin { " (admin)" } else { "" });
        }
    }
    db.close().await;
    Ok(())
}

/// `pley health`: ask the local server for `/api/health`.
pub async fn health(config: &Config) -> anyhow::Result<()> {
    let mut addr = config.bind;
    if addr.ip().is_unspecified() {
        addr.set_ip(std::net::Ipv4Addr::LOCALHOST.into());
    }
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?
        .get(format!("http://{addr}/api/health"))
        .send()
        .await?;
    anyhow::ensure!(response.status().is_success(), "server answered {}", response.status());
    Ok(())
}

fn display_addr(addr: SocketAddr) -> String {
    if addr.ip().is_unspecified() { format!("localhost:{}", addr.port()) } else { addr.to_string() }
}

/// Ctrl+C, SIGTERM, [`request_shutdown`] or `stopping` cancelled (the tray's
/// Quit). On Windows also Ctrl+Break, the console window closing and, for a
/// service, the system shutting down: Windows waits for pley to stop then
/// (a few seconds at most), as tokio holds those events until the process
/// exits. Not a sign-out: a service gets that when anyone signs out. A
/// signal that can't be watched is ignored rather than taken as a request to
/// stop.
async fn shutdown_signal(stopping: &CancellationToken) {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(windows)]
    let terminate = async {
        use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_shutdown};
        match (ctrl_break(), ctrl_close(), ctrl_shutdown()) {
            (Ok(mut brk), Ok(mut close), Ok(mut shutdown)) => {
                tokio::select! {
                    _ = brk.recv() => {},
                    _ = close.recv() => {},
                    _ = shutdown.recv() => {},
                }
            }
            _ => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(any(unix, windows)))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
        () = SHUTDOWN.notified() => {},
        () = stopping.cancelled() => {},
    }
}
