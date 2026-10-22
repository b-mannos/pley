use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use pley_transcoder_api::{AudioMode, JobRequest, VideoMode};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::items::item_or_404;
use crate::auth::AuthUser;
use crate::catalog::{self, ItemRow};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::library::sidecar::EXTERNAL_INDEX_BASE;
use crate::playback::{Method, PlaybackInfo, PlaybackRequest, Session, decide, subtitle_tracks};
use crate::state::AppState;

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
    Json(req): Json<PlaybackRequest>,
) -> ApiResult<Json<PlaybackInfo>> {
    let row = item_or_404(&state, user.id, req.item_id).await?;
    if !matches!(row.kind.as_str(), "movie" | "episode" | "track") {
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

    if decision.method == Method::Direct {
        let duration = file.duration_ms.or(row.duration_ms).unwrap_or(0);
        let session = state.sessions.insert(Session::new(
            user.id,
            user.username.clone(),
            row.id,
            file.id,
            Method::Direct,
            duration,
            None,
            None,
            start_ms,
        ));
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
            subtitles,
            reason: None,
        }));
    }

    let settings = state.settings.get();
    if state.sessions.active_transcodes() >= settings.max_transcodes as usize {
        return Err(AppError::Unavailable(format!(
            "the server is already running its maximum of {} transcodes",
            settings.max_transcodes
        )));
    }
    let job_request = JobRequest {
        input: file.path.clone(),
        media: info,
        label: Some(label(&row)),
        owner: Some(user.username.clone()),
        video: decision.video.clone(),
        audio: decision.audio.clone(),
        subtitle: decision.subtitle.clone(),
        segment_seconds: None,
        start_ms: Some(start_ms),
        options: settings.transcode_options(),
    };
    let created = state.transcoder.create_job(&job_request).await?;
    let method = match (&created.video, &created.audio) {
        (Some(v), _) if v.mode == VideoMode::Copy => Method::Remux,
        (Some(_), _) => Method::Transcode,
        (None, Some(a)) if a.mode == AudioMode::Copy => Method::Remux,
        _ => Method::Transcode,
    };
    reasons.extend(created.warnings.iter().cloned());
    let burn = decision.burn_index;
    let session = state.sessions.insert(Session::new(
        user.id,
        user.username.clone(),
        row.id,
        file.id,
        method,
        created.duration_ms,
        Some(job_request),
        Some(created.id.clone()),
        start_ms,
    ));
    state.events.send(ServerEvent::SessionsChanged);
    Ok(Json(PlaybackInfo {
        session_id: session.id.clone(),
        method,
        url: format!("/api/playback/{}/index.m3u8", session.id),
        hls: true,
        file_id: file.id,
        duration_ms: created.duration_ms,
        audio_index: decision.audio_index,
        subtitle_index: burn,
        burn_subtitles: burn.is_some(),
        subtitles,
        reason: (!reasons.is_empty()).then(|| reasons.join("; ")),
    }))
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
    state.sessions.remove(&session.id);
    if let Some(job) = session.job_id() {
        let transcoder = state.transcoder.clone();
        tokio::spawn(async move {
            let _ = transcoder.delete_job(&job).await;
        });
    }
    state.events.send(ServerEvent::SessionsChanged);
    Ok(StatusCode::NO_CONTENT)
}

/// The transcoder forgot the job (restart, expiry): create it again. The
/// segment layout is deterministic, so the client's playlist stays valid.
async fn replacement_job(state: &AppState, session: &Session, failed: &str) -> ApiResult<String> {
    let _guard = session.recreate.lock().await;
    if let Some(current) = session.job_id() {
        if current != failed {
            return Ok(current);
        }
    }
    let request = session.job_request.clone().ok_or_else(|| AppError::not_found("not a transcode session"))?;
    let created = state.transcoder.create_job(&JobRequest { start_ms: None, ..request }).await?;
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
    session.touch();
    let job = session.job_id().ok_or_else(|| AppError::not_found("not a transcode session"))?;

    if file == "index.m3u8" {
        let playlist = match state.transcoder.playlist(&job).await {
            Err(err) if err.is_not_found() => {
                let job = replacement_job(&state, &session, &job).await?;
                state.transcoder.playlist(&job).await?
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
        Err(err) if err.is_not_found() => {
            let job = replacement_job(&state, &session, &job).await?;
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

/// Direct play: the raw file with HTTP range support.
pub async fn stream_file(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<i64>,
    request: Request,
) -> ApiResult<Response> {
    let file = catalog::file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    let response =
        ServeFile::new(&file.path).oneshot(request).await.map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    let mut response = response.map(Body::new);
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    Ok(response)
}

pub async fn subtitles(
    State(state): State<AppState>,
    _user: AuthUser,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let index: i64 = name
        .strip_suffix(".vtt")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| AppError::not_found("unknown subtitle file"))?;
    let file = catalog::file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    let cache = state.config.cache_dir().join("subtitles").join(format!("{}_{}_{}.vtt", file.id, index, file.mtime));
    let text = match tokio::fs::read_to_string(&cache).await {
        Ok(text) => text,
        Err(_) => {
            let text = if index >= EXTERNAL_INDEX_BASE {
                let sub = file
                    .external_subs()
                    .into_iter()
                    .find(|s| s.index == index)
                    .ok_or_else(|| AppError::not_found("subtitle not found"))?;
                state.transcoder.subtitles(&sub.path, None).await?
            } else {
                state.transcoder.subtitles(&file.path, Some(index)).await?
            };
            if let Some(dir) = cache.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let _ = tokio::fs::write(&cache, &text).await;
            text
        }
    };
    Ok(([(header::CONTENT_TYPE, "text/vtt; charset=utf-8"), (header::CACHE_CONTROL, "private, max-age=3600")], text)
        .into_response())
}
