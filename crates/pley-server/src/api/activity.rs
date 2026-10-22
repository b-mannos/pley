use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use pley_transcoder_api::JobStatus;
use serde::Serialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::AuthUser;
use crate::catalog::{self, ItemSummary};
use crate::error::ApiResult;
use crate::library::scanner::ScanStatus;
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
    Ok(Json(Activity { scans: state.scanner.statuses(), sessions: out }))
}

pub async fn events(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|event| event.ok()).map(|event| {
        Ok(Event::default().json_data(event).unwrap_or_else(|_| Event::default().comment("encode error")))
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
