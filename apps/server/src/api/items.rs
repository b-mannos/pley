use std::collections::HashMap;
use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use pley_transcoder_api::ThumbnailRequest;
use serde::{Deserialize, Serialize};

use crate::auth::{AdminUser, AuthUser};
use crate::catalog::{self, ItemDetail, ItemRow, ItemSummary};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::library::scanner::remux_candidate;
use crate::state::AppState;
use crate::streams;
use crate::util::now;

pub(crate) async fn item_or_404(state: &AppState, user_id: i64, id: i64) -> ApiResult<ItemRow> {
    catalog::item(&state.db, user_id, id).await?.ok_or_else(|| AppError::not_found(format!("item {id} not found")))
}

pub async fn get(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<ItemDetail>> {
    let row = item_or_404(&state, user.id, id).await?;
    let scope = streams::scope(&row);
    let mut detail = catalog::item_detail(&state.db, row).await?;
    detail.locked_fields = crate::overrides::locked_fields(&state.db, id).await?;
    if !detail.files.is_empty() {
        let pref = streams::load(&state.db, user.id, scope).await?;
        for file in &mut detail.files {
            let selection = streams::resolve(pref.as_ref(), &file.streams);
            file.selected_audio = selection.audio;
            file.selected_subtitle = selection.subtitle;
        }
    }
    // Opening an item is a good moment to prepare seek previews and to index
    // keyframes so playback can remux.
    if matches!(detail.summary.kind.as_str(), "movie" | "episode") {
        for file in catalog::files_for_item(&state.db, detail.summary.id).await? {
            state.trickplay.request(&file);
        }
    }
    if state.transcoder.is_available() {
        for file in &detail.files {
            if remux_candidate(file.container.as_deref(), &file.streams) {
                let transcoder = state.transcoder.clone();
                let path = file.path.clone();
                tokio::spawn(async move {
                    let _ = transcoder.request_keyframes(&path).await;
                });
            }
        }
    }
    Ok(Json(detail))
}

#[derive(Deserialize)]
pub struct StreamChoice {
    /// File the indexes refer to (default: the item's first file).
    file_id: Option<i64>,
    audio_index: Option<i64>,
    /// `None` = subtitles off.
    subtitle_index: Option<i64>,
}

/// Remember the audio / subtitle tracks the user picked, for the whole show
/// when this is an episode.
pub async fn set_streams(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(choice): Json<StreamChoice>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let file = match choice.file_id {
        Some(file_id) => files.iter().find(|f| f.id == file_id),
        None => files.first(),
    }
    .ok_or_else(|| AppError::not_found("file not found"))?;
    let pref = streams::from_selection(&file.streams(), choice.audio_index, choice.subtitle_index);
    streams::save(&state.db, user.id, streams::scope(&row), &pref).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct ItemList {
    items: Vec<ItemSummary>,
}

pub async fn children(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<ItemList>> {
    item_or_404(&state, user.id, id).await?;
    let rows = catalog::children(&state.db, user.id, id).await?;
    Ok(Json(ItemList { items: rows.iter().map(ItemRow::to_summary).collect() }))
}

#[derive(Serialize)]
pub struct MaybeItem {
    item: Option<ItemSummary>,
}

pub async fn on_deck(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<MaybeItem>> {
    let row = item_or_404(&state, user.id, id).await?;
    let target = catalog::on_deck(&state.db, user.id, &row).await?;
    let item = match target {
        Some(id) => catalog::item(&state.db, user.id, id).await?.map(|r| r.to_summary()),
        None => None,
    };
    Ok(Json(MaybeItem { item }))
}

pub async fn next(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Json<MaybeItem>> {
    let row = item_or_404(&state, user.id, id).await?;
    let item = match catalog::next_item(&state.db, &row).await? {
        Some(id) => catalog::item(&state.db, user.id, id).await?.map(|r| r.to_summary()),
        None => None,
    };
    Ok(Json(MaybeItem { item }))
}

#[derive(Deserialize)]
pub struct Progress {
    position_ms: i64,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default = "playing")]
    state: String,
    #[serde(default)]
    session_id: Option<String>,
}

fn playing() -> String {
    "playing".into()
}

pub async fn progress(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<Progress>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    if !matches!(row.kind.as_str(), "movie" | "episode" | "track") {
        return Err(AppError::bad_request("progress can only be recorded for playable items"));
    }
    if !matches!(body.state.as_str(), "playing" | "paused" | "stopped" | "buffering") {
        return Err(AppError::bad_request("state must be playing, paused or stopped"));
    }
    let duration = body.duration_ms.filter(|d| *d > 0).or(row.duration_ms).unwrap_or(0);
    let device = crate::accounts::device_name(headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()));
    let became_watched =
        catalog::record_progress(&state.db, user.id, id, body.position_ms, duration, Some(&device)).await?;
    if let Some(session) = body.session_id.as_deref().and_then(|sid| state.sessions.get(sid)) {
        if session.user_id == user.id {
            session.update(body.position_ms, &body.state);
            if session.should_touch_transcoder() {
                if let Some(job) = session.job_id() {
                    let transcoder = state.transcoder.clone();
                    tokio::spawn(async move {
                        let _ = transcoder.touch_job(&job).await;
                    });
                }
            }
            if body.state == "stopped" {
                state.events.send(ServerEvent::SessionsChanged);
            }
        }
    }
    if became_watched {
        state.events.send(ServerEvent::LibraryChanged { library_id: row.library_id });
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Rating {
    rating: Option<f64>,
}

/// `PUT /api/items/{id}/rating`: the user's own rating, 0-10 (two points
/// per star), or `null` to clear it.
pub async fn set_rating(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<Rating>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    let rating = match body.rating {
        Some(value) => {
            Some(catalog::valid_rating(value).ok_or_else(|| AppError::bad_request("rating must be between 0 and 10"))?)
        }
        None => None,
    };
    catalog::set_rating(&state.db, user.id, id, rating).await?;
    state.events.send(ServerEvent::LibraryChanged { library_id: row.library_id });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn watched(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    catalog::set_watched(&state.db, user.id, id, &row.kind, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unwatched(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    catalog::set_watched(&state.db, user.id, id, &row.kind, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn refresh(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    if !state.metadata.enabled() {
        return Err(AppError::bad_request("add a TMDB API key in Settings → Server to fetch metadata"));
    }
    let metadata = state.metadata.clone();
    let events = state.events.clone();
    tokio::spawn(async move {
        match metadata.refresh_item(id).await {
            Ok(()) => events.send(ServerEvent::LibraryChanged { library_id: row.library_id }),
            Err(err) => tracing::warn!("metadata refresh for item {id} failed: {err:#}"),
        }
    });
    Ok(StatusCode::ACCEPTED)
}

/// Find intros and credits again: a movie, or the seasons of an episode,
/// season or show.
pub async fn analyze(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    if !state.markers.analyze_item(id, &row.kind).await? {
        return Err(AppError::bad_request("only movies, shows, seasons and episodes have intros and credits"));
    }
    Ok(StatusCode::ACCEPTED)
}

#[derive(sqlx::FromRow)]
struct ArtworkPaths {
    kind: String,
    poster: Option<String>,
    backdrop: Option<String>,
    thumb: Option<String>,
    parent_id: Option<i64>,
}

pub async fn image(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, kind)): Path<(i64, String)>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    if !matches!(kind.as_str(), "poster" | "backdrop" | "thumb") {
        return Err(AppError::not_found("unknown image kind"));
    }
    if !catalog::is_visible(&state.db, user.id, id).await? {
        return Err(AppError::not_found(format!("item {id} not found")));
    }
    let width = query.get("w").and_then(|w| w.parse::<u32>().ok()).filter(|w| *w > 0);
    let path = resolve_artwork(&state, id, &kind).await?;
    let image = state.images.load(&path, width).await?;
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(image.etag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let mut out = HeaderMap::new();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(image.content_type));
    // Versioned URLs (`?v=<updated_at>`) change whenever the artwork does.
    let cache = if query.contains_key("v") { "private, max-age=31536000, immutable" } else { "private, max-age=300" };
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(etag) = HeaderValue::from_str(&image.etag) {
        out.insert(header::ETAG, etag);
    }
    Ok((out, image.bytes).into_response())
}

/// Path of the artwork file, generating it (frame grab, embedded cover) or
/// falling back to the parent's artwork when the item has none.
async fn resolve_artwork(state: &AppState, id: i64, kind: &str) -> ApiResult<PathBuf> {
    let mut current = id;
    for _ in 0..3 {
        let row: ArtworkPaths =
            sqlx::query_as("SELECT kind, poster, backdrop, thumb, parent_id FROM items WHERE id = ?")
                .bind(current)
                .fetch_optional(&state.db.read)
                .await?
                .ok_or_else(|| AppError::not_found("item not found"))?;
        let existing = match kind {
            "poster" => row.poster.clone(),
            "backdrop" => row.backdrop.clone(),
            _ => row.thumb.clone(),
        };
        if let Some(path) = existing.and_then(|r| state.store.path(&r)).filter(|p| p.is_file()) {
            return Ok(path);
        }
        match (kind, row.kind.as_str()) {
            ("thumb", "movie" | "episode") => return generate_thumb(state, current).await,
            ("poster", "album") => {
                if let Some(path) = extract_cover(state, current).await? {
                    return Ok(path);
                }
                return Err(AppError::not_found("no artwork"));
            }
            ("poster", "season" | "episode" | "track") | ("backdrop", _) => match row.parent_id {
                Some(parent) => current = parent,
                None => break,
            },
            _ => break,
        }
    }
    Err(AppError::not_found("no artwork"))
}

/// A frame grab of the item's first file, kept in the file's media folder.
async fn generate_thumb(state: &AppState, item_id: i64) -> ApiResult<PathBuf> {
    let file = catalog::files_for_item(&state.db, item_id)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found("no media file"))?;
    let fingerprint = state.store.fingerprint_of(file.id, &file.path, file.fingerprint.as_deref()).await?;
    let dir = state.store.media_dir(&fingerprint).ok_or_else(|| AppError::not_found("no media folder"))?;
    let target = dir.join("thumb.jpg");
    if target.is_file() {
        return Ok(target);
    }
    let lock = state.images.generation_lock(file.id);
    let _guard = lock.lock().await;
    if target.is_file() {
        return Ok(target);
    }
    let hdr = file.media_info().and_then(|m| m.video().map(|v| v.dynamic_range().is_hdr())).unwrap_or(false);
    let bytes = state
        .transcoder
        .thumbnail(&ThumbnailRequest {
            path: file.path.clone(),
            time_ms: None,
            width: Some(640),
            duration_ms: file.duration_ms,
            hdr,
        })
        .await?;
    tokio::fs::create_dir_all(&dir).await?;
    let tmp = dir.join("thumb.jpg.tmp");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &target).await?;
    state.images.release_generation_lock(file.id);
    Ok(target)
}

/// The cover embedded in an album's tracks, stored as the album's poster.
async fn extract_cover(state: &AppState, album_id: i64) -> ApiResult<Option<PathBuf>> {
    let lock = state.images.generation_lock(album_id);
    let _guard = lock.lock().await;
    // Another request may have stored it meanwhile.
    let poster: Option<String> =
        sqlx::query_scalar("SELECT poster FROM items WHERE id = ?").bind(album_id).fetch_one(&state.db.read).await?;
    if let Some(path) = poster.and_then(|p| state.store.path(&p)).filter(|p| p.is_file()) {
        return Ok(Some(path));
    }
    // Albums without embedded art are not opened again (until the cache is cleared).
    let marker = state.config.cache_dir().join("covers").join(format!("{album_id}.none"));
    if marker.exists() {
        return Ok(None);
    }
    let tracks: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT f.path, f.media_info FROM media_files f JOIN items t ON t.id = f.item_id WHERE t.parent_id = ? ORDER BY t.disc, t.idx LIMIT 5",
    )
    .bind(album_id)
    .fetch_all(&state.db.read)
    .await?;
    for (path, info) in tracks {
        let has_cover = info
            .as_deref()
            .and_then(|j| serde_json::from_str::<pley_transcoder_api::MediaInfo>(j).ok())
            .is_some_and(|m| m.has_cover_art);
        if !has_cover {
            continue;
        }
        if let Ok((bytes, _)) = state.transcoder.cover(&path).await {
            let Some(ext) = crate::store::picture_ext(&bytes) else { continue };
            let blob = state.store.put(&bytes, ext).await?;
            sqlx::query("UPDATE items SET poster = ?, updated_at = ? WHERE id = ? AND poster IS NULL")
                .bind(&blob)
                .bind(now())
                .bind(album_id)
                .execute(&state.db.write)
                .await?;
            return Ok(state.store.path(&blob));
        }
    }
    if state.transcoder.is_available() {
        if let Some(dir) = marker.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&marker, b"").await;
    }
    Ok(None)
}
