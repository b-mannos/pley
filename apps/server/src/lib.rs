//! pley — a fast, self-hosted media server.
//!
//! The crate is organised as a library so integration tests can build the full
//! application in-process; `main.rs` only parses the command line.

pub mod access;
pub mod accounts;
pub mod api;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod db;
pub mod desktop;
pub mod error;
pub mod events;
pub mod images;
pub mod library;
pub mod logging;
pub mod markers;
pub mod metadata;
pub mod overrides;
pub mod people;
pub mod playback;
pub mod plex;
pub mod ratings;
pub mod settings;
pub mod state;
pub mod streams;
pub mod transcoder;
pub mod trickplay;
pub mod util;
pub mod web;

use std::time::Duration;

pub use config::Config;
pub use state::AppState;

/// Woken by [`request_shutdown`] (the tray's "Quit").
static SHUTDOWN: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Ask the running server to shut down gracefully.
pub fn request_shutdown() {
    SHUTDOWN.notify_one();
}

/// `pley serve`: run the server on a new runtime, with a tray icon when `tray`
/// is set and this is an interactive desktop session.
pub fn serve(config: Config, tray: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("pley-worker").build()?;
    let tray = tray && desktop::interactive_session();
    #[cfg(all(feature = "tray", target_os = "macos"))]
    if tray {
        // AppKit owns the main thread; the server runs on a helper thread.
        return desktop::macos::serve(runtime, config);
    }
    let launcher = tray.then(|| Box::new(desktop::start_tray) as desktop::TrayLauncher);
    runtime.block_on(run_with_tray(config, launcher))
}

/// Build the application state, start background services and serve HTTP until
/// Ctrl+C / SIGTERM.
pub async fn run(config: Config) -> anyhow::Result<()> {
    run_with_tray(config, None).await
}

/// Like [`run`]; once the server listens, `tray` gets the context for the tray
/// icon (if any), which is removed again after shutdown.
pub async fn run_with_tray(config: Config, tray: Option<desktop::TrayLauncher>) -> anyhow::Result<()> {
    let bind = config.bind;
    let state = AppState::init(config).await?;
    state.start_background_tasks();

    let app = api::router(state.clone());
    let listener =
        tokio::net::TcpListener::bind(bind).await.map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    tracing::info!("pley {} listening on http://{}", env!("CARGO_PKG_VERSION"), display_addr(bind));
    let plex = plex::start(&state).await;
    let tray = tray.and_then(|launch| launch(desktop::TrayContext::new(&state, bind)));

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shutting down");
    if let Some(plex) = plex {
        plex.stop().await;
    }
    if tokio::time::timeout(Duration::from_secs(25), state.shutdown()).await.is_err() {
        tracing::warn!("shutdown did not finish in time");
    }
    drop(tray);
    Ok(())
}

/// `pley reset-password <user> <password>`: recovery path for a lost admin password.
pub async fn reset_password(config: Config, username: &str, password: &str, admin: bool) -> anyhow::Result<()> {
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
            println!("password updated for '{username}'");
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

fn display_addr(addr: std::net::SocketAddr) -> String {
    if addr.ip().is_unspecified() { format!("localhost:{}", addr.port()) } else { addr.to_string() }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = SHUTDOWN.notified() => {},
    }
}
