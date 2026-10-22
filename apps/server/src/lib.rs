//! pley — a fast, self-hosted media server.
//!
//! The crate is organised as a library so integration tests can build the full
//! application in-process; `main.rs` only parses the command line.

pub mod api;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod images;
pub mod library;
pub mod logging;
pub mod metadata;
pub mod playback;
pub mod settings;
pub mod state;
pub mod transcoder;
pub mod util;
pub mod web;

use std::time::Duration;

pub use config::Config;
pub use state::AppState;

/// Build the application state, start background services and serve HTTP until
/// Ctrl+C / SIGTERM.
pub async fn run(config: Config) -> anyhow::Result<()> {
    let bind = config.bind;
    let state = AppState::init(config).await?;
    state.start_background_tasks();

    let app = api::router(state.clone());
    let listener =
        tokio::net::TcpListener::bind(bind).await.map_err(|e| anyhow::anyhow!("cannot listen on {bind}: {e}"))?;
    tracing::info!("pley {} listening on http://{}", env!("CARGO_PKG_VERSION"), display_addr(bind));

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shutting down");
    if tokio::time::timeout(Duration::from_secs(25), state.shutdown()).await.is_err() {
        tracing::warn!("shutdown did not finish in time");
    }
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
    }
}
