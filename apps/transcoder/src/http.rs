// SPDX-License-Identifier: LicenseRef-Restricted
//! HTTP API of the transcoder (see docs/TRANSCODER.md).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use pley_transcoder_api::{
    CoverRequest, Fingerprint, FingerprintRequest, Frames, FramesRequest, JobCreated, JobRequest, JobStatus,
    KeyframesRequest, MediaInfo, ProbeRequest, SUBTITLES_COMPLETE_HEADER, SubtitleRequest, ThumbnailRequest,
    TranscoderInfo, TranscoderStatus, TrickplayInfo, TrickplayRequest,
};
use tokio_util::io::ReaderStream;

use crate::error::{ApiError, ApiResult};
use crate::extract::Extractor;
use crate::fingerprint::Fingerprinter;
use crate::job::JobManager;
use crate::keyframes::KeyframeStore;
use crate::metrics::{Metrics, render_prometheus};
use crate::probe::{Prober, Rejected};
use crate::trickplay::Trickplay;
use crate::util::dir_size;

pub struct AppState {
    pub jobs: Arc<JobManager>,
    pub prober: Arc<Prober>,
    pub extractor: Arc<Extractor>,
    pub keyframes: Arc<KeyframeStore>,
    pub fingerprinter: Arc<Fingerprinter>,
    pub trickplay: Arc<Trickplay>,
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
        fingerprinter: Arc<Fingerprinter>,
        trickplay: Arc<Trickplay>,
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
            fingerprinter,
            trickplay,
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
        .route("/fingerprint", post(fingerprint))
        .route("/frames", post(frames))
        .route("/trickplay", post(trickplay))
        .route("/trickplay/{id}/{sheet}", get(trickplay_sheet))
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
    if let (true, Some(index)) = (req.partial, req.stream_index) {
        let (text, complete) = state.extractor.subtitle_so_far(&req.path, index, Duration::from_secs(2)).await?;
        let complete = if complete { "1" } else { "0" };
        return Ok((
            [
                (header::CONTENT_TYPE, "text/vtt; charset=utf-8"),
                (HeaderName::from_static(SUBTITLES_COMPLETE_HEADER), complete),
            ],
            text,
        )
            .into_response());
    }
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

async fn trickplay(State(state): Shared, Json(req): Json<TrickplayRequest>) -> ApiResult<Json<TrickplayInfo>> {
    Ok(Json(state.trickplay.generate(&req).await?))
}

async fn trickplay_sheet(State(state): Shared, Path((id, sheet)): Path<(String, String)>) -> ApiResult<Response> {
    let n: u32 = sheet
        .strip_suffix(".jpg")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| ApiError::NotFound(format!("unknown file {sheet}")))?;
    let path = state.trickplay.sheet_path(&id, n).ok_or_else(|| ApiError::NotFound("no such sheet".into()))?;
    let bytes = tokio::fs::read(path).await?;
    // Read whole: the set may go now that this sheet has been fetched.
    let trickplay = state.trickplay.clone();
    let _ = tokio::task::spawn_blocking(move || trickplay.sheet_served(&id, n)).await;
    Ok(([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response())
}

async fn fingerprint(State(state): Shared, Json(req): Json<FingerprintRequest>) -> ApiResult<Json<Fingerprint>> {
    Ok(Json(state.fingerprinter.fingerprint(&req).await?))
}

async fn frames(State(state): Shared, Json(req): Json<FramesRequest>) -> ApiResult<Json<Frames>> {
    Ok(Json(state.fingerprinter.frames(&req).await?))
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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use pley_transcoder_api::{Capabilities, ErrorBody, Limits};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::ffmpeg::Ffmpeg;

    /// A transcoder whose FFmpeg is missing, on a free local port.
    async fn serve(dir: &std::path::Path, token: Option<&str>) -> SocketAddr {
        let ff = Arc::new(Ffmpeg {
            ffmpeg: dir.join("no-ffmpeg"),
            ffprobe: dir.join("no-ffprobe"),
            caps: Capabilities::default(),
            segment_times_relative: true,
        });
        let metrics = Arc::new(Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics.clone(), 1));
        let keyframes = KeyframeStore::new(ff.clone(), metrics.clone(), dir.join("keyframes"));
        let extractor = Arc::new(Extractor::new(ff.clone(), prober.clone(), metrics.clone(), dir.join("subtitles"), 1));
        let fingerprinter = Arc::new(Fingerprinter::new(ff.clone(), 1));
        let trickplay = Arc::new(Trickplay::new(ff.clone(), prober.clone(), dir.join("trickplay")));
        let limits = Limits { max_jobs: 3, max_hw_jobs: 1 };
        let jobs = JobManager::new(
            ff.clone(),
            prober.clone(),
            keyframes.clone(),
            extractor.clone(),
            metrics.clone(),
            dir.join("jobs"),
            limits.clone(),
        );
        let info =
            TranscoderInfo { version: "1.2.3".into(), capabilities: ff.caps.clone(), limits, ..Default::default() };
        let state = Arc::new(AppState::new(
            jobs,
            prober,
            extractor,
            keyframes,
            fingerprinter,
            trickplay,
            metrics,
            info,
            token.map(str::to_owned),
            dir.to_path_buf(),
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
        addr
    }

    struct Answer {
        status: u16,
        body: String,
    }

    impl Answer {
        fn code(&self) -> String {
            serde_json::from_str::<ErrorBody>(&self.body).map(|e| e.code).unwrap_or_default()
        }
    }

    /// One HTTP/1.1 request, read until the server closes the connection.
    async fn request(addr: SocketAddr, method: &str, path: &str, auth: Option<&str>, json: Option<String>) -> Answer {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let body = json.unwrap_or_default();
        let mut message = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
        if let Some(auth) = auth {
            message.push_str(&format!("Authorization: {auth}\r\n"));
        }
        message.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()));
        stream.write_all(message.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").expect("an HTTP response");
        let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).expect("a status line");
        Answer { status, body: body.to_owned() }
    }

    #[tokio::test]
    async fn the_token_guards_everything_but_health() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), Some("s3cret")).await;
        let health = request(addr, "GET", "/health", None, None).await;
        assert_eq!((health.status, health.body.as_str()), (200, "ok"));
        for auth in [
            None,
            Some("Bearer wrong"),
            Some("Bearer s3cret2"),
            Some("Bearer s3cre"),
            Some("bearer s3cret"),
            Some("s3cret"),
        ] {
            for path in ["/v1/info", "/v1/status", "/v1/jobs", "/metrics"] {
                let answer = request(addr, "GET", path, auth, None).await;
                assert_eq!((answer.status, answer.code()), (401, "unauthorized".to_owned()), "{path} with {auth:?}");
            }
        }
        let info = request(addr, "GET", "/v1/info", Some("Bearer s3cret"), None).await;
        assert_eq!(info.status, 200);
        assert_eq!(serde_json::from_str::<TranscoderInfo>(&info.body).unwrap().version, "1.2.3");
    }

    #[tokio::test]
    async fn status_and_metrics() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), None).await;
        let status = request(addr, "GET", "/v1/status", None, None).await;
        assert_eq!(status.status, 200, "no token configured: open");
        let status: TranscoderStatus = serde_json::from_str(&status.body).unwrap();
        assert_eq!((status.max_jobs, status.max_hw_jobs, status.running_processes), (3, 1, 0));
        assert!(status.jobs.is_empty());
        let metrics = request(addr, "GET", "/metrics", None, None).await;
        assert_eq!(metrics.status, 200);
        assert!(metrics.body.contains("# TYPE pley_transcoder_jobs_created_total counter\n"), "{}", metrics.body);
        assert!(metrics.body.contains("pley_transcoder_max_jobs 3\n"), "{}", metrics.body);
    }

    #[tokio::test]
    async fn unknown_jobs_are_reported_as_gone() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), None).await;
        for (method, path) in [
            ("GET", "/v1/jobs/nope"),
            ("DELETE", "/v1/jobs/nope"),
            ("POST", "/v1/jobs/nope/touch"),
            ("GET", "/v1/jobs/nope/index.m3u8"),
            ("GET", "/v1/jobs/nope/3.ts"),
        ] {
            let answer = request(addr, method, path, None, None).await;
            assert_eq!((answer.status, answer.code()), (404, "job_not_found".to_owned()), "{method} {path}");
        }
    }

    #[tokio::test]
    async fn missing_files_and_bad_names_are_plain_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), None).await;
        let missing = serde_json::json!({"path": dir.path().join("missing.mkv")}).to_string();
        for path in ["/v1/probe", "/v1/keyframes", "/v1/trickplay"] {
            let answer = request(addr, "POST", path, None, Some(missing.clone())).await;
            assert_eq!((answer.status, answer.code()), (404, "not_found".to_owned()), "{path}");
        }
        // Sheet ids can't leave the previews folder.
        std::fs::create_dir_all(dir.path().join("secret")).unwrap();
        std::fs::write(dir.path().join("secret").join("0.jpg"), b"jpeg").unwrap();
        for path in ["/v1/trickplay/..%2Fsecret/0.jpg", "/v1/trickplay/a.b/0.jpg", "/v1/trickplay/abc/first.jpg"] {
            let answer = request(addr, "GET", path, None, None).await;
            assert_eq!((answer.status, answer.code()), (404, "not_found".to_owned()), "{path}");
        }
    }

    #[tokio::test]
    async fn preview_sheets_go_once_all_have_been_fetched() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), None).await;
        let id = "0123abcd-2000-320";
        let set = dir.path().join("trickplay").join(id);
        std::fs::create_dir_all(&set).unwrap();
        let info = TrickplayInfo {
            id: id.into(),
            interval_ms: 2_000,
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            count: 150,
            sheets: 2,
        };
        std::fs::write(set.join("index.json"), serde_json::to_vec(&info).unwrap()).unwrap();
        std::fs::write(set.join("0.jpg"), b"sheet 0").unwrap();
        std::fs::write(set.join("1.jpg"), b"sheet 1").unwrap();
        let sheet = |n: u32| {
            let path = format!("/v1/trickplay/{id}/{n}.jpg");
            async move { request(addr, "GET", &path, None, None).await }
        };
        // In any order, more than once…
        assert_eq!(sheet(1).await.status, 200);
        assert_eq!(sheet(1).await.status, 200);
        assert!(set.exists(), "sheet 0 has not been fetched yet");
        let last = sheet(0).await;
        assert_eq!((last.status, last.body.as_str()), (200, "sheet 0"));
        // …and once every sheet has been fetched, the server has its copy.
        assert!(!set.exists());
        assert_eq!(sheet(0).await.status, 404);
    }

    #[tokio::test]
    async fn jobs_need_a_working_ffmpeg() {
        let dir = tempfile::tempdir().unwrap();
        let addr = serve(dir.path(), None).await;
        let input = dir.path().join("movie.mkv");
        std::fs::write(&input, b"not really a movie").unwrap();
        let job = serde_json::json!({"input": input, "video": {}, "audio": {}}).to_string();
        let answer = request(addr, "POST", "/v1/jobs", None, Some(job)).await;
        assert_eq!((answer.status, answer.code()), (500, "ffmpeg_failed".to_owned()));
        let jobs = request(addr, "GET", "/v1/jobs", None, None).await;
        assert_eq!(jobs.body, "[]", "nothing was left behind");
    }
}
