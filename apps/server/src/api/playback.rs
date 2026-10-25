use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use pley_transcoder_api::{AudioMode, JobRequest, SUBTITLES_COMPLETE_HEADER, VideoMode};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::items::item_or_404;
use crate::auth::AuthUser;
use crate::catalog::{self, ItemRow};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::library::sidecar::EXTERNAL_INDEX_BASE;
use crate::playback::{
    Method, PlaybackInfo, PlaybackRequest, Session, SessionStatus, TranscodeSpec, decide, sentence, subtitle_tracks,
    transcode_info,
};
use crate::state::AppState;

/// Length of HLS segments. Nothing plays before FFmpeg has written a whole
/// segment, and every start, seek and quality switch starts a process that
/// writes one first: 2 s ones come sooner than the transcoder's default of
/// 4 s (by 0.35 s for 4K at 60 fps tone mapped, less for lighter video), for
/// a keyframe every 2 s.
const SEGMENT_SECONDS: f64 = 2.0;

fn label(row: &ItemRow) -> String {
    match row.kind.as_str() {
        "episode" => format!(
            "{} — S{:02}E{:02} · {}",
            row.grandparent_title.as_deref().unwrap_or("?"),
            row.parent_idx.unwrap_or(0),
            row.idx.unwrap_or(0),
            row.title
        ),
        "track" => format!("{} — {}", row.grandparent_title.as_deref().unwrap_or("?"), row.title),
        _ => match row.year {
            Some(y) => format!("{} ({y})", row.title),
            None => row.title.clone(),
        },
    }
}

pub async fn start(
    State(state): State<AppState>,
    user: AuthUser,
    Json(mut req): Json<PlaybackRequest>,
) -> ApiResult<Json<PlaybackInfo>> {
    let row = item_or_404(&state, user.id, req.item_id).await?;
    // A transcoder older than master playlists can't name Dolby Vision
    // (or anything else) in one: the player gets what older servers gave.
    let master_ok = state.transcoder.info().is_some_and(|info| info.capabilities.hls_master_playlist);
    if !master_ok {
        if let Some(hls) = req.caps.hls.as_mut() {
            hls.master_playlist = false;
        }
    }
    if !matches!(row.kind.as_str(), "movie" | "episode" | "track" | "extra") {
        return Err(AppError::bad_request("this item cannot be played directly"));
    }
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let file = match req.file_id {
        Some(id) => files.into_iter().find(|f| f.id == id).ok_or_else(|| AppError::not_found("file not found"))?,
        None => files.into_iter().next().ok_or_else(|| AppError::not_found("this item has no media file"))?,
    };
    let info = file.media_info();
    let streams = file.streams();
    let decision = decide(&file, info.as_ref(), &streams, &req);
    let mut reasons = decision.reasons.clone();
    let subtitles = subtitle_tracks(file.id, &streams);
    let start_ms = req.start_ms.unwrap_or(0).max(0);
    let client_id = req.client_id.clone().filter(|c| !c.is_empty() && c.len() <= 128);

    // A client plays one thing at a time: end what it played before (a
    // quality or audio switch, the next episode) so its transcode slot is
    // free before the limit is checked.
    if let Some(client) = &client_id {
        let previous = state.sessions.take_client_sessions(user.id, client);
        if !previous.is_empty() {
            end_sessions(&state, previous);
        }
    }

    if decision.method == Method::Direct {
        let duration = file.duration_ms.or(row.duration_ms).unwrap_or(0);
        let mut session = Session::new(
            user.id,
            user.username.clone(),
            client_id,
            row.id,
            file.id,
            Method::Direct,
            duration,
            None,
            None,
            start_ms,
        );
        session.app_grant = user.app.as_ref().map(|app| app.grant_id);
        let session = state.sessions.insert(session);
        state.events.send(ServerEvent::SessionsChanged);
        return Ok(Json(PlaybackInfo {
            session_id: session.id.clone(),
            method: Method::Direct,
            url: format!("/api/files/{}/stream", file.id),
            hls: false,
            file_id: file.id,
            duration_ms: duration,
            audio_index: decision.audio_index,
            subtitle_index: None,
            burn_subtitles: false,
            audio_converted: false,
            subtitles,
            reasons: Vec::new(),
            reason: None,
            auto_quality: None,
        }));
    }

    let settings = state.settings.get();
    // Taken before the job is created (that takes seconds) and handed to the
    // session; given back if this request fails or the client goes away.
    let Some(slot) = state.sessions.reserve_transcode(settings.max_transcodes as usize) else {
        return Err(AppError::Unavailable(format!(
            "the server is already running its maximum of {} transcodes",
            settings.max_transcodes
        )));
    };
    let job_request = JobRequest {
        input: file.path.clone(),
        media: info,
        label: Some(label(&row)),
        owner: Some(user.username.clone()),
        video: decision.video.clone(),
        audio: decision.audio.clone(),
        subtitle: decision.subtitle.clone(),
        segment_seconds: Some(SEGMENT_SECONDS),
        start_ms: Some(start_ms),
        options: settings.transcode_options(),
    };
    let created = state.transcoder.create_job(&job_request).await.map_err(|err| for_viewer(&user, err))?;
    let method = match (&created.video, &created.audio) {
        (Some(v), _) if v.mode == VideoMode::Copy => Method::Remux,
        (Some(_), _) => Method::Transcode,
        (None, Some(a)) if a.mode == AudioMode::Copy => Method::Remux,
        _ => Method::Transcode,
    };
    reasons.extend(created.warnings.iter().map(|w| sentence(w)));
    let burn = decision.burn_index;
    // Pin the effective video mode: the segment layout depends on it.
    let mut request = job_request;
    if let (Some(video), Some(plan)) = (request.video.as_mut(), created.video.as_ref()) {
        video.mode = plan.mode;
    }
    let spec = TranscodeSpec { request, segment_count: created.segment_count, duration_ms: created.duration_ms };
    let mut session = Session::new(
        user.id,
        user.username.clone(),
        client_id,
        row.id,
        file.id,
        method,
        created.duration_ms,
        Some(spec),
        Some(created.id.clone()),
        start_ms,
    );
    session.app_grant = user.app.as_ref().map(|app| app.grant_id);
    let session = state.sessions.insert_transcode(session, slot);
    state.events.send(ServerEvent::SessionsChanged);
    // Players that read multivariant playlists learn the codecs (Dolby
    // Vision, above all) before the first segment.
    let playlist = if req.caps.master_playlist() { "master.m3u8" } else { "index.m3u8" };
    Ok(Json(PlaybackInfo {
        session_id: session.id.clone(),
        method,
        url: format!("/api/playback/{}/{playlist}", session.id),
        hls: true,
        file_id: file.id,
        duration_ms: created.duration_ms,
        audio_index: decision.audio_index,
        subtitle_index: burn,
        burn_subtitles: burn.is_some(),
        audio_converted: created.audio.as_ref().is_some_and(|a| a.mode != AudioMode::Copy),
        subtitles,
        reason: (!reasons.is_empty()).then(|| reasons.join(" ")),
        reasons,
        // Only when the video really is converted (a remux the transcoder
        // turned into a transcode keeps its own default).
        auto_quality: decision.quality.filter(|_| method == Method::Transcode).map(str::to_owned),
    }))
}

/// `GET /api/playback/{session}`: how the session plays now, with the live
/// numbers of its transcode (what it converts to, on what, how fast), for
/// the player's details.
pub async fn status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<SessionStatus>> {
    let session = owned_session(&state, &user, &id)?;
    let transcode = match session.job_id() {
        Some(job) => state.transcoder.job_status(&job).await.ok().map(|status| transcode_info(&status)),
        None => None,
    };
    let transcode = transcode.map(|mut info| {
        // FFmpeg's last words name the media file.
        if user.app.is_some() && info.last_error.is_some() {
            info.last_error = Some("the conversion failed".to_owned());
        }
        info
    });
    Ok(Json(SessionStatus { session_id: session.id.clone(), method: session.method, transcode }))
}

/// What a viewer is told when the transcoder refused: its own sentence,
/// which may name the media file's path, for pley's own sign-in; one of
/// pley's for an app, since apps never see paths.
fn for_viewer(user: &AuthUser, err: impl Into<AppError>) -> AppError {
    let err = err.into();
    if user.app.is_none() {
        return err;
    }
    match err {
        AppError::NotFound(_) => AppError::not_found("this title's file can't be read right now"),
        AppError::BadRequest(_) => AppError::bad_request("this title's file can't be played"),
        err => err,
    }
}

fn owned_session(state: &AppState, user: &AuthUser, id: &str) -> ApiResult<Arc<Session>> {
    let session = state.sessions.get(id).ok_or_else(|| AppError::not_found("playback session expired"))?;
    if session.user_id != user.id && !user.is_admin {
        return Err(AppError::forbidden("not your session"));
    }
    Ok(session)
}

pub async fn stop(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let session = owned_session(&state, &user, &id)?;
    if let Some(session) = state.sessions.remove(&session.id) {
        end_sessions(&state, vec![session]);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Forget sessions that were already removed from the registry and delete
/// their transcoder jobs in the background.
fn end_sessions(state: &AppState, sessions: Vec<Arc<Session>>) {
    let jobs: Vec<String> = sessions.iter().filter_map(|s| s.job_id()).collect();
    if !jobs.is_empty() {
        let transcoder = state.transcoder.clone();
        tokio::spawn(async move {
            for job in jobs {
                let _ = transcoder.delete_job(&job).await;
            }
        });
    }
    state.events.send(ServerEvent::SessionsChanged);
}

/// The transcoder forgot the job (restart, expiry): create it again with the
/// same effective request. The client never reloads its VOD playlist, so the
/// new job must have the very same segment layout; when it does not (the
/// keyframe cache was cleared), the session is over and the client starts a
/// new one.
async fn replacement_job(state: &AppState, user: &AuthUser, session: &Session, failed: &str) -> ApiResult<String> {
    let _guard = session.recreate.lock().await;
    if let Some(current) = session.job_id() {
        if current != failed {
            return Ok(current);
        }
    }
    let spec = session.transcode.as_ref().ok_or_else(|| AppError::not_found("not a transcode session"))?;
    // Pre-start near the playhead instead of at the beginning.
    let request = JobRequest { start_ms: Some(session.position_ms()), ..spec.request.clone() };
    let created = state.transcoder.create_job(&request).await.map_err(|err| for_viewer(user, err))?;
    let same_mode = match (&spec.request.video, &created.video) {
        (Some(wanted), Some(got)) => wanted.mode == got.mode,
        (None, None) => true,
        _ => false,
    };
    if !same_mode || created.segment_count != spec.segment_count || created.duration_ms != spec.duration_ms {
        let _ = state.transcoder.delete_job(&created.id).await;
        *session.job_id.lock() = None;
        tracing::info!("transcode job for session {} cannot be recreated with the same layout", session.id);
        return Err(AppError::not_found("playback session expired"));
    }
    tracing::info!("recreated transcode job for session {}", session.id);
    *session.job_id.lock() = Some(created.id.clone());
    Ok(created.id)
}

pub async fn hls_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, file)): Path<(String, String)>,
) -> ApiResult<Response> {
    let session = owned_session(&state, &user, &id)?;
    if session.touch() {
        state.events.send(ServerEvent::SessionsChanged);
    }
    if session.transcode.is_none() {
        return Err(AppError::not_found("not a transcode session"));
    }
    let job = session.job_id().ok_or_else(|| AppError::not_found("playback session expired"))?;

    if file == "index.m3u8" || file == "master.m3u8" {
        let playlist = match state.transcoder.playlist_file(&job, &file).await {
            Err(err) if err.is_job_gone() => {
                let job = replacement_job(&state, &user, &session, &job).await?;
                state.transcoder.playlist_file(&job, &file).await?
            }
            other => other?,
        };
        return Ok((
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl"), (header::CACHE_CONTROL, "no-cache")],
            playlist,
        )
            .into_response());
    }

    let n: u32 = file
        .strip_suffix(".ts")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| AppError::not_found("unknown playlist file"))?;
    let upstream = match state.transcoder.segment(&job, n).await {
        Err(err) if err.is_job_gone() => {
            let job = replacement_job(&state, &user, &session, &job).await?;
            state.transcoder.segment(&job, n).await?
        }
        other => other?,
    };
    let mut response =
        Response::builder().header(header::CONTENT_TYPE, "video/mp2t").header(header::CACHE_CONTROL, "no-cache");
    if let Some(len) = upstream.content_length() {
        response = response.header(header::CONTENT_LENGTH, len);
    }
    response.body(Body::from_stream(upstream.bytes_stream())).map_err(|e| AppError::Internal(e.into()))
}

/// A media file of an item the user may see.
pub(crate) async fn visible_file(state: &AppState, user: &AuthUser, id: i64) -> ApiResult<catalog::FileRow> {
    let file = catalog::file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    if !catalog::is_visible(&state.db, user.id, file.item_id).await? {
        return Err(AppError::not_found("file not found"));
    }
    Ok(file)
}

/// Seek-bar previews of a file; `404` while they are being made (asking
/// queues them).
pub async fn trickplay(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<pley_transcoder_api::TrickplayInfo>> {
    let file = visible_file(&state, &user, id).await?;
    match state.trickplay.index(&file) {
        Some(index) => Ok(Json(index)),
        None => {
            state.trickplay.request(&file);
            Err(AppError::not_found("seek previews are not ready yet"))
        }
    }
}

/// One sprite sheet (`{n}.jpg`); versioned by the `?v=` the client adds.
pub async fn trickplay_sheet(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, sheet)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let n: u32 =
        sheet.strip_suffix(".jpg").and_then(|n| n.parse().ok()).ok_or_else(|| AppError::not_found("unknown sheet"))?;
    let file = visible_file(&state, &user, id).await?;
    let path = state.trickplay.sheet(&file, n).ok_or_else(|| AppError::not_found("unknown sheet"))?;
    let bytes = tokio::fs::read(path).await.map_err(|e| AppError::Internal(e.into()))?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], bytes)
        .into_response())
}

/// A chapter's picture (`{n}.jpg`), cut from the seek previews.
pub async fn chapter_image(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let n: usize =
        name.strip_suffix(".jpg").and_then(|n| n.parse().ok()).ok_or_else(|| AppError::not_found("unknown chapter"))?;
    let file = visible_file(&state, &user, id).await?;
    let path = state
        .trickplay
        .chapter_image(&file, n)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::not_found("no chapter image"))?;
    let bytes = tokio::fs::read(path).await.map_err(|e| AppError::Internal(e.into()))?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], bytes)
        .into_response())
}

/// Direct play: the raw file with HTTP range support.
pub async fn stream_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    request: Request,
) -> ApiResult<Response> {
    let file = visible_file(&state, &user, id).await?;
    if !crate::library::walk::leads_to_its_kind(std::path::Path::new(&file.path)) {
        return Err(AppError::not_found("file not found"));
    }
    let response =
        ServeFile::new(&file.path).oneshot(request).await.map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    let mut response = response.map(Body::new);
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    Ok(response)
}

pub async fn subtitles(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let index: i64 = name
        .strip_suffix(".vtt")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| AppError::not_found("unknown subtitle file"))?;
    let file = visible_file(&state, &user, id).await?;
    let sidecar = if index >= EXTERNAL_INDEX_BASE {
        let sub = file
            .external_subs()
            .into_iter()
            .find(|s| s.index == index)
            .filter(|s| crate::library::walk::leads_to_its_kind(std::path::Path::new(&s.path)))
            .ok_or_else(|| AppError::not_found("subtitle not found"))?;
        Some(sub.path)
    } else {
        None
    };
    // A sidecar can be replaced without its video changing (subtitle
    // downloaders upgrade them): its own time is part of the name too.
    let version = match &sidecar {
        Some(path) => {
            let modified = tokio::fs::metadata(path).await.ok().and_then(|m| m.modified().ok());
            format!("_{}", modified.map(crate::util::system_time_secs).unwrap_or(0))
        }
        None => String::new(),
    };
    let cache =
        state.config.cache_dir().join("subtitles").join(format!("{}_{}_{}{version}.vtt", file.id, index, file.mtime));
    // Anything but WebVTT is a file cut short by an earlier version (written
    // in place) that crashed or ran out of space: it is made again.
    if let Ok(text) = tokio::fs::read_to_string(&cache).await {
        if text.starts_with("WEBVTT") {
            return Ok(vtt(text));
        }
    }
    let Some(path) = sidecar else {
        // An embedded track: the cues found so far, at once, while the rest
        // of the file is read (minutes for a big one on a network share).
        // The player asks again until it has them all.
        let (text, complete) =
            state.transcoder.subtitles_so_far(&file.path, index).await.map_err(|err| for_viewer(&user, err))?;
        if complete {
            cache_subtitles(&cache, &text).await;
            return Ok(vtt(text));
        }
        return Ok((
            [
                (header::CONTENT_TYPE, "text/vtt; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
                (header::HeaderName::from_static(SUBTITLES_COMPLETE_HEADER), "0"),
            ],
            text,
        )
            .into_response());
    };
    let text = extract_subtitles(&state, path, cache).await.map_err(|err| for_viewer(&user, err))?;
    Ok(vtt((*text).clone()))
}

/// A conversion under way, and which one it is.
#[derive(Clone)]
struct Extraction {
    id: u64,
    result: Shared<BoxFuture<'static, Result<Arc<String>, Arc<AppError>>>>,
}

/// Subtitles being pulled out of their files, by cache file.
static EXTRACTIONS: LazyLock<DashMap<PathBuf, Extraction>> = LazyLock::new(DashMap::new);
static NEXT_EXTRACTION: AtomicU64 = AtomicU64::new(0);

/// The subtitle file at `path` (a sidecar) as WebVTT, made once and cached
/// at `cache`: requests for the same subtitles wait for one conversion, which
/// goes on to the cache when nobody waits any more (a player that moved on,
/// or restarted).
async fn extract_subtitles(state: &AppState, path: String, cache: PathBuf) -> ApiResult<Arc<String>> {
    let extraction = EXTRACTIONS
        .entry(cache.clone())
        .or_insert_with(|| {
            let id = NEXT_EXTRACTION.fetch_add(1, Ordering::Relaxed);
            let state = state.clone();
            let cache = cache.clone();
            let task = tokio::spawn(async move {
                let _done = Forget { cache: cache.clone(), id };
                let text = state.transcoder.subtitles(&path, None).await?;
                cache_subtitles(&cache, &text).await;
                Ok::<_, AppError>(Arc::new(text))
            });
            let result = async move {
                match task.await {
                    Ok(result) => result.map_err(Arc::new),
                    Err(err) => Err(Arc::new(AppError::Internal(err.into()))),
                }
            };
            Extraction { id, result: result.boxed().shared() }
        })
        .clone();
    extraction.result.await.map_err(|err| err.again())
}

/// Takes a conversion out of [`EXTRACTIONS`] once it's over, however it ends
/// and whoever still waits for it: the next request finds the cache, or
/// tries again after a failure. A later conversion in its place stays.
struct Forget {
    cache: PathBuf,
    id: u64,
}

impl Drop for Forget {
    fn drop(&mut self) {
        EXTRACTIONS.remove_if(&self.cache, |_, extraction| extraction.id == self.id);
    }
}

fn vtt(text: String) -> Response {
    ([(header::CONTENT_TYPE, "text/vtt; charset=utf-8"), (header::CACHE_CONTROL, "private, max-age=3600")], text)
        .into_response()
}

/// Caches converted subtitles whole or not at all: they go to a temporary
/// file that is renamed into place, so a crash or a full disk never leaves a
/// file cut short to be served from then on.
async fn cache_subtitles(path: &std::path::Path, text: &str) {
    let Some(dir) = path.parent() else { return };
    let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let written = async {
        tokio::fs::create_dir_all(dir).await?;
        tokio::fs::write(&tmp, text).await?;
        tokio::fs::rename(&tmp, path).await
    }
    .await;
    if let Err(err) = written {
        let _ = tokio::fs::remove_file(&tmp).await;
        tracing::debug!("subtitles not cached in {}: {err}", path.display());
    }
}
