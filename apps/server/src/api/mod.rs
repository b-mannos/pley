//! HTTP API (`/api/*`, see docs/API.md) plus the web UI fallback.

mod accounts;
mod activity;
mod admin;
mod auth;
mod browse;
mod editing;
pub(crate) mod items;
mod libraries;
pub(crate) mod playback;

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, header};
use axum::routing::{delete, get, patch, post, put};
use tower_http::compression::predicate::{NotForContentType, Predicate, SizeAbove};
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(|| async { axum::Json(serde_json::json!({ "status": "ok" })) }))
        .route("/server", get(auth::server_status))
        .route("/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/password", put(auth::change_password))
        .route("/auth/profiles", get(auth::profiles))
        .route("/auth/switch", post(auth::switch))
        .route("/me", patch(accounts::update_me))
        .route("/me/pin", put(accounts::set_my_pin))
        .route(
            "/me/avatar",
            put(accounts::upload_avatar).delete(accounts::delete_avatar).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/me/devices", get(accounts::my_devices))
        .route("/me/devices/{id}", delete(accounts::sign_out_device))
        .route("/libraries", get(libraries::list).post(libraries::create))
        .route("/libraries/scan", post(libraries::scan_all))
        .route("/libraries/{id}", get(libraries::get).patch(libraries::update).delete(libraries::remove))
        .route("/libraries/{id}/scan", post(libraries::scan))
        .route("/libraries/{id}/items", get(libraries::items))
        .route("/libraries/{id}/filters", get(libraries::filters))
        .route("/libraries/{id}/hubs", get(libraries::hubs))
        .route("/items/{id}", get(items::get).patch(editing::edit))
        .route("/items/{id}/matches", get(editing::matches))
        .route("/items/{id}/match", post(editing::rematch))
        .route(
            "/items/{id}/artwork/{kind}",
            get(editing::artwork_options).put(editing::choose_artwork).delete(editing::reset_artwork),
        )
        .route(
            "/items/{id}/artwork/{kind}/upload",
            post(editing::upload_artwork).layer(DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
        .route("/items/{id}/children", get(items::children))
        .route("/items/{id}/on-deck", get(items::on_deck))
        .route("/items/{id}/next", get(items::next))
        .route("/items/{id}/progress", post(items::progress))
        .route("/items/{id}/watched", post(items::watched))
        .route("/items/{id}/streams", put(items::set_streams))
        .route("/items/{id}/unwatched", post(items::unwatched))
        .route("/items/{id}/refresh", post(items::refresh))
        .route("/items/{id}/analyze", post(items::analyze))
        .route("/items/{id}/images/{kind}", get(items::image))
        .route("/hubs/home", get(browse::home))
        .route("/search", get(browse::search))
        .route("/playback", post(playback::start))
        .route("/playback/{session}", delete(playback::stop))
        .route("/playback/{session}/{file}", get(playback::hls_file))
        .route("/files/{id}/stream", get(playback::stream_file))
        .route("/files/{id}/subtitles/{file}", get(playback::subtitles))
        .route("/files/{id}/trickplay", get(playback::trickplay))
        .route("/files/{id}/trickplay/{sheet}", get(playback::trickplay_sheet))
        .route("/activity", get(activity::activity))
        .route("/events", get(activity::events))
        .route("/users", get(accounts::list_users).post(accounts::create_user))
        .route("/users/{id}", patch(accounts::update_user).delete(accounts::delete_user))
        .route("/users/{id}/avatar", get(accounts::avatar))
        .route("/users/{id}/devices", get(accounts::user_devices).delete(accounts::sign_out_everywhere))
        .route("/invites", get(accounts::list_invites).post(accounts::create_invite))
        .route("/invites/{code}", get(accounts::check_invite).delete(accounts::delete_invite))
        .route("/invites/{code}/accept", post(accounts::accept_invite))
        .route("/system", get(admin::system))
        .route("/transcoder", get(admin::transcoder_status))
        .route("/settings", get(admin::get_settings).put(admin::put_settings))
        .route("/fs", get(admin::browse_fs))
        .fallback(|| async { AppError::not_found("no such endpoint") })
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")));

    // Compress text responses only (never media, never the SSE stream).
    let compressible = SizeAbove::new(1024)
        .and(NotForContentType::new("video/"))
        .and(NotForContentType::new("audio/"))
        .and(NotForContentType::new("image/"))
        .and(NotForContentType::new("text/event-stream"))
        .and(NotForContentType::new("application/octet-stream"));

    Router::new()
        .nest("/api", api)
        .fallback(crate::web::static_handler)
        .layer(CompressionLayer::new().quality(CompressionLevel::Precise(4)).compress_when(compressible))
        .layer(TraceLayer::new_for_http().make_span_with(
            |req: &axum::http::Request<_>| tracing::debug_span!("http", method = %req.method(), uri = %req.uri()),
        ))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60 * 30),
        ))
        .with_state(state)
}

/// Query string helper: `"true"`/`"1"` → true.
pub(crate) fn truthy(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes")
}
