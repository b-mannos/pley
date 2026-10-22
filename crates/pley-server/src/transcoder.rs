//! Client for the pley transcoder (a separate program; see LICENSING.md).
//!
//! The server never links or runs FFmpeg itself. In *managed* mode it spawns
//! the `pley-transcoder` executable (found next to the server binary unless
//! configured), reads the announced address, supervises it (restarting with
//! backoff) and forwards its log output. In *external* mode it talks to a
//! transcoder running elsewhere (`transcoder_url`).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use parking_lot::RwLock;
use pley_transcoder_api::{
    Announce, CoverRequest, ErrorBody, JobCreated, JobRequest, JobStatus, KeyframesRequest, MediaInfo, ProbeRequest,
    SubtitleRequest, TOKEN_ENV, ThumbnailRequest, TranscoderInfo, TranscoderStatus,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;

use crate::config::Config;
use crate::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum TranscoderError {
    #[error("the transcoder is not available{}", .0.as_deref().map(|e| format!(": {e}")).unwrap_or_default())]
    Unavailable(Option<String>),
    #[error("{message}")]
    Api { status: u16, code: String, message: String },
    #[error("transcoder request failed: {0}")]
    Http(#[from] reqwest::Error),
}

impl TranscoderError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, TranscoderError::Api { status: 404, .. })
    }
}

impl From<TranscoderError> for AppError {
    fn from(err: TranscoderError) -> Self {
        match err {
            TranscoderError::Unavailable(_) => AppError::Unavailable(err.to_string()),
            TranscoderError::Api { status: 404, message, .. } => AppError::NotFound(message),
            TranscoderError::Api { status: 400, message, .. } => AppError::BadRequest(message),
            TranscoderError::Api { status: 503, message, .. } => AppError::Unavailable(message),
            TranscoderError::Api { status: 504, message, .. } => AppError::Unavailable(message),
            other => AppError::Internal(anyhow::anyhow!(other.to_string())),
        }
    }
}

#[derive(Default)]
struct Node {
    base: Option<String>,
    info: Option<TranscoderInfo>,
    error: Option<String>,
}

pub struct Transcoder {
    http: reqwest::Client,
    token: String,
    managed: Option<ManagedSpec>,
    node: RwLock<Node>,
    ready: Notify,
    started: AtomicBool,
    stopping: AtomicBool,
    stop: Notify,
}

#[derive(Clone)]
struct ManagedSpec {
    binary: PathBuf,
    args: Vec<OsString>,
}

pub type TranscoderResult<T> = Result<T, TranscoderError>;

impl Transcoder {
    pub fn new(config: &Config) -> Arc<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .no_proxy()
            .build()
            .expect("http client");
        let (token, managed, base) = match &config.transcoder_url {
            Some(url) => {
                (config.transcoder_token.clone().unwrap_or_default(), None, Some(url.trim_end_matches('/').to_owned()))
            }
            None => {
                let token = crate::auth::new_token();
                let binary = config.transcoder_path.clone().unwrap_or_else(default_transcoder_path);
                let args: Vec<OsString> = vec![
                    "--listen".into(),
                    "127.0.0.1:0".into(),
                    "--announce".into(),
                    "--exit-with-parent".into(),
                    "--work-dir".into(),
                    config.transcode_dir.clone().into_os_string(),
                    "--ffmpeg".into(),
                    config.ffmpeg.clone().into_os_string(),
                    "--ffprobe".into(),
                    config.ffprobe.clone().into_os_string(),
                    "--max-jobs".into(),
                    config.transcoder_max_jobs.to_string().into(),
                ];
                (token, Some(ManagedSpec { binary, args }), None)
            }
        };
        Arc::new(Self {
            http,
            token,
            managed,
            node: RwLock::new(Node { base, ..Default::default() }),
            ready: Notify::new(),
            started: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            stop: Notify::new(),
        })
    }

    pub fn is_managed(&self) -> bool {
        self.managed.is_some()
    }

    pub fn start(self: &Arc<Self>) {
        self.started.store(true, Ordering::SeqCst);
        let this = self.clone();
        if let Some(spec) = self.managed.clone() {
            tokio::spawn(async move { this.supervise(spec).await });
        } else {
            tokio::spawn(async move { this.watch_external().await });
        }
    }

    pub async fn shutdown(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.stop.notify_waiters();
    }

    pub fn info(&self) -> Option<TranscoderInfo> {
        self.node.read().info.clone()
    }

    pub fn base_url(&self) -> Option<String> {
        self.node.read().base.clone()
    }

    pub fn last_error(&self) -> Option<String> {
        self.node.read().error.clone()
    }

    pub fn is_available(&self) -> bool {
        self.node.read().info.is_some()
    }

    /// Wait until the transcoder answered `/v1/info` (or the timeout elapses).
    /// Returns immediately when it was never started or failed permanently.
    pub async fn wait_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let notified = self.ready.notified();
            if self.is_available() {
                return true;
            }
            if !self.started.load(Ordering::SeqCst) || self.last_error().is_some_and(|e| e.contains("not found")) {
                return false;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let _ = tokio::time::timeout(deadline - now, notified).await;
        }
    }

    // ---------------------------------------------------------------- API

    pub async fn probe(&self, path: &str) -> TranscoderResult<MediaInfo> {
        self.post_json("/v1/probe", &ProbeRequest { path: path.to_owned() }, Duration::from_secs(150)).await
    }

    pub async fn thumbnail(&self, req: &ThumbnailRequest) -> TranscoderResult<Bytes> {
        let resp = self.post("/v1/thumbnail", req, Duration::from_secs(90)).await?;
        Ok(resp.bytes().await?)
    }

    pub async fn cover(&self, path: &str) -> TranscoderResult<(Bytes, String)> {
        let resp = self.post("/v1/cover", &CoverRequest { path: path.to_owned() }, Duration::from_secs(60)).await?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_owned();
        Ok((resp.bytes().await?, content_type))
    }

    pub async fn subtitles(&self, path: &str, stream_index: Option<i64>) -> TranscoderResult<String> {
        let req = SubtitleRequest { path: path.to_owned(), stream_index };
        let resp = self.post("/v1/subtitles", &req, Duration::from_secs(25 * 60)).await?;
        Ok(resp.text().await?)
    }

    pub async fn request_keyframes(&self, path: &str) -> TranscoderResult<()> {
        self.post("/v1/keyframes", &KeyframesRequest { path: path.to_owned() }, Duration::from_secs(30)).await?;
        Ok(())
    }

    pub async fn create_job(&self, req: &JobRequest) -> TranscoderResult<JobCreated> {
        self.post_json("/v1/jobs", req, Duration::from_secs(25 * 60)).await
    }

    pub async fn delete_job(&self, id: &str) -> TranscoderResult<()> {
        let base = self.base()?;
        let resp = self
            .authed(self.http.delete(format!("{base}/v1/jobs/{id}")))
            .timeout(Duration::from_secs(20))
            .send()
            .await?;
        if resp.status().is_success() || resp.status().as_u16() == 404 { Ok(()) } else { Err(api_error(resp).await) }
    }

    pub async fn touch_job(&self, id: &str) -> TranscoderResult<()> {
        self.post(&format!("/v1/jobs/{id}/touch"), &serde_json::json!({}), Duration::from_secs(10)).await?;
        Ok(())
    }

    pub async fn job_status(&self, id: &str) -> TranscoderResult<JobStatus> {
        self.get_json(&format!("/v1/jobs/{id}"), Duration::from_secs(10)).await
    }

    pub async fn status(&self) -> TranscoderResult<TranscoderStatus> {
        self.get_json("/v1/status", Duration::from_secs(10)).await
    }

    pub async fn playlist(&self, job_id: &str) -> TranscoderResult<String> {
        let base = self.base()?;
        let resp = self
            .authed(self.http.get(format!("{base}/v1/jobs/{job_id}/index.m3u8")))
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(api_error(resp).await);
        }
        Ok(resp.text().await?)
    }

    /// Streaming response of segment `n` (the caller forwards the body).
    pub async fn segment(&self, job_id: &str, n: u32) -> TranscoderResult<reqwest::Response> {
        let base = self.base()?;
        let resp = self
            .authed(self.http.get(format!("{base}/v1/jobs/{job_id}/{n}.ts")))
            .timeout(Duration::from_secs(120))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(api_error(resp).await);
        }
        Ok(resp)
    }

    fn base(&self) -> TranscoderResult<String> {
        let node = self.node.read();
        node.base.clone().ok_or_else(|| TranscoderError::Unavailable(node.error.clone()))
    }

    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.token.is_empty() { req } else { req.bearer_auth(&self.token) }
    }

    async fn post<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> TranscoderResult<reqwest::Response> {
        let base = self.base()?;
        let resp = self.authed(self.http.post(format!("{base}{path}"))).json(body).timeout(timeout).send().await?;
        if resp.status().is_success() { Ok(resp) } else { Err(api_error(resp).await) }
    }

    async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> TranscoderResult<T> {
        Ok(self.post(path, body, timeout).await?.json().await?)
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, timeout: Duration) -> TranscoderResult<T> {
        let base = self.base()?;
        let resp = self.authed(self.http.get(format!("{base}{path}"))).timeout(timeout).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp).await);
        }
        Ok(resp.json().await?)
    }

    async fn fetch_info(&self, base: &str) -> TranscoderResult<TranscoderInfo> {
        let resp =
            self.authed(self.http.get(format!("{base}/v1/info"))).timeout(Duration::from_secs(10)).send().await?;
        if !resp.status().is_success() {
            return Err(api_error(resp).await);
        }
        Ok(resp.json().await?)
    }

    fn set_available(&self, base: String, info: TranscoderInfo) {
        let mut node = self.node.write();
        node.base = Some(base);
        node.info = Some(info);
        node.error = None;
        drop(node);
        self.ready.notify_waiters();
    }

    fn set_unavailable(&self, error: String) {
        let mut node = self.node.write();
        if self.managed.is_some() {
            node.base = None;
        }
        node.info = None;
        node.error = Some(error);
    }

    // ---------------------------------------------------------- supervision

    async fn watch_external(self: Arc<Self>) {
        let Some(base) = self.base_url() else { return };
        loop {
            match self.fetch_info(&base).await {
                Ok(info) => {
                    if !self.is_available() {
                        tracing::info!("transcoder {} at {base} is available", info.version);
                    }
                    self.set_available(base.clone(), info);
                }
                Err(err) => {
                    if self.is_available() || self.last_error().is_none() {
                        tracing::warn!("transcoder at {base} unavailable: {err}");
                    }
                    self.set_unavailable(err.to_string());
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(15)) => {}
                _ = self.stop.notified() => return,
            }
        }
    }

    async fn supervise(self: Arc<Self>, spec: ManagedSpec) {
        let mut backoff = Duration::from_secs(1);
        while !self.stopping.load(Ordering::SeqCst) {
            if !spec.binary.is_file() {
                let message = format!("transcoder binary not found at {}", spec.binary.display());
                if self.last_error().as_deref() != Some(message.as_str()) {
                    tracing::error!("{message}; transcoding, probing and thumbnails are unavailable");
                }
                self.set_unavailable(message);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => continue,
                    _ = self.stop.notified() => return,
                }
            }
            let started = Instant::now();
            match self.run_once(&spec).await {
                Ok(()) => return,
                Err(message) => {
                    tracing::warn!("transcoder stopped: {message}");
                    self.set_unavailable(message);
                }
            }
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }
            if started.elapsed() > Duration::from_secs(60) {
                backoff = Duration::from_secs(1);
            }
            tracing::info!("restarting transcoder in {}s", backoff.as_secs());
            tokio::select! {
                _ = tokio::time::sleep(backoff) => {}
                _ = self.stop.notified() => return,
            }
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    /// Run the transcoder until it exits (`Err`) or we are shutting down (`Ok`).
    async fn run_once(&self, spec: &ManagedSpec) -> Result<(), String> {
        let mut cmd = tokio::process::Command::new(&spec.binary);
        cmd.args(&spec.args)
            .env(TOKEN_ENV, &self.token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let mut child = cmd.spawn().map_err(|e| format!("failed to start {}: {e}", spec.binary.display()))?;
        // Keep stdin open: the transcoder exits when it closes (we died).
        let _stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = child.stderr.take().ok_or("no stderr")?;

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                forward_log(&line);
            }
        });

        let mut stdout_lines = BufReader::new(stdout).lines();
        let announce = tokio::time::timeout(Duration::from_secs(120), async {
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                if let Ok(a) = serde_json::from_str::<Announce>(&line) {
                    if a.event == "listening" {
                        return Some(a.addr);
                    }
                }
            }
            None
        })
        .await;
        let addr = match announce {
            Ok(Some(addr)) => addr,
            Ok(None) => {
                let status = child.wait().await.map(|s| s.to_string()).unwrap_or_default();
                return Err(format!("transcoder exited before announcing its address ({status})"));
            }
            Err(_) => return Err("transcoder did not start within 120s".into()),
        };
        let base = format!("http://{addr}");
        match self.fetch_info(&base).await {
            Ok(info) => {
                tracing::info!(
                    "transcoder {} ({}) ready at {base}: ffmpeg {}, encoders [{}]",
                    info.version,
                    info.license,
                    info.capabilities.ffmpeg_version.as_deref().unwrap_or("not found"),
                    info.capabilities.encoders.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ")
                );
                self.set_available(base, info);
            }
            Err(err) => return Err(format!("transcoder at {base} did not answer: {err}")),
        }
        tokio::spawn(async move { while let Ok(Some(_)) = stdout_lines.next_line().await {} });

        tokio::select! {
            status = child.wait() => Err(format!("transcoder exited ({})", status.map(|s| s.to_string()).unwrap_or_default())),
            _ = self.stop.notified() => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                Ok(())
            }
        }
    }
}

fn default_transcoder_path() -> PathBuf {
    let name = if cfg!(windows) { "pley-transcoder.exe" } else { "pley-transcoder" };
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Re-emit a transcoder log line through our own logger at its level.
fn forward_log(line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let body =
        trimmed.split_once(" INFO ").or_else(|| trimmed.split_once(" WARN ")).or_else(|| trimmed.split_once(" ERROR "));
    let message = body.map(|(_, m)| m.trim()).unwrap_or(trimmed);
    if trimmed.contains(" ERROR ") {
        tracing::error!("[transcoder] {message}");
    } else if trimmed.contains(" WARN ") {
        tracing::warn!("[transcoder] {message}");
    } else if trimmed.contains(" DEBUG ") || trimmed.contains(" TRACE ") {
        tracing::debug!("[transcoder] {message}");
    } else {
        tracing::info!("[transcoder] {message}");
    }
}

async fn api_error(resp: reqwest::Response) -> TranscoderError {
    let status = resp.status().as_u16();
    match resp.json::<ErrorBody>().await {
        Ok(body) => TranscoderError::Api { status, code: body.code, message: body.message },
        Err(_) => {
            TranscoderError::Api { status, code: "http".into(), message: format!("transcoder returned HTTP {status}") }
        }
    }
}
