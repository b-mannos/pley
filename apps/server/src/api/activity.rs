use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use pley_transcoder_api::JobStatus;
use serde::Serialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::access;
use crate::auth::{AuthUser, CurrentToken};
use crate::catalog::{self, ItemSummary};
use crate::error::ApiResult;
use crate::library::scanner::ScanStatus;
use crate::markers::AnalysisStatus;
use crate::playback::{Method, TranscodeInfo, transcode_info};
use crate::state::AppState;

#[derive(Serialize)]
pub struct SessionInfo {
    session_id: String,
    user_id: i64,
    username: String,
    item: ItemSummary,
    method: Method,
    position_ms: i64,
    state: String,
    started_at: i64,
    updated_at: i64,
    transcode: Option<TranscodeInfo>,
}

#[derive(Serialize)]
pub struct Activity {
    scans: Vec<ScanStatus>,
    sessions: Vec<SessionInfo>,
    /// Intro/credits detection in progress.
    analysis: Option<AnalysisStatus>,
    /// Files waiting for seek previews (administrators only).
    previews_pending: usize,
}

pub async fn activity(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Activity>> {
    let sessions: Vec<_> =
        state.sessions.list().into_iter().filter(|s| user.is_admin || s.user_id == user.id).collect();
    let jobs: HashMap<String, JobStatus> = if sessions.iter().any(|s| s.job_id().is_some()) {
        match state.transcoder.status().await {
            Ok(status) => status.jobs.into_iter().map(|j| (j.id.clone(), j)).collect(),
            Err(_) => HashMap::new(),
        }
    } else {
        HashMap::new()
    };
    let mut out = Vec::new();
    for session in sessions {
        let Some(row) = catalog::item(&state.db, session.user_id, session.item_id).await? else { continue };
        let play = session.play.lock().clone();
        out.push(SessionInfo {
            session_id: session.id.clone(),
            user_id: session.user_id,
            username: session.username.clone(),
            item: row.to_summary(),
            method: session.method,
            position_ms: play.position_ms,
            state: play.state,
            started_at: session.started_at,
            updated_at: play.updated_at,
            transcode: session.job_id().and_then(|id| jobs.get(&id)).map(transcode_info),
        });
    }
    let analysis = if user.is_admin { state.markers.status() } else { None };
    let previews_pending = if user.is_admin { state.trickplay.pending() } else { 0 };
    // Scans of libraries the user can't open stay private (their names too).
    let libraries = access::library_ids(&state.db, &user).await?;
    let scans = state
        .scanner
        .statuses()
        .into_iter()
        .filter(|s| libraries.as_ref().is_none_or(|ids| ids.contains(&s.library_id)))
        .collect();
    Ok(Json(Activity { scans, sessions: out, analysis, previews_pending }))
}

/// How often an open event stream checks its session again.
const RECHECK: Duration = Duration::from_secs(30);

pub async fn events(
    State(state): State<AppState>,
    user: AuthUser,
    CurrentToken(token): CurrentToken,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Events about libraries the user can't open aren't theirs to see.
    let libraries = Arc::new(parking_lot::RwLock::new(access::library_ids(&state.db, &user).await?));
    // The session is checked again every so often: the stream ends once it
    // is signed out (or its user disabled), and follows changes of access.
    let ended = tokio_util::sync::CancellationToken::new();
    {
        let (state, libraries, ended) = (state.clone(), libraries.clone(), ended.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(RECHECK);
            tick.tick().await;
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    () = ended.cancelled() => break,
                }
                let Some(user) = (match token.as_deref() {
                    Some(token) => state.auth.validate(token).await,
                    None => None,
                }) else {
                    ended.cancel();
                    break;
                };
                if let Ok(ids) = access::library_ids(&state.db, &user).await {
                    *libraries.write() = ids;
                }
            }
        });
    }
    // Dropped with the stream (the page went away): the check above stops.
    let gone = ended.clone().drop_guard();
    let stream = BroadcastStream::new(state.events.subscribe())
        .filter_map(|event| event.ok())
        .filter(move |event| match (event.library_id(), &*libraries.read()) {
            (Some(id), Some(ids)) => ids.contains(&id),
            _ => true,
        })
        .map(move |event| {
            let _ = &gone;
            Ok(Event::default().json_data(event).unwrap_or_else(|_| Event::default().comment("encode error")))
        });
    // The stream never ends on its own and the web UI keeps one open while
    // someone is signed in: end it when the server stops, or the graceful
    // shutdown would wait for it.
    let stream = futures::StreamExt::take_until(stream, state.stopping.clone().cancelled_owned());
    let stream = futures::StreamExt::take_until(stream, ended.cancelled_owned());
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
