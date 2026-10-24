//! HTTP API (`/api/*`, see docs/API.md) plus the web UI fallback.

mod accounts;
mod activity;
mod admin;
mod auth;
mod browse;
pub(crate) mod collections;
pub(crate) mod editing;
mod history;
pub(crate) mod items;
mod libraries;
mod people;
pub(crate) mod playback;
mod playlists;
mod two_step;

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, header};
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
        .route("/me/two-step", get(two_step::status).post(two_step::enable).delete(two_step::disable))
        .route("/me/two-step/setup", post(two_step::setup))
        .route("/me/two-step/recovery-codes", post(two_step::recovery_codes))
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
        .route("/libraries/order", put(libraries::reorder))
        .route("/libraries/{id}", get(libraries::get).patch(libraries::update).delete(libraries::remove))
        .route("/libraries/{id}/scan", post(libraries::scan))
        .route("/libraries/{id}/items", get(libraries::items))
        .route("/libraries/{id}/filters", get(libraries::filters))
        .route("/libraries/{id}/hubs", get(libraries::hubs))
        .route("/libraries/{id}/collections", get(collections::list).post(collections::create))
        .route("/collections/{id}", get(collections::get).patch(collections::update).delete(collections::delete))
        .route("/collections/{id}/items", post(collections::add_items))
        .route("/collections/{id}/items/{item_id}", delete(collections::remove_item))
        .route("/collections/{id}/order", put(collections::reorder))
        .route("/collections/{id}/images/{kind}", get(collections::image))
        .route("/playlists", get(playlists::list).post(playlists::create))
        .route("/playlists/{id}", get(playlists::get).patch(playlists::rename).delete(playlists::delete))
        .route("/playlists/{id}/items", post(playlists::add))
        .route("/playlists/{id}/items/{entry_id}", delete(playlists::remove))
        .route("/playlists/{id}/order", put(playlists::reorder))
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
        .route("/items/{id}/related", get(items::related))
        .route("/items/{id}/prefetch", post(items::prefetch))
        .route("/items/{id}/progress", post(items::progress))
        .route("/items/{id}/watched", post(items::watched))
        .route("/items/{id}/streams", put(items::set_streams))
        .route("/items/{id}/rating", put(items::set_rating))
        .route("/items/{id}/unwatched", post(items::unwatched))
        .route("/items/{id}/refresh", post(items::refresh))
        .route("/items/{id}/analyze", post(items::analyze))
        .route("/items/{id}/images/{kind}", get(items::image))
        .route("/items/{id}/collections", get(collections::for_item))
        .route("/people/{id}", get(people::get))
        .route("/people/{id}/photo", get(people::photo))
        .route("/history", get(history::list))
        .route("/history/{id}", delete(history::delete))
        .route("/hubs/home", get(browse::home))
        .route("/search", get(browse::search))
        .route("/playback", post(playback::start))
        .route("/playback/{session}", get(playback::status).delete(playback::stop))
        .route("/playback/{session}/{file}", get(playback::hls_file))
        .route("/files/{id}/stream", get(playback::stream_file))
        .route("/files/{id}/subtitles/{file}", get(playback::subtitles))
        .route("/files/{id}/trickplay", get(playback::trickplay))
        .route("/files/{id}/trickplay/{sheet}", get(playback::trickplay_sheet))
        .route("/files/{id}/chapters/{image}", get(playback::chapter_image))
        .route("/activity", get(activity::activity))
        .route("/events", get(activity::events))
        .route("/users", get(accounts::list_users).post(accounts::create_user))
        .route("/users/{id}", patch(accounts::update_user).delete(accounts::delete_user))
        .route("/users/{id}/avatar", get(accounts::avatar))
        .route("/users/{id}/devices", get(accounts::user_devices).delete(accounts::sign_out_everywhere))
        .route("/users/{id}/two-step", delete(two_step::reset_user))
        .route("/invites", get(accounts::list_invites).post(accounts::create_invite))
        .route("/invites/{code}", get(accounts::check_invite).delete(accounts::delete_invite))
        .route("/invites/{code}/accept", post(accounts::accept_invite))
        .route("/system", get(admin::system))
        .route("/system/storage", get(admin::storage))
        .route("/system/backups", get(admin::backups).post(admin::create_backup))
        .route("/system/storage/clean", post(admin::clean_storage))
        .route("/transcoder", get(admin::transcoder_status))
        .route("/settings", get(admin::get_settings).put(admin::put_settings))
        .route("/settings/test", post(admin::test_service))
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
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        ))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("same-origin")))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cross-origin-opener-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(CompressionLayer::new().quality(CompressionLevel::Precise(4)).compress_when(compressible))
        .layer(TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
            tracing::debug_span!("http", method = %req.method(), uri = %loggable_uri(req.uri()))
        }))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60 * 30),
        ))
        .with_state(state)
}

/// What pages of the web app may load and do: only pley's own scripts and
/// styles (an injected script can't run, nor send what it finds elsewhere),
/// pictures from pley and TMDB (Fix Match, artwork), media and hls.js's
/// worker from pley or `blob:`, and no other site may frame them.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
    img-src 'self' data: blob: https://image.tmdb.org; media-src 'self' blob:; worker-src 'self' blob:; \
    connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; \
    frame-ancestors 'none'";

/// Query string helper: `"true"`/`"1"` → true.
pub(crate) fn truthy(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes")
}

/// The request URI for logs, with credentials in the query (`token`,
/// `X-Plex-Token`, `api_key`, and the `code` and `state` of a sign-in) left
/// out.
pub(crate) fn loggable_uri(uri: &axum::http::Uri) -> String {
    let Some(query) = uri.query() else { return uri.path().to_owned() };
    let query: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _))
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "token" | "x-plex-token" | "api_key" | "code" | "state"
                ) =>
            {
                format!("{key}=…")
            }
            _ => pair.to_owned(),
        })
        .collect();
    format!("{}?{}", uri.path(), query.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_is_one_header_line() {
        assert!(HeaderValue::from_static(CONTENT_SECURITY_POLICY).to_str().is_ok());
        assert!(!CONTENT_SECURITY_POLICY.contains("unsafe") && !CONTENT_SECURITY_POLICY.contains("  "));
    }

    #[test]
    fn logs_leave_credentials_out() {
        let uri: axum::http::Uri = "/api/files/3/stream?token=secret&t=10&X-Plex-Token=abc&api_key=k".parse().unwrap();
        assert_eq!(loggable_uri(&uri), "/api/files/3/stream?token=…&t=10&X-Plex-Token=…&api_key=…");
        let uri: axum::http::Uri = "/api/x?code=c0de&state=st4te".parse().unwrap();
        assert_eq!(loggable_uri(&uri), "/api/x?code=…&state=…");
        assert_eq!(loggable_uri(&"/api/items/1".parse().unwrap()), "/api/items/1");
    }
}
