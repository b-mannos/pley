// SPDX-License-Identifier: LicenseRef-Restricted
//! HLS transcode jobs.
//!
//! A job owns a fixed segment layout (its VOD playlist never changes) and at
//! most one ffmpeg process producing a window of segments. Segment requests
//! are served from disk when ready; otherwise the running process is awaited
//! when it is about to produce the segment, or a new process is started right
//! at the requested segment (seeking). Idle processes are stopped, idle jobs
//! expire, and segments far behind the playhead are pruned.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use pley_transcoder_api::{
    EncoderInfo, JobCreated, JobRequest, JobState, JobStatus, Limits, MediaInfo, ProcessStatus, VideoCodec, VideoMode,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::sync::{Notify, oneshot, watch};

use crate::adaptive;
use crate::error::{ApiError, ApiResult};
use crate::extract::Extractor;
use crate::ffmpeg::Ffmpeg;
use crate::keyframes::KeyframeStore;
use crate::metrics::Metrics;
use crate::plan::{ArgsContext, Burn, MAX_DURATION_MS, PROCESS_WINDOW, Plan};
use crate::probe::Prober;
use crate::util::now_ms;

/// Wait for the running process instead of restarting when the requested
/// segment is at most this many segments ahead of it.
const LOOKAHEAD: u32 = 3;
const SEGMENT_WAIT: Duration = Duration::from_secs(45);
/// Stop ffmpeg when nobody requested anything for this long (paused player).
const PROCESS_IDLE_MS: i64 = 60_000;
/// Forget jobs nobody touched for this long.
const JOB_EXPIRE_MS: i64 = 20 * 60_000;
/// Segments kept behind the most recently requested one.
const KEEP_BEHIND: u32 = 90;
/// Give up on a segment after this many failed process starts at it.
const MAX_FAILURES: u32 = 2;
/// A process younger than this is not killed for a far-away request while
/// another request is waiting for it (prevents two requests from restarting
/// each other forever).
const PREEMPT_GRACE: Duration = Duration::from_secs(3);
/// Segments deleted per lock acquisition when pruning.
const PRUNE_BATCH: usize = 32;

/// Concurrency budget shared by all jobs.
pub struct Slots {
    used: Mutex<(u32, u32)>,
    max: u32,
    max_hw: u32,
}

impl Slots {
    pub fn new(limits: &Limits) -> Self {
        Self { used: Mutex::new((0, 0)), max: limits.max_jobs.max(1), max_hw: limits.max_hw_jobs }
    }

    /// Reserve a process slot. Returns whether a hardware session was granted,
    /// or `None` when every slot is taken.
    fn acquire(&self, want_hw: bool) -> Option<bool> {
        let mut used = self.used.lock();
        if used.0 >= self.max {
            return None;
        }
        used.0 += 1;
        let hw = want_hw && used.1 < self.max_hw;
        if hw {
            used.1 += 1;
        }
        Some(hw)
    }

    fn release(&self, hw: bool) {
        let mut used = self.used.lock();
        used.0 = used.0.saturating_sub(1);
        if hw {
            used.1 = used.1.saturating_sub(1);
        }
    }

    pub fn counts(&self) -> (u32, u32) {
        *self.used.lock()
    }
}

pub struct JobManager {
    ff: Arc<Ffmpeg>,
    prober: Arc<Prober>,
    keyframes: Arc<KeyframeStore>,
    extractor: Arc<Extractor>,
    metrics: Arc<Metrics>,
    jobs_dir: PathBuf,
    pub limits: Limits,
    pub slots: Arc<Slots>,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    /// Set on shutdown: no new jobs are accepted.
    closed: AtomicBool,
}

pub struct Job {
    pub id: String,
    pub request: JobRequest,
    pub media: Arc<MediaInfo>,
    pub plan: Plan,
    pub input: PathBuf,
    pub dir: PathBuf,
    pub created_at_ms: i64,
    last_access_ms: AtomicI64,
    last_requested: AtomicU32,
    ready: Mutex<Vec<bool>>,
    ready_count: AtomicU32,
    /// Segments that requests are currently waiting for (one entry per request).
    waiting: Mutex<Vec<u32>>,
    /// Set when the job is removed; no process may be started afterwards.
    closed: AtomicBool,
    notify: Notify,
    process: tokio::sync::Mutex<Option<Proc>>,
    runtime: Mutex<Runtime>,
    served_segments: AtomicU64,
    served_bytes: AtomicU64,
}

#[derive(Default)]
struct Runtime {
    state: JobState,
    process: Option<ProcessStatus>,
    last_error: Option<String>,
    restarts: u32,
    started_any: bool,
    warnings: Vec<String>,
    encoder_override: Option<EncoderInfo>,
    failures: HashMap<u32, u32>,
    /// The automatic preset of the running process (index in
    /// [`adaptive::LADDER`]), and when it started or moved.
    preset: Option<(usize, Instant)>,
}

struct Proc {
    end: u32,
    started: Instant,
    next: Arc<AtomicU32>,
    exited: watch::Receiver<bool>,
    kill: Option<oneshot::Sender<()>>,
}

impl Proc {
    fn running(&self) -> bool {
        !*self.exited.borrow()
    }

    /// Whether this process will produce segment `n` soon, so a request for it
    /// should wait rather than restart ffmpeg. Segments the process already
    /// passed (pruned or skipped) are not coming back.
    fn will_produce(&self, n: u32) -> bool {
        let next = self.next.load(Relaxed);
        n >= next && n < self.end && n <= next + LOOKAHEAD
    }

    async fn stop(mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
        let _ = tokio::time::timeout(Duration::from_secs(10), self.exited.wait_for(|exited| *exited)).await;
    }
}

impl JobManager {
    pub fn new(
        ff: Arc<Ffmpeg>,
        prober: Arc<Prober>,
        keyframes: Arc<KeyframeStore>,
        extractor: Arc<Extractor>,
        metrics: Arc<Metrics>,
        jobs_dir: PathBuf,
        limits: Limits,
    ) -> Arc<Self> {
        Arc::new(Self {
            slots: Arc::new(Slots::new(&limits)),
            ff,
            prober,
            keyframes,
            extractor,
            metrics,
            jobs_dir,
            limits,
            jobs: Mutex::default(),
            closed: AtomicBool::new(false),
        })
    }

    pub fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.jobs.lock().get(id).cloned()
    }

    pub fn list(&self) -> Vec<Arc<Job>> {
        let mut jobs: Vec<_> = self.jobs.lock().values().cloned().collect();
        jobs.sort_by_key(|j| j.created_at_ms);
        jobs
    }

    pub async fn create(self: &Arc<Self>, req: JobRequest) -> ApiResult<JobCreated> {
        if self.closed.load(Relaxed) {
            return Err(ApiError::Busy("the transcoder is shutting down".into()));
        }
        if !self.ff.caps.ffmpeg_available {
            return Err(ApiError::Failed("ffmpeg is not available on the transcoder host".into()));
        }
        let input = PathBuf::from(&req.input);
        match tokio::fs::metadata(&input).await {
            Ok(m) if m.is_file() => {}
            Ok(_) => return Err(ApiError::BadRequest(format!("{} is not a file", req.input))),
            Err(_) => return Err(ApiError::NotFound(format!("input not found: {}", req.input))),
        }
        let wants_copy = req.video.as_ref().is_some_and(|v| v.mode == VideoMode::Copy);
        // Remuxing needs the container start time (keyframe times are absolute,
        // seeking is relative to it); media info from older servers lacks it.
        // An implausible duration (stored from an older transcoder's probe) is
        // probed again: the probe now looks past it.
        let media = match req
            .media
            .clone()
            .filter(|m| m.duration_ms.is_some_and(|d| d > 0 && d <= MAX_DURATION_MS) && !m.streams.is_empty())
            .filter(|m| !wants_copy || m.start_ms.is_some())
        {
            Some(media) => Arc::new(media),
            None => self.prober.probe(&input).await.map_err(|e| ApiError::Failed(format!("probe failed: {e:#}")))?,
        };
        let keyframes = if wants_copy {
            let index = self.keyframes.get_or_read(&input).await;
            if index.is_none() {
                self.keyframes.request(&input);
            }
            index
        } else {
            None
        };
        let plan = Plan::build(&req, &media, &self.ff, keyframes.as_deref().map(Vec::as_slice))
            .map_err(ApiError::BadRequest)?;

        let id = uuid::Uuid::new_v4().simple().to_string();
        let dir = self.jobs_dir.join(&id);
        tokio::fs::create_dir_all(&dir).await?;
        let unmade = Unmade(Some(dir.clone()));
        if let Some(Burn::Text { source, file_name, .. }) = &plan.burn {
            self.extractor.extract_ass(&input, source, &dir.join(file_name)).await?;
        }

        let count = plan.segment_count() as usize;
        let start_segment = plan.segment_for(req.start_ms.unwrap_or(0));
        let created = JobCreated {
            id: id.clone(),
            duration_ms: plan.duration_ms,
            segment_seconds: plan.segment_seconds,
            segment_count: plan.segment_count(),
            video: plan.video_plan(&media, None),
            audio: plan.audio_plan(&media),
            warnings: plan.warnings.clone(),
        };
        let job = Arc::new(Job {
            id: id.clone(),
            input,
            dir,
            created_at_ms: now_ms(),
            last_access_ms: AtomicI64::new(now_ms()),
            last_requested: AtomicU32::new(start_segment),
            ready: Mutex::new(vec![false; count]),
            ready_count: AtomicU32::new(0),
            waiting: Mutex::default(),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
            process: tokio::sync::Mutex::new(None),
            runtime: Mutex::new(Runtime::default()),
            served_segments: AtomicU64::new(0),
            served_bytes: AtomicU64::new(0),
            request: req,
            media,
            plan,
        });
        self.jobs.lock().insert(id.clone(), job.clone());
        unmade.made();
        Metrics::inc(&self.metrics.jobs_created);
        tracing::info!(
            job = %id,
            "new job '{}' for {}: {}",
            job.request.label.as_deref().unwrap_or("-"),
            job.request.owner.as_deref().unwrap_or("-"),
            describe(&created)
        );

        // Start producing right away so the first segment request is fast.
        if let Err(err) = self.ensure_producing(&job, start_segment).await {
            if matches!(err, ApiError::Busy(_)) {
                self.remove(&id).await;
                return Err(err);
            }
            tracing::warn!(job = %id, "could not pre-start transcode: {err:?}");
        }
        Ok(created)
    }

    /// Path of segment `n`, waiting for or (re)starting ffmpeg as needed.
    pub async fn segment(self: &Arc<Self>, job: &Arc<Job>, n: u32) -> ApiResult<PathBuf> {
        if n >= job.plan.segment_count() {
            return Err(ApiError::NotFound(format!("segment {n} out of range")));
        }
        job.touch();
        job.last_requested.store(n, Relaxed);
        let _waiting = Waiting::register(job, n);
        let deadline = Instant::now() + SEGMENT_WAIT;
        loop {
            let notified = job.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if job.is_ready(n) {
                return Ok(job.segment_path(n));
            }
            self.ensure_producing(job, n).await?;
            if job.is_ready(n) {
                return Ok(job.segment_path(n));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(ApiError::Timeout(format!("segment {n} was not ready in time")));
            }
            let wait = (deadline - now).min(Duration::from_secs(1));
            tokio::select! {
                _ = &mut notified => {}
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    async fn ensure_producing(self: &Arc<Self>, job: &Arc<Job>, n: u32) -> ApiResult<()> {
        let mut guard = job.process.lock().await;
        // Checked under the process lock: `remove` sets the flag before taking
        // the lock, so no process can be started for a removed job.
        if job.closed.load(Relaxed) {
            return Err(ApiError::JobNotFound(format!("job {} was removed", job.id)));
        }
        if job.is_ready(n) {
            return Ok(());
        }
        if let Some(p) = guard.as_ref().filter(|p| p.running()) {
            if p.will_produce(n) {
                return Ok(());
            }
            // Another request is waiting for this young process: let it be
            // served first. This request retries after the next segment or second.
            if p.started.elapsed() < PREEMPT_GRACE && job.is_awaited(|m| m != n && p.will_produce(m)) {
                return Ok(());
            }
        }
        {
            let rt = job.runtime.lock();
            if rt.failures.get(&n).copied().unwrap_or(0) >= MAX_FAILURES {
                return Err(ApiError::Failed(rt.last_error.clone().unwrap_or_else(|| "transcode failed".into())));
            }
        }
        if let Some(old) = guard.take() {
            old.stop().await;
        }
        *guard = Some(self.spawn(job, n, None)?);
        Ok(())
    }

    /// Software encodes running now.
    fn software_encodes(&self) -> u32 {
        self.list().iter().filter(|job| job.encodes_in_software()).count() as u32
    }

    /// What the automatic preset depends on for `job` now, counting it.
    fn load_for(&self, job: &Job) -> adaptive::Load {
        let (pixels, hevc) = job
            .plan
            .video
            .as_ref()
            .map_or((0, false), |v| (v.width.max(0) as u64 * v.height.max(0) as u64, v.codec == VideoCodec::Hevc));
        let others = self.list().iter().filter(|j| j.id != job.id && j.encodes_in_software()).count() as u32;
        adaptive::Load {
            software_jobs: others + 1,
            cores: std::thread::available_parallelism().map_or(4, |n| n.get() as u32),
            pixels,
            hevc,
        }
    }

    /// Moves jobs on the automatic preset a step faster or slower when their
    /// lead over the player and their speed call for it (see
    /// [`crate::adaptive`]), restarting ffmpeg where it was.
    async fn adapt(self: &Arc<Self>) {
        if self.software_encodes() == 0 {
            return;
        }
        for job in self.list() {
            let Some((current, moved_at)) = job.runtime.lock().preset else { continue };
            let Ok(mut guard) = job.process.try_lock() else { continue };
            let Some(next) = guard.as_ref().filter(|p| p.running()).map(|p| p.next.load(Relaxed)) else { continue };
            if guard.as_ref().is_some_and(|p| next >= p.end) {
                continue;
            }
            let speed = job.runtime.lock().process.as_ref().and_then(|p| p.speed);
            let lead = job.lead_secs(next);
            let target = adaptive::target(self.load_for(&job));
            let to = match adaptive::decide(current, target, lead, speed, moved_at.elapsed()) {
                adaptive::Move::Faster => current - 1,
                adaptive::Move::Slower => current + 1,
                adaptive::Move::Stay => continue,
            };
            tracing::info!(
                job = %job.id,
                "preset {} → {} ({lead:.0} s ahead of the player at {}× real time)",
                adaptive::LADDER[current],
                adaptive::LADDER[to],
                speed.map_or("?".into(), |s| format!("{s:.1}"))
            );
            if let Some(old) = guard.take() {
                old.stop().await;
            }
            match self.spawn(&job, next, Some(to)) {
                Ok(process) => *guard = Some(process),
                Err(err) => tracing::warn!(job = %job.id, "could not restart at a new preset: {err:?}"),
            }
            drop(guard);
            job.notify.notify_waiters();
        }
    }

    fn spawn(self: &Arc<Self>, job: &Arc<Job>, start: u32, preset: Option<usize>) -> ApiResult<Proc> {
        let plan = &job.plan;
        // Stop before the first segment that is already on disk: ffmpeg would
        // truncate and rewrite it while it may be being served.
        let limit = (start + PROCESS_WINDOW).min(plan.segment_count());
        let end = job.first_ready(start + 1, limit).unwrap_or(limit);
        let planned = plan.video.as_ref().filter(|v| !v.copy).and_then(|v| v.encoder.as_ref());
        let want_hw = planned.is_some_and(|e| e.hardware);
        let hw = self
            .slots
            .acquire(want_hw)
            .ok_or_else(|| ApiError::Busy(format!("all {} transcode slots are in use", self.limits.max_jobs)))?;
        let fallback = if want_hw && !hw {
            let Some(software) = planned.and_then(|e| self.ff.software_encoder(e.codec)) else {
                self.slots.release(false);
                return Err(ApiError::Busy(format!(
                    "all {} hardware encoder sessions are in use and no software encoder is available",
                    self.limits.max_hw_jobs
                )));
            };
            job.warn("hardware encoder sessions exhausted, using software encoding");
            Some(software)
        } else {
            None
        };

        // The software encoder's preset: the planned one, or the automatic
        // one, which starts a step faster when nothing is ready ahead.
        let software = planned.is_some() && (!want_hw || fallback.is_some());
        let auto = job.plan.video.as_ref().is_some_and(|v| v.preset == adaptive::AUTO);
        let chosen = (software && auto).then(|| {
            preset.unwrap_or_else(|| adaptive::start(adaptive::target(self.load_for(job)), job.lead_secs(start)))
        });
        let preset_name = match chosen {
            Some(i) => Some(adaptive::LADDER[i].to_owned()),
            None if software => job.plan.video.as_ref().map(|v| v.preset.clone()),
            None => None,
        };
        let ctx = ArgsContext {
            input: &job.input,
            encoder: fallback,
            relative_split_times: self.ff.segment_times_relative,
            preset: preset_name.as_deref(),
        };
        let args = plan.ffmpeg_args(&ctx, start, end);
        tracing::debug!(job = %job.id, "ffmpeg {}", args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
        let mut cmd = self.ff.command();
        cmd.args(&args).current_dir(&job.dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                self.slots.release(hw);
                return Err(ApiError::Internal(anyhow::anyhow!("failed to start ffmpeg: {err}")));
            }
        };
        Metrics::inc(&self.metrics.processes_started);
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            self.slots.release(hw);
            return Err(ApiError::Internal(anyhow::anyhow!("ffmpeg pipes unavailable")));
        };

        let start_ms = plan.boundaries[start as usize];
        {
            let mut rt = job.runtime.lock();
            if rt.started_any {
                rt.restarts += 1;
            }
            rt.started_any = true;
            rt.state = JobState::Running;
            rt.encoder_override = fallback.cloned();
            rt.preset = chosen.map(|i| (i, Instant::now()));
            rt.process = Some(ProcessStatus {
                pid: child.id(),
                start_segment: start,
                started_at_ms: now_ms(),
                out_time_ms: start_ms,
                hardware: hw,
                preset: preset_name.clone(),
                ..Default::default()
            });
        }

        let next = Arc::new(AtomicU32::new(start));
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        tokio::spawn(read_segment_list(job.clone(), stdout, next.clone(), self.metrics.clone()));
        tokio::spawn(read_progress(job.clone(), stderr, start_ms, tail.clone()));

        let (exit_tx, exit_rx) = watch::channel(false);
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let slots = self.slots.clone();
        let metrics = self.metrics.clone();
        let job_ref = job.clone();
        let next_ref = next.clone();
        tokio::spawn(async move {
            let waited = tokio::select! {
                status = child.wait() => Some(status),
                _ = kill_rx => None,
            };
            let (status, killed) = match waited {
                Some(status) => (status, false),
                None => {
                    let _ = child.start_kill();
                    (child.wait().await, true)
                }
            };
            slots.release(hw);
            job_ref.on_exit(end, next_ref.load(Relaxed), status, killed, &tail, &metrics);
            let _ = exit_tx.send(true);
            job_ref.notify.notify_waiters();
        });

        Ok(Proc { end, started: Instant::now(), next, exited: exit_rx, kill: Some(kill_tx) })
    }

    pub async fn remove(&self, id: &str) -> bool {
        let Some(job) = self.jobs.lock().remove(id) else { return false };
        job.closed.store(true, Relaxed);
        if let Some(p) = job.process.lock().await.take() {
            p.stop().await;
        }
        // Wake waiting requests so they fail now instead of at their timeout.
        job.notify.notify_waiters();
        remove_dir_with_retry(&job.dir).await;
        tracing::info!(job = %id, "job removed");
        true
    }

    pub fn spawn_reaper(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                let Some(mgr) = weak.upgrade() else { break };
                mgr.reap().await;
            }
        });
    }

    async fn reap(self: &Arc<Self>) {
        self.adapt().await;
        let now = now_ms();
        for job in self.list() {
            let idle = now - job.last_access_ms.load(Relaxed);
            if idle > JOB_EXPIRE_MS {
                tracing::info!(job = %job.id, "job expired after {}s idle", idle / 1000);
                self.remove(&job.id).await;
                continue;
            }
            if idle > PROCESS_IDLE_MS {
                if let Ok(mut guard) = job.process.try_lock() {
                    if guard.as_ref().is_some_and(Proc::running) {
                        tracing::debug!(job = %job.id, "stopping idle ffmpeg process");
                        if let Some(p) = guard.take() {
                            p.stop().await;
                        }
                    }
                }
            }
            let pruned = job.clone();
            let _ = tokio::task::spawn_blocking(move || pruned.prune()).await;
        }
    }

    /// Stop every job (concurrently) and refuse new ones.
    pub async fn shutdown(&self) {
        self.closed.store(true, Relaxed);
        let ids: Vec<String> = self.jobs.lock().keys().cloned().collect();
        futures::future::join_all(ids.iter().map(|id| self.remove(id))).await;
    }
}

/// Registers a request as waiting for a segment for as long as it lives.
struct Waiting<'a> {
    job: &'a Job,
    n: u32,
}

impl<'a> Waiting<'a> {
    fn register(job: &'a Job, n: u32) -> Self {
        job.waiting.lock().push(n);
        Self { job, n }
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let mut waiting = self.job.waiting.lock();
        if let Some(i) = waiting.iter().position(|&m| m == self.n) {
            waiting.swap_remove(i);
        }
    }
}

impl Job {
    pub fn touch(&self) {
        self.last_access_ms.store(now_ms(), Relaxed);
    }

    /// Whether a process of this job is encoding video in software now.
    fn encodes_in_software(&self) -> bool {
        let rt = self.runtime.lock();
        rt.state == JobState::Running
            && rt.process.as_ref().is_some_and(|p| !p.hardware)
            && self.plan.video.as_ref().is_some_and(|v| !v.copy)
    }

    /// Seconds of video ready past the segment the player asked for last,
    /// with `next` the first segment not ready (negative: it is waiting).
    fn lead_secs(&self, next: u32) -> f64 {
        let asked = self.last_requested.load(Relaxed);
        (f64::from(next) - f64::from(asked) - 1.0) * self.plan.segment_seconds
    }

    pub fn segment_path(&self, n: u32) -> PathBuf {
        self.dir.join(format!("{n}.ts"))
    }

    fn is_ready(&self, n: u32) -> bool {
        self.ready.lock().get(n as usize).copied().unwrap_or(false)
    }

    /// First ready segment in `[from, to)`.
    fn first_ready(&self, from: u32, to: u32) -> Option<u32> {
        let ready = self.ready.lock();
        (from..to).find(|&n| ready.get(n as usize).copied().unwrap_or(false))
    }

    /// Whether any request is waiting for a segment matching `pred`.
    fn is_awaited(&self, pred: impl Fn(u32) -> bool) -> bool {
        self.waiting.lock().iter().any(|&m| pred(m))
    }

    fn mark_ready(&self, n: u32) {
        let mut ready = self.ready.lock();
        if let Some(slot) = ready.get_mut(n as usize) {
            if !*slot {
                *slot = true;
                self.ready_count.fetch_add(1, Relaxed);
            }
        }
        drop(ready);
        self.notify.notify_waiters();
    }

    fn warn(&self, message: &str) {
        let mut rt = self.runtime.lock();
        if !rt.warnings.iter().any(|w| w == message) {
            rt.warnings.push(message.to_owned());
        }
    }

    pub fn record_served(&self, bytes: u64) {
        self.served_segments.fetch_add(1, Relaxed);
        self.served_bytes.fetch_add(bytes, Relaxed);
    }

    fn on_exit(
        &self,
        end: u32,
        next: u32,
        status: std::io::Result<ExitStatus>,
        killed: bool,
        tail: &Mutex<VecDeque<String>>,
        metrics: &Metrics,
    ) {
        let success = status.as_ref().is_ok_and(ExitStatus::success);
        tracing::debug!(job = %self.id, "ffmpeg exited at segment {next} (killed: {killed}, success: {success})");
        let mut rt = self.runtime.lock();
        rt.process = None;
        if killed {
            rt.state = JobState::Idle;
        } else if success {
            rt.state = if end >= self.plan.segment_count() { JobState::Finished } else { JobState::Idle };
        } else {
            Metrics::inc(&metrics.process_failures);
            if rt.state != JobState::Failed {
                Metrics::inc(&metrics.jobs_failed);
            }
            let lines: Vec<String> = tail.lock().iter().cloned().collect();
            let message = if lines.is_empty() {
                match &status {
                    Ok(s) => format!("ffmpeg exited with {s}"),
                    Err(e) => format!("ffmpeg could not be awaited: {e}"),
                }
            } else {
                lines[lines.len().saturating_sub(4)..].join(" | ")
            };
            tracing::warn!(job = %self.id, "ffmpeg failed at segment {next}: {message}");
            rt.last_error = Some(message);
            rt.state = JobState::Failed;
            *rt.failures.entry(next).or_insert(0) += 1;
        }
    }

    /// Delete segments far behind the playhead. Blocking: run it on the
    /// blocking pool. The lock is held per batch so segment lookups never wait
    /// long, and a segment is only marked missing once its file is gone.
    fn prune(&self) {
        let last = self.last_requested.load(Relaxed);
        if last <= KEEP_BEHIND {
            return;
        }
        let cutoff = (last - KEEP_BEHIND) as usize;
        let mut n = 0;
        loop {
            let mut ready = self.ready.lock();
            let end = cutoff.min(ready.len());
            if n >= end {
                return;
            }
            let mut deleted = 0;
            while n < end && deleted < PRUNE_BATCH {
                if ready[n] && std::fs::remove_file(self.segment_path(n as u32)).is_ok() {
                    ready[n] = false;
                    self.ready_count.fetch_sub(1, Relaxed);
                    deleted += 1;
                }
                n += 1;
            }
        }
    }

    pub fn playlist(&self) -> String {
        let bounds = &self.plan.boundaries;
        let longest = bounds.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
        let target = ((longest as f64) / 1000.0).ceil().max(1.0) as i64;
        let mut out = String::with_capacity(bounds.len() * 24 + 256);
        out.push_str("#EXTM3U\n#EXT-X-VERSION:3\n");
        out.push_str(&format!("#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n"));
        if self.plan.video.is_some() {
            out.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");
        }
        for (i, w) in bounds.windows(2).enumerate() {
            out.push_str(&format!("#EXTINF:{:.3},\n{i}.ts\n", (w[1] - w[0]) as f64 / 1000.0));
        }
        out.push_str("#EXT-X-ENDLIST\n");
        out
    }

    pub fn status(&self) -> JobStatus {
        let rt = self.runtime.lock();
        let video = self.plan.video_plan(&self.media, rt.encoder_override.as_ref());
        let mut warnings = self.plan.warnings.clone();
        warnings.extend(rt.warnings.iter().cloned());
        JobStatus {
            id: self.id.clone(),
            label: self.request.label.clone(),
            owner: self.request.owner.clone(),
            input: self.request.input.clone(),
            state: rt.state,
            created_at_ms: self.created_at_ms,
            last_access_ms: self.last_access_ms.load(Relaxed),
            duration_ms: self.plan.duration_ms,
            segment_seconds: self.plan.segment_seconds,
            segment_count: self.plan.segment_count(),
            segments_ready: self.ready_count.load(Relaxed),
            video,
            audio: self.plan.audio_plan(&self.media),
            process: rt.process.clone(),
            restarts: rt.restarts,
            served_segments: self.served_segments.load(Relaxed),
            served_bytes: self.served_bytes.load(Relaxed),
            last_error: rt.last_error.clone(),
            warnings,
        }
    }

    fn update_progress(&self, progress: &HashMap<String, String>, start_ms: i64) {
        let mut rt = self.runtime.lock();
        let Some(p) = rt.process.as_mut() else { return };
        let num = |k: &str| progress.get(k).and_then(|v| v.trim().parse::<f64>().ok());
        p.frame = num("frame").map(|v| v as u64);
        p.fps = num("fps").filter(|v| *v > 0.0);
        p.speed =
            progress.get("speed").and_then(|v| v.trim().trim_end_matches('x').parse().ok()).filter(|v: &f64| *v > 0.0);
        p.bitrate_kbps = progress.get("bitrate").and_then(|v| v.trim().trim_end_matches("kbits/s").parse().ok());
        p.total_size = num("total_size").map(|v| v as u64);
        p.dup_frames = num("dup_frames").map(|v| v as u64);
        p.drop_frames = num("drop_frames").map(|v| v as u64);
        if let Some(us) = num("out_time_us").filter(|v| *v >= 0.0) {
            p.out_time_ms = start_ms + (us / 1000.0) as i64;
        }
    }
}

async fn read_segment_list(job: Arc<Job>, stdout: ChildStdout, next: Arc<AtomicU32>, metrics: Arc<Metrics>) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(n) = parse_list_line(&line) {
            tracing::trace!(job = %job.id, "segment {n} ready");
            job.mark_ready(n);
            next.store(n + 1, Relaxed);
            Metrics::inc(&metrics.segments_produced);
        }
    }
}

async fn read_progress(job: Arc<Job>, stderr: ChildStderr, start_ms: i64, tail: Arc<Mutex<VecDeque<String>>>) {
    const PROGRESS_KEYS: [&str; 12] = [
        "frame",
        "fps",
        "bitrate",
        "total_size",
        "out_time_us",
        "out_time_ms",
        "out_time",
        "dup_frames",
        "drop_frames",
        "speed",
        "progress",
        "stream_0_0_q",
    ];
    let mut lines = BufReader::new(stderr).lines();
    let mut block = HashMap::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if PROGRESS_KEYS.contains(&key) || key.starts_with("stream_") {
                if key == "progress" {
                    job.update_progress(&block, start_ms);
                    block.clear();
                } else {
                    block.insert(key.to_owned(), value.to_owned());
                }
                continue;
            }
        }
        let line = line.trim();
        if !line.is_empty() {
            let mut tail = tail.lock();
            tail.push_back(line.to_owned());
            if tail.len() > 20 {
                tail.pop_front();
            }
        }
    }
}

/// `"12.ts,48.000000,52.000000"` → `Some(12)`.
fn parse_list_line(line: &str) -> Option<u32> {
    let name = line.split(',').next()?.trim();
    let name = name.rsplit(['/', '\\']).next()?;
    name.strip_suffix(".ts")?.parse().ok()
}

fn describe(created: &JobCreated) -> String {
    let video = created.video.as_ref().map(|v| {
        format!(
            "video {} {}→{} {}x{}→{}x{}{}{}",
            if v.mode == VideoMode::Copy { "copy" } else { "transcode" },
            v.codec_in,
            v.codec_out,
            v.width_in.unwrap_or(0),
            v.height_in.unwrap_or(0),
            v.width_out.unwrap_or(0),
            v.height_out.unwrap_or(0),
            v.encoder.as_deref().map(|e| format!(" via {e}")).unwrap_or_default(),
            v.tonemap.map(|t| format!(" tonemap {}", t.as_str())).unwrap_or_default(),
        )
    });
    let audio = created
        .audio
        .as_ref()
        .map(|a| format!("audio {}→{} {}ch", a.codec_in, a.codec_out, a.channels_out.unwrap_or(0)));
    [video, audio, Some(format!("{} segments", created.segment_count))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
}

/// The folder of a job being made, removed with what's in it (a burn-in
/// track half written) when the job isn't made after all: an error, or its
/// request gone (the server gave up waiting, the player left).
struct Unmade(Option<PathBuf>);

impl Unmade {
    /// The job is made: its folder stays.
    fn made(mut self) {
        self.0 = None;
    }
}

impl Drop for Unmade {
    fn drop(&mut self) {
        let Some(dir) = self.0.take() else { return };
        // FFmpeg, killed with the request, may hold a file there a moment longer.
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move { remove_dir_with_retry(&dir).await });
            }
            Err(_) => {
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
}

async fn remove_dir_with_retry(dir: &Path) {
    for attempt in 0..5 {
        match tokio::fs::remove_dir_all(dir).await {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                if attempt == 4 {
                    tracing::warn!("could not remove {}: {e}", dir.display());
                }
                tokio::time::sleep(Duration::from_millis(200 * (attempt + 1))).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn folders_of_jobs_never_made_go() {
        let dir = tempfile::tempdir().unwrap();
        let (made, unmade) = (dir.path().join("made"), dir.path().join("unmade"));
        for job in [&made, &unmade] {
            std::fs::create_dir(job).unwrap();
            std::fs::write(job.join("burn.ass"), "[Script Info]\n").unwrap();
        }
        Unmade(Some(made.clone())).made();
        // Dropped midway (an error, or the request gone).
        drop(Unmade(Some(unmade.clone())));
        for _ in 0..50 {
            if !unmade.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!unmade.exists() && made.join("burn.ass").exists());
    }

    #[test]
    fn parses_segment_list_lines() {
        assert_eq!(parse_list_line("12.ts,48.000000,52.000000"), Some(12));
        assert_eq!(parse_list_line("C:\\tmp\\job\\3.ts,1,2"), Some(3));
        assert_eq!(parse_list_line("index.m3u8"), None);
    }

    #[test]
    fn waits_only_for_segments_ahead_of_the_process() {
        let (_exit_tx, exited) = watch::channel(false);
        let p = Proc { end: 20, started: Instant::now(), next: Arc::new(AtomicU32::new(10)), exited, kill: None };
        assert!(!p.will_produce(9), "already passed (pruned or skipped): must restart");
        assert!(p.will_produce(10));
        assert!(p.will_produce(13));
        assert!(!p.will_produce(14), "too far ahead: restart there");
        p.next.store(18, Relaxed);
        assert!(p.will_produce(19));
        assert!(!p.will_produce(20), "outside the process window");
    }

    #[test]
    fn slots_fall_back_to_software() {
        let slots = Slots::new(&Limits { max_jobs: 2, max_hw_jobs: 1 });
        assert_eq!(slots.acquire(true), Some(true));
        assert_eq!(slots.acquire(true), Some(false));
        assert_eq!(slots.acquire(false), None);
        slots.release(true);
        assert_eq!(slots.counts(), (1, 0));
        assert_eq!(slots.acquire(true), Some(true));
        // Releasing more than was taken never underflows.
        let slots = Slots::new(&Limits { max_jobs: 0, max_hw_jobs: 0 });
        slots.release(true);
        assert_eq!(slots.counts(), (0, 0));
        assert_eq!(slots.acquire(true), Some(false), "at least one job, in software");
    }

    /// A job with the given segment boundaries whose segments live in `dir`.
    fn job(dir: &Path, boundaries: Vec<i64>) -> Job {
        let count = boundaries.len() - 1;
        Job {
            id: "test".into(),
            request: JobRequest::default(),
            media: Arc::new(MediaInfo::default()),
            plan: Plan {
                duration_ms: *boundaries.last().unwrap(),
                split_at: boundaries.clone(),
                boundaries,
                segment_seconds: 4.0,
                video: None,
                audio: None,
                burn: None,
                throttle: false,
                warnings: vec![],
            },
            input: dir.join("input.mkv"),
            dir: dir.to_path_buf(),
            created_at_ms: 0,
            last_access_ms: AtomicI64::new(0),
            last_requested: AtomicU32::new(0),
            ready: Mutex::new(vec![false; count]),
            ready_count: AtomicU32::new(0),
            waiting: Mutex::default(),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
            process: tokio::sync::Mutex::new(None),
            runtime: Mutex::new(Runtime::default()),
            served_segments: AtomicU64::new(0),
            served_bytes: AtomicU64::new(0),
        }
    }

    /// A manager whose FFmpeg "works" but whose binaries are missing.
    fn manager(dir: &Path) -> Arc<JobManager> {
        let ff = Arc::new(Ffmpeg {
            ffmpeg: dir.join("no-ffmpeg"),
            ffprobe: dir.join("no-ffprobe"),
            caps: pley_transcoder_api::Capabilities { ffmpeg_available: true, ..Default::default() },
            segment_times_relative: true,
        });
        let metrics = Arc::new(Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics.clone(), 1));
        let keyframes = KeyframeStore::new(ff.clone(), metrics.clone(), dir.join("keyframes"));
        let extractor = Arc::new(Extractor::new(ff.clone(), prober.clone(), metrics.clone(), dir.join("subtitles"), 1));
        let limits = Limits { max_jobs: 2, max_hw_jobs: 0 };
        JobManager::new(ff, prober, keyframes, extractor, metrics, dir.join("jobs"), limits)
    }

    #[tokio::test]
    async fn implausible_durations_from_the_server_are_probed_again() {
        use pley_transcoder_api::{AudioRequest, MediaStream, StreamKind};
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.flac");
        std::fs::write(&input, b"not really audio").unwrap();
        let jobs = manager(dir.path());
        let request = |duration_ms: i64| JobRequest {
            input: input.to_string_lossy().into_owned(),
            media: Some(MediaInfo {
                duration_ms: Some(duration_ms),
                streams: vec![MediaStream {
                    index: 0,
                    kind: StreamKind::Audio,
                    codec: "flac".into(),
                    sample_rate: Some(44_100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            audio: Some(AudioRequest::default()),
            ..Default::default()
        };
        // A believable duration is used as it is (nothing is produced without
        // ffmpeg, but the job exists).
        let created = jobs.create(request(60_000)).await.unwrap();
        assert_eq!(created.segment_count, 15);
        // 1e12 s would be 250 billion segments: never trusted, the file is
        // probed again (and ffprobe is missing too).
        let err = jobs.create(request(1_000_000_000_000_000)).await.unwrap_err();
        assert!(matches!(&err, ApiError::Failed(m) if m.contains("probe failed")), "{err:?}");
        assert_eq!(jobs.list().len(), 1);
    }

    #[test]
    fn playlists_list_every_segment_with_its_length() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_400, 10_500]);
        assert_eq!(
            job.playlist(),
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:5\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXTINF:4.000,\n0.ts\n#EXTINF:4.400,\n1.ts\n#EXTINF:2.100,\n2.ts\n#EXT-X-ENDLIST\n",
            "the target duration covers the longest segment"
        );
    }

    #[test]
    fn segments_become_ready_once() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000, 12_000]);
        job.mark_ready(1);
        job.mark_ready(1);
        job.mark_ready(7);
        assert_eq!(job.ready_count.load(Relaxed), 1, "counted once; out of range ignored");
        assert!(job.is_ready(1) && !job.is_ready(0) && !job.is_ready(7));
        assert_eq!(job.first_ready(0, 3), Some(1));
        assert_eq!(job.first_ready(2, 3), None);
        assert_eq!(job.status().segments_ready, 1);
        assert_eq!(job.segment_path(2), dir.path().join("2.ts"));
        job.record_served(1_000);
        job.record_served(500);
        let status = job.status();
        assert_eq!((status.served_segments, status.served_bytes), (2, 1_500));
    }

    #[test]
    fn requests_count_as_waiting_while_they_wait() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000]);
        let first = Waiting::register(&job, 1);
        let second = Waiting::register(&job, 1);
        assert!(job.is_awaited(|n| n == 1));
        drop(first);
        assert!(job.is_awaited(|n| n == 1), "the other request still waits");
        drop(second);
        assert!(!job.is_awaited(|n| n == 1));
    }

    #[cfg(unix)]
    fn exit(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn exit(code: u32) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }

    #[test]
    fn process_exits_decide_the_job_state() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000]);
        let metrics = Metrics::default();
        let tail = Mutex::new(VecDeque::new());
        job.on_exit(2, 2, Ok(exit(0)), false, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Finished, "produced up to the end");
        job.on_exit(1, 1, Ok(exit(0)), false, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Idle, "the window ended before the file did");
        job.on_exit(2, 1, Ok(exit(1)), true, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Idle, "stopped on purpose is no failure");
        assert_eq!(metrics.process_failures.load(Relaxed), 0);

        // A failure keeps the end of ffmpeg's output.
        tail.lock().extend(["one", "two", "three", "four", "five"].map(String::from));
        job.on_exit(2, 1, Ok(exit(1)), false, &tail, &metrics);
        let status = job.status();
        assert_eq!(status.state, JobState::Failed);
        assert_eq!(status.last_error.as_deref(), Some("two | three | four | five"));
        job.on_exit(2, 1, Err(std::io::Error::other("gone")), false, &Mutex::new(VecDeque::new()), &metrics);
        assert!(job.status().last_error.unwrap().contains("could not be awaited: gone"));
        assert_eq!(metrics.process_failures.load(Relaxed), 2);
        assert_eq!(metrics.jobs_failed.load(Relaxed), 1, "a failing job counts once");
        assert_eq!(job.runtime.lock().failures.get(&1), Some(&2), "failures per start segment");
    }

    #[test]
    fn progress_lines_update_the_running_process() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000]);
        let block = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
        };
        job.update_progress(&block(&[("fps", "50")]), 0);
        assert!(job.status().process.is_none(), "no process, nothing to update");

        job.runtime.lock().process = Some(ProcessStatus::default());
        job.update_progress(
            &block(&[
                ("frame", "120"),
                ("fps", "48.5"),
                ("speed", " 2.1x"),
                ("bitrate", "3000.5kbits/s"),
                ("out_time_us", "5000000"),
                ("drop_frames", "2"),
            ]),
            12_000,
        );
        let p = job.status().process.unwrap();
        assert_eq!((p.frame, p.fps, p.speed, p.bitrate_kbps), (Some(120), Some(48.5), Some(2.1), Some(3000.5)));
        assert_eq!((p.out_time_ms, p.drop_frames), (17_000, Some(2)), "absolute media time");

        // Values ffmpeg doesn't know yet.
        job.update_progress(
            &block(&[("fps", "0.00"), ("speed", "N/A"), ("bitrate", "N/A"), ("out_time_us", "-9223372036854775807")]),
            12_000,
        );
        let p = job.status().process.unwrap();
        assert_eq!((p.fps, p.speed, p.bitrate_kbps), (None, None, None));
        assert_eq!(p.out_time_ms, 17_000, "a negative time keeps the last one");
    }

    #[test]
    fn segments_far_behind_the_playhead_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), (0..=200).map(|i| i * 4_000).collect());
        for n in [0, 10, 59, 60, 100, 150] {
            std::fs::write(job.segment_path(n), b"ts").unwrap();
            job.mark_ready(n);
        }
        job.last_requested.store(KEEP_BEHIND, Relaxed);
        job.prune();
        assert_eq!(job.ready_count.load(Relaxed), 6, "nothing is that far behind yet");

        job.last_requested.store(150, Relaxed);
        job.prune();
        for n in [0, 10, 59] {
            assert!(!job.is_ready(n) && !job.segment_path(n).exists(), "segment {n}");
        }
        for n in [60, 100, 150] {
            assert!(job.is_ready(n) && job.segment_path(n).exists(), "segment {n}");
        }
        assert_eq!(job.ready_count.load(Relaxed), 3);
    }
}
