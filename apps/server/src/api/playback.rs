use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use pley_transcoder_api::{
    AudioMode, ConversionState, ConversionStatus, FileContainer, JobRequest, SUBTITLES_COMPLETE_HEADER, StreamKind,
    VideoMode,
};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::items::item_or_404;
use crate::auth::AuthUser;
use crate::catalog::{self, ItemRow};
use crate::error::{ApiResult, AppError};
use crate::events::ServerEvent;
use crate::library::sidecar::EXTERNAL_INDEX_BASE;
use crate::playback::{
    Method, PlaybackInfo, PlaybackRequest, Session, SessionStatus, TranscodeSpec, decide, new_media_key, sentence,
    subtitle_tracks, transcode_info,
};
use crate::state::AppState;
use crate::streams;

/// Length of HLS segments. Nothing plays before FFmpeg has written a whole
/// segment, and every start, seek and quality switch starts a process that
/// writes one first: 2 s ones come sooner than the transcoder's default of
/// 4 s (by 0.35 s for 4K at 60 fps tone mapped, less for lighter video), for
/// a keyframe every 2 s.
const SEGMENT_SECONDS: f64 = 2.0;
/// How long the transcoder's background work waits for a direct play's
/// first reads (the head, the index, the start).
const DIRECT_PLAY_HOLD: Duration = Duration::from_secs(10);

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
    if !master_ok && let Some(hls) = req.caps.hls.as_mut() {
        hls.master_playlist = false;
    }
    // Transcodes keep HDR only with a transcoder that can (HEVC Main 10).
    req.hdr_transcodes = state.transcoder.info().is_some_and(|info| info.capabilities.hdr_transcode);
    if !matches!(row.kind.as_str(), "movie" | "episode" | "track" | "extra") {
        return Err(AppError::bad_request("this item cannot be played directly"));
    }
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let file = match req.file_id {
        Some(id) => files.into_iter().find(|f| f.id == id).ok_or_else(|| AppError::not_found("file not found"))?,
        None => files.into_iter().next().ok_or_else(|| AppError::not_found("this item has no media file"))?,
    };
    // Background work (seek previews, intro detection) keeps off the disk
    // and the network from here on: creating a transcode reads the file too,
    // before the session exists.
    let _starting = state.sessions.begin_start();
    let info = file.media_info();
    let streams = file.streams();
    // No audio track named: the one the user's choices pick (what the item's
    // details told the player, `selected_audio`), not just the file's
    // default. A player that plays its own pick as it is would otherwise get
    // a session for the default track, find it isn't the one it shows, and
    // start another (a job made for nothing).
    if req.audio_index.is_none() {
        let pref = streams::load(&state.db, user.id, streams::scope(&row)).await?;
        let defaults = streams::defaults(&state.db, user.id).await?;
        req.audio_index = streams::resolve(pref.as_ref(), &defaults, &streams).audio;
    }
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
        session.media_key = req.keyed_urls.then(new_media_key);
        // The transcoder doesn't see direct plays: its background work waits
        // for the player's first reads too.
        if state.transcoder.is_available() {
            let transcoder = state.transcoder.clone();
            tokio::spawn(async move {
                let _ = transcoder.hold_background(DIRECT_PLAY_HOLD).await;
            });
        }
        let session = state.sessions.insert(session);
        state.events.send(ServerEvent::SessionsChanged);
        let url = match &session.media_key {
            // Named for its container: some players go by the extension.
            Some(key) => format!("/api/media/{key}/file.{}", container_extension(file.container.as_deref())),
            None => format!("/api/files/{}/stream", file.id),
        };
        return Ok(Json(PlaybackInfo {
            session_id: session.id.clone(),
            method: Method::Direct,
            url,
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
        container: None,
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
    session.media_key = req.keyed_urls.then(new_media_key);
    let session = state.sessions.insert_transcode(session, slot);
    state.events.send(ServerEvent::SessionsChanged);
    // Players that read multivariant playlists learn the codecs (Dolby
    // Vision, above all) before the first segment.
    let playlist = if req.caps.master_playlist() { "master.m3u8" } else { "index.m3u8" };
    // Playlists name their segments by relative addresses, so a keyed
    // playlist's segments carry the key too.
    let url = match &session.media_key {
        Some(key) => format!("/api/media/{key}/{playlist}"),
        None => format!("/api/playback/{}/{playlist}", session.id),
    };
    Ok(Json(PlaybackInfo {
        session_id: session.id.clone(),
        method,
        url,
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
/// which may name the media file's path, for an administrator (on pley's
/// own sign-in, never an app's); one of pley's for anyone else, who never
/// sees paths.
fn for_viewer(user: &AuthUser, err: impl Into<AppError>) -> AppError {
    let err = err.into();
    if user.is_admin {
        return err;
    }
    match err {
        AppError::NotFound(_) => AppError::not_found("this title's file can't be read right now"),
        AppError::BadRequest(_) => AppError::bad_request("this title's file can't be played"),
        err => err,
    }
}

fn owned_session(state: &AppState, user: &AuthUser, id: &str) -> ApiResult<Arc<Session>> {
    let Some(session) = state.sessions.get(id) else {
        // Stopped from elsewhere: a player without the event stream learns
        // it from its next request.
        return Err(match state.sessions.stopped(id) {
            Some(stopped) if stopped.user_id == user.id || user.is_admin => AppError::SessionStopped(stopped.message),
            _ => AppError::not_found("playback session expired"),
        });
    };
    if session.user_id != user.id && !user.is_admin {
        return Err(AppError::forbidden("not your session"));
    }
    Ok(session)
}

pub async fn stop(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let session = match owned_session(&state, &user, &id) {
        // Over already: what the player wanted.
        Err(AppError::SessionStopped(_)) => return Ok(StatusCode::NO_CONTENT),
        other => other?,
    };
    if let Some(session) = state.sessions.remove(&session.id) {
        end_sessions(&state, vec![session]);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The most a stopped session's message says.
const STOP_MESSAGE_MAX: usize = 200;

#[derive(Deserialize, Default)]
pub struct StopSession {
    #[serde(default)]
    message: Option<String>,
}

/// `DELETE /api/sessions/{id}` with an optional `{ message }`: ends a
/// playback session at once, from elsewhere than its player (the dashboard,
/// another device). Administrators stop any session, others their own.
pub async fn stop_session(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<StatusCode> {
    let body: StopSession = if body.iter().all(u8::is_ascii_whitespace) {
        StopSession::default()
    } else {
        serde_json::from_slice(&body).map_err(|err| AppError::bad_request(format!("invalid body: {err}")))?
    };
    let message = body.message.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
    if message.as_ref().is_some_and(|m| m.chars().count() > STOP_MESSAGE_MAX) {
        return Err(AppError::bad_request(format!("the message is longer than {STOP_MESSAGE_MAX} characters")));
    }
    stop_by(&state, &user, &id, message)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stops session `id` for `user` (an administrator: any session; anyone
/// else: their own): its transcode ends, its player hears `session_stopped`
/// and its next requests answer `410`. A session stopped already is fine.
pub(crate) fn stop_by(state: &AppState, user: &AuthUser, id: &str, message: Option<String>) -> ApiResult<()> {
    let Some(session) = state.sessions.get(id) else {
        return match state.sessions.stopped(id) {
            Some(stopped) if stopped.user_id == user.id || user.is_admin => Ok(()),
            Some(_) => Err(AppError::forbidden("not your session")),
            None => Err(AppError::not_found("no such session")),
        };
    };
    if session.user_id != user.id && !user.is_admin {
        return Err(AppError::forbidden("not your session"));
    }
    let Some(session) = state.sessions.stop(id, message.clone()) else { return Ok(()) };
    state.events.send(ServerEvent::SessionStopped {
        user_id: session.user_id,
        session_id: session.id.clone(),
        message,
        by_admin: session.user_id != user.id,
    });
    end_sessions(state, vec![session]);
    Ok(())
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
    // Its player waits: as much a start as the first one.
    let _starting = state.sessions.begin_start();
    // Stopped while this request waited: its job went on purpose.
    if let Some(stopped) = state.sessions.stopped(&session.id) {
        return Err(AppError::SessionStopped(stopped.message));
    }
    if let Some(current) = session.job_id()
        && current != failed
    {
        return Ok(current);
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
    // Stopped while the job was being created: nothing else will delete it.
    if let Some(stopped) = state.sessions.stopped(&session.id) {
        let _ = state.transcoder.delete_job(&created.id).await;
        return Err(AppError::SessionStopped(stopped.message));
    }
    tracing::info!("recreated transcode job for session {}", session.id);
    *session.job_id.lock() = Some(created.id.clone());
    Ok(created.id)
}

/// Where a download stands: the original, or a conversion of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    /// The original plays on the player as it is: `url` is it.
    Direct,
    Queued,
    Running,
    /// The converted file is whole: `url` is it.
    Done,
    Failed,
}

/// `POST /api/conversions` and `GET /api/conversions/{id}`: what a
/// download of an item takes (docs/API.md, "Downloads").
#[derive(Debug, Serialize)]
pub struct DownloadInfo {
    /// The conversion's id; `None` for the original.
    pub id: Option<String>,
    pub state: DownloadState,
    /// How far a conversion has come, 0 to 1 (1 for the original).
    pub progress: f64,
    /// Bytes, once known: the original's, or the converted file's once done.
    pub size: Option<u64>,
    pub error: Option<String>,
    /// Where the file comes from (with ranges): the original, or the converted file.
    pub url: String,
}

impl DownloadInfo {
    /// `status` as `user` sees it: an app doesn't get FFmpeg's words, which
    /// name the server's files (the log keeps them).
    fn for_viewer(status: &ConversionStatus, user: &AuthUser) -> Self {
        let mut info = Self::of(status);
        if user.app.is_some() {
            info.error = None;
        }
        info
    }

    fn of(status: &ConversionStatus) -> Self {
        Self {
            id: Some(status.id.clone()),
            state: match status.state {
                ConversionState::Queued => DownloadState::Queued,
                ConversionState::Running => DownloadState::Running,
                ConversionState::Done => DownloadState::Done,
                ConversionState::Failed => DownloadState::Failed,
            },
            progress: status.progress,
            size: status.size,
            error: status.error.clone(),
            url: format!("/api/conversions/{}/file", status.id),
        }
    }
}

/// `POST /api/conversions`: what a download of the item takes on the
/// player of `req.caps`, decided as a playback is. The original, when it
/// plays there as it is; else one file made by the transcoder of what a
/// playback would stream (the video copied or made anew, the sound made
/// AAC), once for every player that needs the same.
pub async fn convert(
    State(state): State<AppState>,
    user: AuthUser,
    Json(mut req): Json<PlaybackRequest>,
) -> ApiResult<Json<DownloadInfo>> {
    let row = item_or_404(&state, user.id, req.item_id).await?;
    if !matches!(row.kind.as_str(), "movie" | "episode" | "extra") {
        return Err(AppError::bad_request("only movies, episodes and extras are downloaded"));
    }
    let transcoder = state.transcoder.info();
    req.hdr_transcodes = transcoder.as_ref().is_some_and(|info| info.capabilities.hdr_transcode);
    req.download = true;
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let file = match req.file_id {
        Some(id) => files.into_iter().find(|f| f.id == id).ok_or_else(|| AppError::not_found("file not found"))?,
        None => files.into_iter().next().ok_or_else(|| AppError::not_found("this item has no media file"))?,
    };
    let info = file.media_info();
    let streams = file.streams();
    if req.audio_index.is_none() {
        let pref = streams::load(&state.db, user.id, streams::scope(&row)).await?;
        let defaults = streams::defaults(&state.db, user.id).await?;
        req.audio_index = streams::resolve(pref.as_ref(), &defaults, &streams).audio;
    }
    let decision = decide(&file, info.as_ref(), &streams, &req);
    if decision.method == Method::Direct {
        return Ok(Json(DownloadInfo {
            id: None,
            state: DownloadState::Direct,
            progress: 1.0,
            size: u64::try_from(file.size).ok(),
            error: None,
            url: format!("/api/files/{}/stream", file.id),
        }));
    }
    // A player that opens no Matroska (QuickTime) gets MP4.
    let container = req.caps.download_container();
    // Not running (starting, restarting): a moment's trouble. Running but
    // too old to make files: for good.
    match transcoder {
        None => return Err(AppError::Unavailable("the transcoder isn't running: try again in a moment".into())),
        Some(info) if !info.capabilities.conversions => {
            return Err(AppError::NotImplemented(
                "this server's transcoder can't make files for downloading: update it, or download the original".into(),
            ));
        }
        Some(info) if container == FileContainer::Mp4 && !info.capabilities.conversions_mp4 => {
            return Err(AppError::NotImplemented(
                "this server's transcoder can't make MP4 files: update it, or download the original".into(),
            ));
        }
        Some(_) => {}
    }
    let job = JobRequest {
        input: file.path.clone(),
        media: info,
        label: Some(label(&row)),
        owner: Some(user.username.clone()),
        video: decision.video.clone(),
        audio: decision.audio.clone(),
        subtitle: decision.subtitle.clone(),
        segment_seconds: None,
        start_ms: None,
        options: state.settings.get().transcode_options(),
        container: (container == FileContainer::Mp4).then_some(container),
    };
    let status = state.transcoder.create_conversion(&job).await.map_err(|err| match AppError::from(err) {
        // The file can't be read now (a share asleep, unmounted): a moment's
        // trouble for a download, which asks again later.
        AppError::NotFound(_) => AppError::Unavailable("this title's file can't be read right now".into()),
        err => for_viewer(&user, err),
    })?;
    state.sessions.note_conversion(&status.id, file.id);
    Ok(Json(DownloadInfo::for_viewer(&status, &user)))
}

/// The file conversion `id` is made of, when this user may see it.
async fn conversion_of(state: &AppState, user: &AuthUser, id: &str) -> ApiResult<i64> {
    let file_id = state.sessions.conversion_file(id).ok_or_else(|| AppError::not_found("conversion not found"))?;
    visible_file(state, user, file_id).await?;
    Ok(file_id)
}

/// `GET /api/conversions/{id}`: how a conversion goes.
pub async fn conversion(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<DownloadInfo>> {
    conversion_of(&state, &user, &id).await?;
    let status = state.transcoder.conversion(&id).await.map_err(|err| for_viewer(&user, err))?;
    Ok(Json(DownloadInfo::for_viewer(&status, &user)))
}

/// `DELETE /api/conversions/{id}`: the download that wanted it went before
/// the file was made, so the conversion stops. One done stays, for whoever
/// asks for the same next.
pub async fn cancel_conversion(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    conversion_of(&state, &user, &id).await?;
    let status = state.transcoder.conversion(&id).await.map_err(|err| for_viewer(&user, err))?;
    if status.state != ConversionState::Done {
        state.transcoder.delete_conversion(&id).await.map_err(|err| for_viewer(&user, err))?;
        state.sessions.forget_conversion(&id);
        tracing::info!("conversion {id} stopped: its download was deleted");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/conversions/{id}/file`: the converted file once whole, with
/// ranges (a download cut short goes on), from the transcoder as it comes.
pub async fn conversion_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    request: Request,
) -> ApiResult<Response> {
    conversion_of(&state, &user, &id).await?;
    let range = request.headers().get(header::RANGE);
    let upstream = state.transcoder.conversion_file(&id, range).await.map_err(|err| for_viewer(&user, err))?;
    let mut response =
        Response::builder().status(upstream.status().as_u16()).header(header::CACHE_CONTROL, "private, no-cache");
    for name in [header::CONTENT_TYPE, header::CONTENT_LENGTH, header::CONTENT_RANGE, header::ACCEPT_RANGES] {
        if let Some(value) = upstream.headers().get(name.as_str()) {
            response = response.header(name, value.as_bytes());
        }
    }
    response.body(Body::from_stream(upstream.bytes_stream())).map_err(|e| AppError::Internal(e.into()))
}

pub async fn hls_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, file)): Path<(String, String)>,
) -> ApiResult<Response> {
    let session = owned_session(&state, &user, &id)?;
    session_hls(&state, &user, &session, &file).await
}

/// A playlist or segment (`index.m3u8`, `master.m3u8`, `{n}.ts`) of a
/// transcode session, for `user`.
async fn session_hls(state: &AppState, user: &AuthUser, session: &Arc<Session>, file: &str) -> ApiResult<Response> {
    if session.touch() {
        state.events.send(ServerEvent::SessionsChanged);
    }
    if session.transcode.is_none() {
        return Err(AppError::not_found("not a transcode session"));
    }
    let job = session.job_id().ok_or_else(|| AppError::not_found("playback session expired"))?;

    if file == "index.m3u8" || file == "master.m3u8" {
        let playlist = match state.transcoder.playlist_file(&job, file).await {
            Err(err) if err.is_job_gone() => {
                let job = replacement_job(state, user, session, &job).await?;
                state.transcoder.playlist_file(&job, file).await?
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
            let job = replacement_job(state, user, session, &job).await?;
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

/// `GET /api/media/{key}/{file}`: a session's media for a player that can't
/// send the Authorization header, a TV's own player (docs/API.md, "Keyed
/// URLs"). The key its play request asked for (`keyed_urls`) stands in for
/// the token, for this session's playlists and segments or its file, and
/// nothing else; it ends with the session, the account, or the app's
/// sign-in. A session stopped from elsewhere answers `410` here too.
pub async fn keyed_media(
    State(state): State<AppState>,
    Path((key, file)): Path<(String, String)>,
    request: Request,
) -> ApiResult<Response> {
    let Some(session) = state.sessions.by_media_key(&key) else {
        return Err(match state.sessions.stopped_by_media_key(&key) {
            Some(stopped) => AppError::SessionStopped(stopped.message),
            None => AppError::not_found("playback session expired"),
        });
    };
    let gone = || AppError::not_found("playback session expired");
    let mut user = AuthUser::load(&state.db, session.user_id).await?.ok_or_else(gone)?;
    if let Some(grant) = session.app_grant {
        // Signed out since: the key goes with the token.
        let access = crate::auth::grants::access_of(&state.db, grant).await?.ok_or_else(gone)?;
        user.is_admin = false;
        user.app = Some(Arc::new(access));
    }
    if session.transcode.is_some() {
        return session_hls(&state, &user, &session, &file).await;
    }
    // Direct play: the session's own file, whatever its name says ("file.mkv").
    if file.split('.').next() != Some("file") {
        return Err(AppError::not_found("unknown file"));
    }
    if session.touch() {
        state.events.send(ServerEvent::SessionsChanged);
    }
    let row = visible_file(&state, &user, session.file_id).await?;
    if !crate::library::walk::leads_to_its_kind(std::path::Path::new(&row.path)) {
        return Err(AppError::not_found("file not found"));
    }
    let response =
        ServeFile::new(&row.path).oneshot(request).await.map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    let mut response = response.map(Body::new);
    // Never cached anywhere: the address carries a key.
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok(response)
}

/// The extension a keyed file's address ends with: its container's, when
/// it is a plain word ("mkv", "mp4"), else "bin".
fn container_extension(container: Option<&str>) -> String {
    match container {
        Some(c) if !c.is_empty() && c.len() <= 8 && c.chars().all(|ch| ch.is_ascii_alphanumeric()) => {
            c.to_ascii_lowercase()
        }
        _ => "bin".to_owned(),
    }
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

/// `/api/files/{id}/subtitles/{index}.vtt`: a text track (or subtitle file)
/// as WebVTT; `{index}.mks`: a picture track in a Matroska file of its own.
pub async fn subtitles(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, name)): Path<(i64, String)>,
    request: Request,
) -> ApiResult<Response> {
    if let Some(index) = name.strip_suffix(".mks") {
        let index: i64 = index.parse().map_err(|_| AppError::not_found("unknown subtitle file"))?;
        let file = visible_file(&state, &user, id).await?;
        return picture_track(&state, &user, file, index, request).await;
    }
    let index: i64 = name
        .strip_suffix(".vtt")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| AppError::not_found("unknown subtitle file"))?;
    let file = visible_file(&state, &user, id).await?;
    let (sidecar, version) = subtitle_source(&file, index).await?;
    let cache =
        state.config.cache_dir().join("subtitles").join(format!("{}_{}_{}{version}.vtt", file.id, index, file.mtime));
    // Anything but WebVTT is a file cut short by an earlier version (written
    // in place) that crashed or ran out of space: it is made again.
    if let Ok(text) = tokio::fs::read_to_string(&cache).await
        && text.starts_with("WEBVTT")
    {
        return Ok(vtt(text));
    }
    let Some(path) = sidecar else {
        // An embedded track: the cues found so far, at once, while the rest
        // of the file is read (minutes for a big one on a network share).
        // The player asks again until it has them all.
        let (text, complete) =
            state.transcoder.subtitles_so_far(&file.path, index).await.map_err(|err| for_viewer(&user, err))?;
        if complete {
            cache_subtitles(&cache, text.as_bytes()).await;
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
    let transcoder = state.transcoder.clone();
    let made = extract_once(cache, async move { Ok(transcoder.subtitles(&path, None).await?.into_bytes()) })
        .await
        .map_err(|err| for_viewer(&user, err))?;
    Ok(vtt(String::from_utf8_lossy(&made).into_owned()))
}

/// Where subtitle `index` of `file` comes from: a sidecar's path (`None`:
/// the track is in the file), and what tells its versions apart in cache
/// names. A sidecar can be replaced without its video changing (subtitle
/// downloaders upgrade them): its own time is part of the name too.
async fn subtitle_source(file: &catalog::FileRow, index: i64) -> ApiResult<(Option<String>, String)> {
    if index < EXTERNAL_INDEX_BASE {
        return Ok((None, String::new()));
    }
    let sub = file
        .external_subs()
        .into_iter()
        .find(|s| s.index == index)
        .filter(|s| crate::library::walk::leads_to_its_kind(std::path::Path::new(&s.path)))
        .ok_or_else(|| AppError::not_found("subtitle not found"))?;
    let modified = tokio::fs::metadata(&sub.path).await.ok().and_then(|m| m.modified().ok());
    Ok((Some(sub.path), format!("_{}", modified.map(crate::util::system_time_secs).unwrap_or(0))))
}

/// What `.mks` files are served as.
const MATROSKA: &str = "video/x-matroska";

/// A picture subtitle track (PGS, VobSub, DVB; in the file or next to it)
/// copied as it is into a Matroska file of its own, for players that show
/// it next to an HLS stream instead of having it drawn onto the video. Its
/// times are the original's from the start of the file, as the stream's
/// are. Made once per file version (by the transcoder) and cached, then
/// served from the cache with ranges. `404` for a text track or anything
/// but subtitles.
async fn picture_track(
    state: &AppState,
    user: &AuthUser,
    file: catalog::FileRow,
    index: i64,
    request: Request,
) -> ApiResult<Response> {
    let picture = file
        .streams()
        .iter()
        .any(|s| s.index == index && s.kind == StreamKind::Subtitle && !s.text_based.unwrap_or(true));
    if !picture {
        return Err(AppError::not_found("no picture subtitle track has this index"));
    }
    let (sidecar, version) = subtitle_source(&file, index).await?;
    let cache =
        state.config.cache_dir().join("subtitles").join(format!("{}_{}_{}{version}.mks", file.id, index, file.mtime));
    if !is_matroska(&cache).await {
        if !state.transcoder.copies_subtitles() {
            return Err(AppError::Unavailable("the transcoder can't copy subtitle tracks yet: update it".into()));
        }
        let (path, stream_index) = match sidecar {
            Some(path) => (path, None),
            None => (file.path.clone(), Some(index)),
        };
        let transcoder = state.transcoder.clone();
        let made =
            extract_once(cache.clone(), async move { Ok(transcoder.subtitle_matroska(&path, stream_index).await?) })
                .await
                .map_err(|err| for_viewer(user, err))?;
        if !is_matroska(&cache).await {
            // Not cached (a full disk): served as it is.
            return Ok((
                [(header::CONTENT_TYPE, MATROSKA), (header::CACHE_CONTROL, "private, max-age=3600")],
                (*made).clone(),
            )
                .into_response());
        }
    }
    let response =
        ServeFile::new(&cache).oneshot(request).await.map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?;
    let mut response = response.map(Body::new);
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(MATROSKA));
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    Ok(response)
}

/// Whether `path` is a whole Matroska file (cached ones are renamed into
/// place whole, so the EBML header says enough).
async fn is_matroska(path: &std::path::Path) -> bool {
    use tokio::io::AsyncReadExt;
    let mut head = [0u8; 4];
    match tokio::fs::File::open(path).await {
        Ok(mut file) => file.read_exact(&mut head).await.is_ok() && head == [0x1A, 0x45, 0xDF, 0xA3],
        Err(_) => false,
    }
}

/// A conversion under way, and which one it is.
#[derive(Clone)]
struct Extraction {
    id: u64,
    result: Shared<BoxFuture<'static, Made>>,
}

/// What a conversion made, shared by the requests that wait for it.
type Made = Result<Arc<Vec<u8>>, Arc<AppError>>;

/// Subtitles being pulled out of their files, by cache file.
static EXTRACTIONS: LazyLock<DashMap<PathBuf, Extraction>> = LazyLock::new(DashMap::new);
static NEXT_EXTRACTION: AtomicU64 = AtomicU64::new(0);

/// What `work` makes (subtitles pulled out of their file), made once and
/// cached at `cache`: requests for the same file wait for one conversion,
/// which goes on to the cache when nobody waits any more (a player that
/// moved on, or restarted). `work` is dropped unused when one is under way.
async fn extract_once(
    cache: PathBuf,
    work: impl Future<Output = ApiResult<Vec<u8>>> + Send + 'static,
) -> ApiResult<Arc<Vec<u8>>> {
    let extraction = EXTRACTIONS
        .entry(cache.clone())
        .or_insert_with(|| {
            let id = NEXT_EXTRACTION.fetch_add(1, Ordering::Relaxed);
            let cache = cache.clone();
            let task = tokio::spawn(async move {
                let _done = Forget { cache: cache.clone(), id };
                let made = work.await?;
                cache_subtitles(&cache, &made).await;
                Ok::<_, AppError>(Arc::new(made))
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
async fn cache_subtitles(path: &std::path::Path, data: &[u8]) {
    let Some(dir) = path.parent() else { return };
    let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let written = async {
        tokio::fs::create_dir_all(dir).await?;
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, path).await
    }
    .await;
    if let Err(err) = written {
        let _ = tokio::fs::remove_file(&tmp).await;
        tracing::debug!("subtitles not cached in {}: {err}", path.display());
    }
}
