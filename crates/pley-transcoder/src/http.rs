// SPDX-License-Identifier: LicenseRef-Restricted
//! HTTP API of the transcoder (see docs/TRANSCODER.md).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use pley_transcoder_api::{
    CoverRequest, JobCreated, JobRequest, JobStatus, KeyframesRequest, MediaInfo, ProbeRequest, SubtitleRequest,
    ThumbnailRequest, TranscoderInfo, TranscoderStatus,
};
use tokio_util::io::ReaderStream;

use crate::error::{ApiError, ApiResult};
use crate::extract::Extractor;
use crate::job::JobManager;
use crate::keyframes::KeyframeStore;
use crate::metrics::{Metrics, render_prometheus};
use crate::probe::{Prober, Rejected};
use crate::util::dir_size;

pub struct AppState {
    pub jobs: Arc<JobManager>,
    pub prober: Arc<Prober>,
    pub extractor: Arc<Extractor>,
    pub keyframes: Arc<KeyframeStore>,
    pub metrics: Arc<Metrics>,
    pub info: TranscoderInfo,
    pub token: Option<String>,
    pub started: Instant,
    pub work_dir: PathBuf,
    /// Size of the work directory, refreshed in the background.
    work_dir_bytes: AtomicU64,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        jobs: Arc<JobManager>,
        prober: Arc<Prober>,
        extractor: Arc<Extractor>,
        keyframes: Arc<KeyframeStore>,
        metrics: Arc<Metrics>,
        info: TranscoderInfo,
        token: Option<String>,
        work_dir: PathBuf,
    ) -> Self {
        Self {
            jobs,
            prober,
            extractor,
            keyframes,
            metrics,
            info,
            token,
            started: Instant::now(),
            work_dir,
            work_dir_bytes: AtomicU64::new(0),
        }
    }

    /// Walk the work directory every few seconds on the blocking pool.
    pub fn spawn_disk_usage(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                tick.tick().await;
                let Some(state) = weak.upgrade() else { break };
                let dir = state.work_dir.clone();
                if let Ok(bytes) = tokio::task::spawn_blocking(move || dir_size(&dir)).await {
                    state.work_dir_bytes.store(bytes, Ordering::Relaxed);
                }
            }
        });
    }

    fn status(&self) -> TranscoderStatus {
        let (running, hw) = self.jobs.slots.counts();
        let work_dir_bytes = self.work_dir_bytes.load(Ordering::Relaxed);
        TranscoderStatus {
            version: self.info.version.clone(),
            uptime_secs: self.started.elapsed().as_secs(),
            ffmpeg_version: self.info.capabilities.ffmpeg_version.clone(),
            max_jobs: self.jobs.limits.max_jobs,
            running_processes: running,
            hw_processes: hw,
            max_hw_jobs: self.jobs.limits.max_hw_jobs,
            work_dir_bytes,
            counters: self.metrics.snapshot(),
            jobs: self.jobs.list().iter().map(|j| j.status()).collect(),
        }
    }
}

type Shared = State<Arc<AppState>>;

pub fn router(state: Arc<AppState>) -> Router {
    let v1 = Router::new()
        .route("/info", get(info))
        .route("/status", get(status))
        .route("/probe", post(probe))
        .route("/thumbnail", post(thumbnail))
        .route("/cover", post(cover))
        .route("/subtitles", post(subtitles))
        .route("/keyframes", post(keyframes))
        .route("/jobs", get(list_jobs).post(create_job))
        .route("/jobs/{id}", get(get_job).delete(delete_job))
        .route("/jobs/{id}/touch", post(touch_job))
        .route("/jobs/{id}/{file}", get(job_file));
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .nest("/v1", v1)
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

async fn auth(State(state): Shared, request: Request, next: Next) -> Response {
    if let Some(expected) = &state.token {
        if request.uri().path() != "/health" {
            let presented = request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .unwrap_or_default();
            if !constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
                return ApiError::Unauthorized.into_response();
            }
        }
    }
    next.run(request).await
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn info(State(state): Shared) -> Json<TranscoderInfo> {
    Json(state.info.clone())
}

async fn status(State(state): Shared) -> Json<TranscoderStatus> {
    Json(state.status())
}

async fn metrics(State(state): Shared) -> impl IntoResponse {
    let body = render_prometheus(&state.status());
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body)
}

async fn probe(State(state): Shared, Json(req): Json<ProbeRequest>) -> ApiResult<Json<MediaInfo>> {
    let path = std::path::Path::new(&req.path);
    if !path.is_file() {
        return Err(ApiError::NotFound(format!("file not found: {}", req.path)));
    }
    let info = state.prober.probe(path).await.map_err(|e| {
        // Only a file ffprobe could not read is a definitive failure; timeouts
        // (sleeping NAS) and start-up problems are worth retrying later.
        if e.downcast_ref::<Rejected>().is_some() {
            ApiError::Failed(format!("{e:#}"))
        } else if e.downcast_ref::<tokio::time::error::Elapsed>().is_some() {
            ApiError::Timeout(format!("{e:#}"))
        } else {
            ApiError::Internal(e)
        }
    })?;
    Ok(Json((*info).clone()))
}

async fn thumbnail(State(state): Shared, Json(req): Json<ThumbnailRequest>) -> ApiResult<Response> {
    let image = state.extractor.thumbnail(&req).await?;
    Ok(([(header::CONTENT_TYPE, image.content_type)], image.bytes).into_response())
}

async fn cover(State(state): Shared, Json(req): Json<CoverRequest>) -> ApiResult<Response> {
    let image = state.extractor.cover(&req.path).await?;
    Ok(([(header::CONTENT_TYPE, image.content_type)], image.bytes).into_response())
}

async fn subtitles(State(state): Shared, Json(req): Json<SubtitleRequest>) -> ApiResult<Response> {
    let text = state.extractor.subtitle(&req.path, req.stream_index).await?;
    Ok(([(header::CONTENT_TYPE, "text/vtt; charset=utf-8")], text).into_response())
}

async fn keyframes(State(state): Shared, Json(req): Json<KeyframesRequest>) -> ApiResult<StatusCode> {
    let path = std::path::Path::new(&req.path);
    if !path.is_file() {
        return Err(ApiError::NotFound(format!("file not found: {}", req.path)));
    }
    if state.keyframes.get(path).is_some() {
        return Ok(StatusCode::OK);
    }
    state.keyframes.request(path);
    Ok(StatusCode::ACCEPTED)
}

async fn list_jobs(State(state): Shared) -> Json<Vec<JobStatus>> {
    Json(state.jobs.list().iter().map(|j| j.status()).collect())
}

async fn create_job(State(state): Shared, Json(req): Json<JobRequest>) -> ApiResult<(StatusCode, Json<JobCreated>)> {
    let created = state.jobs.create(req).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_job(State(state): Shared, Path(id): Path<String>) -> ApiResult<Json<JobStatus>> {
    let job = state.jobs.get(&id).ok_or_else(|| ApiError::JobNotFound(format!("job {id} not found")))?;
    Ok(Json(job.status()))
}

async fn delete_job(State(state): Shared, Path(id): Path<String>) -> ApiResult<StatusCode> {
    if state.jobs.remove(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::JobNotFound(format!("job {id} not found")))
    }
}

async fn touch_job(State(state): Shared, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let job = state.jobs.get(&id).ok_or_else(|| ApiError::JobNotFound(format!("job {id} not found")))?;
    job.touch();
    Ok(StatusCode::NO_CONTENT)
}

async fn job_file(State(state): Shared, Path((id, file)): Path<(String, String)>) -> ApiResult<Response> {
    let job = state.jobs.get(&id).ok_or_else(|| ApiError::JobNotFound(format!("job {id} not found")))?;
    if file == "index.m3u8" {
        job.touch();
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.apple.mpegurl"));
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        return Ok((headers, job.playlist()).into_response());
    }
    let n: u32 = file
        .strip_suffix(".ts")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| ApiError::NotFound(format!("unknown file {file}")))?;
    let path = state.jobs.segment(&job, n).await?;
    let handle = tokio::fs::File::open(&path).await?;
    let len = handle.metadata().await?.len();
    job.record_served(len);
    Metrics::inc(&state.metrics.segments_served);
    state.metrics.bytes_served.fetch_add(len, Ordering::Relaxed);
    let body = Body::from_stream(ReaderStream::with_capacity(handle, 256 * 1024));
    Response::builder()
        .header(header::CONTENT_TYPE, "video/mp2t")
        .header(header::CONTENT_LENGTH, len)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(body)
        .map_err(|e| ApiError::Internal(e.into()))
}
