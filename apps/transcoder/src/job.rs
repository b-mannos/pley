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
use crate::background::{Background, Starting};
use crate::error::{ApiError, ApiResult};
use crate::extract::Extractor;
use crate::ffmpeg::Ffmpeg;
use crate::hls;
use crate::keyframes::KeyframeStore;
use crate::metrics::Metrics;
use crate::plan::{ArgsContext, Burn, MAX_DURATION_MS, PROCESS_WINDOW, Plan, READRATE, READRATE_BURST_MS};
use crate::probe::Prober;
use crate::tsedit::{self, Edits};
use crate::util::now_ms;

/// Wait for the running process instead of restarting when the requested
/// segment is at most this far ahead of it (3 segments of 4 s, 6 of 2 s).
const LOOKAHEAD: Duration = Duration::from_secs(12);
const SEGMENT_WAIT: Duration = Duration::from_secs(45);
/// Stop ffmpeg when nobody requested anything for this long (paused player).
const PROCESS_IDLE_MS: i64 = 60_000;
/// Forget jobs nobody touched for this long.
const JOB_EXPIRE_MS: i64 = 20 * 60_000;
/// Segments kept behind the most recently requested one: this long of them
/// (90 segments of 4 s, 180 of 2 s).
const KEEP_BEHIND: Duration = Duration::from_secs(6 * 60);
/// Give up on a segment after this many failed process starts at it.
const MAX_FAILURES: u32 = 2;
/// A process younger than this is not killed for a far-away request while
/// another request is waiting for it (prevents two requests from restarting
/// each other forever).
const PREEMPT_GRACE: Duration = Duration::from_secs(3);
/// What stopping a process costs, until one was stopped (FFmpeg takes
/// 0.1-0.3 s to exit, tone mapping on Vulkan the longest).
const STOP_COST: Duration = Duration::from_millis(300);
/// Segments deleted per lock acquisition when pruning.
const PRUNE_BATCH: usize = 32;
/// Jobs whose hardware encoder failed before the capability cache is
/// deleted, so that the next start tests the encoders again: a driver
/// update or a GPU gone leaves the cache saying they work.
const HARDWARE_FAILURES: u32 = 3;
/// A process starts (and background work waits, see [`crate::background`])
/// until it has made this many segments: what players buffer before they
/// play.
const STARTED_AFTER: u32 = 3;

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

    /// A process slot, with a hardware session when `want_hw` and one is
    /// free; `None` when every slot is taken.
    pub(crate) fn take(self: &Arc<Self>, want_hw: bool) -> Option<Slot> {
        self.acquire(want_hw).map(|hw| Slot { slots: self.clone(), hw })
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

/// A transcode slot, given back however the process (or conversion)
/// holding it ends.
pub(crate) struct Slot {
    slots: Arc<Slots>,
    /// Whether it holds a hardware encoder session.
    pub hw: bool,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.slots.release(self.hw);
    }
}

pub struct JobManager {
    ff: Arc<Ffmpeg>,
    prober: Arc<Prober>,
    keyframes: Arc<KeyframeStore>,
    extractor: Arc<Extractor>,
    metrics: Arc<Metrics>,
    jobs_dir: PathBuf,
    /// libplacebo's shader cache (see [`crate::plan::shader_cache_option`]).
    shader_cache: Option<PathBuf>,
    /// The capability cache, deleted after [`HARDWARE_FAILURES`] jobs whose
    /// hardware encoder failed.
    capabilities: Option<PathBuf>,
    hardware_failures: AtomicU32,
    pub limits: Limits,
    pub slots: Arc<Slots>,
    /// Background work, held while jobs start.
    pub background: Arc<Background>,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    /// Set on shutdown: no new jobs are accepted.
    closed: AtomicBool,
    /// Job creations begun so far, and when the last began: reading ahead
    /// (`prefetch.rs`) keeps out of a starting playback's way.
    creations: AtomicU64,
    last_creation: Mutex<Option<Instant>>,
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
    /// Counts the processes started: the latest one's is the only exit that
    /// says how the job stands (one stopped long before may go after
    /// another started, see [`Proc::stop`]).
    generation: u64,
    process: Option<ProcessStatus>,
    last_error: Option<String>,
    restarts: u32,
    started_any: bool,
    warnings: Vec<String>,
    encoder_override: Option<EncoderInfo>,
    /// The hardware encoder failed before its first segment: the software
    /// one takes over (see [`JobManager::spawn`]).
    hardware_failed: bool,
    failures: HashMap<u32, u32>,
    /// The automatic preset of the running process (index in
    /// [`adaptive::LADDER`]), and when it started or moved.
    preset: Option<(usize, Instant)>,
    /// What a restart costs this job: from starting FFmpeg to its first
    /// segment (the latest process's), and stopping the one before.
    start_cost: Option<Duration>,
    stop_cost: Option<Duration>,
}

struct Proc {
    /// The segment it started at.
    start: u32,
    end: u32,
    started: Instant,
    next: Arc<AtomicU32>,
    /// [`LOOKAHEAD`] in the job's segments.
    lookahead: u32,
    pace: Arc<Mutex<Pace>>,
    exited: watch::Receiver<bool>,
    kill: Option<oneshot::Sender<()>>,
}

/// How fast a process has been going over the last few seconds. FFmpeg's
/// own `speed` is the average since the process started, which the burst
/// before the read rate applies keeps high long after it ended.
#[derive(Default)]
struct Pace {
    /// When the process was where (ms of media), oldest first.
    samples: VecDeque<(Instant, i64)>,
}

impl Pace {
    /// How long the speed is measured over.
    const WINDOW: Duration = Duration::from_secs(4);
    /// The least of them that gives a speed.
    const MIN_SPAN: Duration = Duration::from_secs(2);

    fn record(&mut self, at: Instant, out_ms: i64) {
        self.samples.push_back((at, out_ms));
        while self.samples.get(1).is_some_and(|(t, _)| at.saturating_duration_since(*t) >= Self::WINDOW) {
            self.samples.pop_front();
        }
    }

    /// Media seconds made per second lately.
    fn speed(&self) -> Option<f64> {
        let (&(t0, m0), &(t1, m1)) = (self.samples.front()?, self.samples.back()?);
        let span = t1.saturating_duration_since(t0);
        (span >= Self::MIN_SPAN).then(|| (m1 - m0) as f64 / 1000.0 / span.as_secs_f64())
    }
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
        n >= next && n < self.end && n <= next + self.lookahead
    }

    async fn stop(mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
        let _ = tokio::time::timeout(Duration::from_secs(10), self.exited.wait_for(|exited| *exited)).await;
    }
}

impl JobManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ff: Arc<Ffmpeg>,
        prober: Arc<Prober>,
        keyframes: Arc<KeyframeStore>,
        extractor: Arc<Extractor>,
        metrics: Arc<Metrics>,
        jobs_dir: PathBuf,
        shader_cache: Option<PathBuf>,
        capabilities: Option<PathBuf>,
        limits: Limits,
        background: Arc<Background>,
    ) -> Arc<Self> {
        Arc::new(Self {
            slots: Arc::new(Slots::new(&limits)),
            background,
            ff,
            prober,
            keyframes,
            extractor,
            metrics,
            jobs_dir,
            shader_cache,
            capabilities,
            hardware_failures: AtomicU32::new(0),
            limits,
            jobs: Mutex::default(),
            closed: AtomicBool::new(false),
            creations: AtomicU64::new(0),
            last_creation: Mutex::new(None),
        })
    }

    /// How many job creations have begun so far.
    pub fn creations(&self) -> u64 {
        self.creations.load(Relaxed)
    }

    /// A job creation begins.
    pub(crate) fn note_creation(&self) {
        *self.last_creation.lock() = Some(Instant::now());
        self.creations.fetch_add(1, Relaxed);
    }

    /// Whether a job creation began within `window`.
    pub fn created_within(&self, window: Duration) -> bool {
        self.last_creation.lock().is_some_and(|at| at.elapsed() < window)
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
        self.note_creation();
        // Until its process takes over (see `spawn`).
        let _starting = self.background.start();
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
                match job.throttled_wait(p, n) {
                    None => return Ok(()),
                    Some(wait) => tracing::debug!(
                        job = %job.id,
                        "segment {n}: restarting rather than waiting {:.1} s for the throttled process",
                        wait.as_secs_f64()
                    ),
                }
            } else if p.started.elapsed() < PREEMPT_GRACE && job.is_awaited(|m| m != n && p.will_produce(m)) {
                // Another request is waiting for this young process: let it be
                // served first. This request retries after the next segment or second.
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
            let running = old.running();
            let stopping = Instant::now();
            old.stop().await;
            if running {
                job.runtime.lock().stop_cost = Some(stopping.elapsed());
            }
        }
        *guard = Some(self.spawn(job, n, None)?);
        Ok(())
    }

    /// A job's hardware encoder failed: after [`HARDWARE_FAILURES`], the
    /// next start tests the encoders again.
    fn hardware_failed(&self) {
        let failures = self.hardware_failures.fetch_add(1, Relaxed) + 1;
        let Some(cache) = self.capabilities.as_ref().filter(|_| failures == HARDWARE_FAILURES) else { return };
        match std::fs::remove_file(cache) {
            Ok(()) => tracing::warn!(
                "the hardware encoder failed {failures} jobs: the encoders are tested again at the next start"
            ),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!("could not delete {}: {err}", cache.display()),
        }
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
        let planned = plan.encoder();
        let stand_in = plan.software_stand_in(&self.ff);
        // Once the hardware encoder failed the job, the software one takes over.
        let retreat = stand_in.filter(|_| job.runtime.lock().hardware_failed);
        let want_hw = planned.is_some_and(|e| e.hardware) && retreat.is_none();
        let slot = self
            .slots
            .take(want_hw)
            .ok_or_else(|| ApiError::Busy(format!("all {} transcode slots are in use", self.limits.max_jobs)))?;
        let hw = slot.hw;
        let fallback = if let Some(software) = retreat {
            if job.warn("the hardware encoder failed, using software encoding") {
                let failed = planned.map_or("the hardware encoder", |e| e.name.as_str());
                tracing::warn!(
                    job = %job.id,
                    "{failed} failed before its first segment: encoding with {} instead",
                    software.name
                );
                self.hardware_failed();
            }
            Some(software)
        } else if want_hw && !hw {
            let Some(software) = stand_in else {
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
            shader_cache: self.shader_cache.as_deref(),
        };
        let args = plan.ffmpeg_args(&ctx, start, end);
        tracing::debug!(job = %job.id, "ffmpeg {}", args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
        let mut cmd = self.ff.command();
        cmd.args(&args).current_dir(&job.dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let starting = self.background.start();
        let spawned = Instant::now();
        let mut child =
            cmd.spawn().map_err(|err| ApiError::Internal(anyhow::anyhow!("failed to start ffmpeg: {err}")))?;
        Metrics::inc(&self.metrics.processes_started);
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(ApiError::Internal(anyhow::anyhow!("ffmpeg pipes unavailable")));
        };

        let start_ms = plan.boundaries[start as usize];
        let generation = {
            let mut rt = job.runtime.lock();
            rt.generation += 1;
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
            rt.generation
        };

        let next = Arc::new(AtomicU32::new(start));
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let pace = Arc::new(Mutex::new(Pace::default()));
        let lister =
            tokio::spawn(read_segment_list(job.clone(), stdout, next.clone(), spawned, starting, self.metrics.clone()));
        tokio::spawn(read_progress(job.clone(), stderr, start_ms, tail.clone(), pace.clone()));

        let (exit_tx, exit_rx) = watch::channel(false);
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
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
            // Every segment it listed is ready (edited, when it needs it)
            // before the process counts as gone: a request coming then
            // must not start another one that writes them again.
            let _ = lister.await;
            drop(slot);
            job_ref.on_exit(generation, end, next_ref.load(Relaxed), status, killed, &tail, &metrics);
            let _ = exit_tx.send(true);
            job_ref.notify.notify_waiters();
        });

        Ok(Proc {
            start,
            end,
            started: Instant::now(),
            next,
            lookahead: job.segments_in(LOOKAHEAD),
            pace,
            exited: exit_rx,
            kill: Some(kill_tx),
        })
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
            if idle > PROCESS_IDLE_MS
                && let Ok(mut guard) = job.process.try_lock()
                && guard.as_ref().is_some_and(Proc::running)
            {
                tracing::debug!(job = %job.id, "stopping idle ffmpeg process");
                if let Some(p) = guard.take() {
                    p.stop().await;
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
        drop(waiting);
        // A request held back for this one (see `ensure_producing`), now
        // gone (served, or aborted by its player), looks again right away
        // rather than at its next timer tick.
        self.job.notify.notify_waiters();
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

    /// The wait for segment `n` from process `p`, which will produce it, when
    /// restarting at `n` is quicker, so the request doesn't wait. That is when
    /// the process is held back by the read rate: past its burst, going at
    /// that rate (the encoder isn't what limits it), so slower than a new
    /// process with a burst of its own. And when the wait is longer than a
    /// restart costs this job, and no other request waits for a segment the
    /// process is about to make (players that fetch several at once).
    fn throttled_wait(&self, p: &Proc, n: u32) -> Option<Duration> {
        let bounds = &self.plan.boundaries;
        let next = p.next.load(Relaxed) as usize;
        let n = n as usize;
        if !self.plan.throttle || n < next || n + 1 >= bounds.len() || next + 1 >= bounds.len() {
            return None;
        }
        if bounds[next] - bounds[p.start as usize] < READRATE_BURST_MS {
            return None;
        }
        let speed = p.pace.lock().speed()?;
        if (speed - READRATE).abs() > READRATE * 0.1 {
            return None;
        }
        // Up to the end of `n`, from halfway through the one being made.
        let media_ms = (bounds[n + 1] - bounds[next]) - (bounds[next + 1] - bounds[next]) / 2;
        let wait = Duration::from_secs_f64(media_ms.max(0) as f64 / 1000.0 / speed);
        let restart = {
            let rt = self.runtime.lock();
            rt.start_cost? + rt.stop_cost.unwrap_or(STOP_COST)
        };
        let others = self.is_awaited(|m| m as usize != n && p.will_produce(m));
        (wait > restart && !others).then_some(wait)
    }

    /// How many of this job's segments last `span` (at least one).
    fn segments_in(&self, span: Duration) -> u32 {
        (span.as_secs_f64() / self.plan.segment_seconds).round().max(1.0) as u32
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

    /// Marks segment `n` ready, without waking anyone: see [`segment_written`].
    fn mark_ready(&self, n: u32) {
        let mut ready = self.ready.lock();
        if let Some(slot) = ready.get_mut(n as usize)
            && !*slot
        {
            *slot = true;
            self.ready_count.fetch_add(1, Relaxed);
        }
    }

    /// Adds `message` to the job's warnings: whether it is a new one.
    fn warn(&self, message: &str) -> bool {
        let mut rt = self.runtime.lock();
        let new = !rt.warnings.iter().any(|w| w == message);
        if new {
            rt.warnings.push(message.to_owned());
        }
        new
    }

    pub fn record_served(&self, bytes: u64) {
        self.served_segments.fetch_add(1, Relaxed);
        self.served_bytes.fetch_add(bytes, Relaxed);
    }

    /// Process `generation` exited, at segment `next` of those it was to
    /// make up to `end`.
    #[allow(clippy::too_many_arguments)]
    fn on_exit(
        &self,
        generation: u64,
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
        if rt.generation != generation {
            // Stopped long ago, gone at last: the job is the running one's.
            return;
        }
        let process = rt.process.take();
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
            // A hardware encoder failing before it made anything (a driver
            // update, a GPU gone, a session refused) won't do better at the
            // next try.
            if process.is_some_and(|p| p.hardware && next == p.start_segment) {
                rt.hardware_failed = true;
            }
        }
    }

    /// Delete segments far behind the playhead. Blocking: run it on the
    /// blocking pool. The lock is held per batch so segment lookups never wait
    /// long, and a segment is only marked missing once its file is gone.
    fn prune(&self) {
        let last = self.last_requested.load(Relaxed);
        let keep = self.segments_in(KEEP_BEHIND);
        if last <= keep {
            return;
        }
        let cutoff = (last - keep) as usize;
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

    /// The multivariant playlist, naming the codecs (see [`crate::hls`]).
    pub fn master_playlist(&self) -> String {
        hls::master_playlist(&self.plan, &self.media)
    }

    /// Does to segment `n`'s video what FFmpeg couldn't (see
    /// [`crate::tsedit`]), before anyone is told it is ready.
    async fn edit_segment(&self, n: u32, edits: Edits) {
        let path = self.segment_path(n);
        let edited = tokio::task::spawn_blocking(move || tsedit::edit_segment(&path, edits)).await;
        match edited {
            Ok(Ok(edited)) => {
                if let Some(err) = edited.first_error.as_deref().filter(|_| edited.rpus_dropped > 0) {
                    tracing::debug!(job = %self.id, "segment {n}: {} RPUs left out: {err}", edited.rpus_dropped);
                    self.warn(&format!(
                        "some Dolby Vision metadata could not be made profile 8.1 and was left out ({err})"
                    ));
                }
            }
            Ok(Err(err)) => {
                tracing::warn!(job = %self.id, "segment {n} could not be edited: {err}");
                self.warn(&format!("a segment's video could not be edited: {err}"));
            }
            Err(err) => tracing::warn!(job = %self.id, "segment {n} could not be edited: {err}"),
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

async fn read_segment_list(
    job: Arc<Job>,
    stdout: ChildStdout,
    next: Arc<AtomicU32>,
    spawned: Instant,
    starting: Starting,
    metrics: Arc<Metrics>,
) {
    let mut lines = BufReader::new(stdout).lines();
    let mut first = true;
    let mut starting = Some(starting);
    let mut made = 0;
    let edits = job.plan.segment_edits();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(n) = parse_list_line(&line) {
            if let Some(edits) = edits {
                job.edit_segment(n, edits).await;
            }
            tracing::trace!(job = %job.id, "segment {n} ready");
            if std::mem::take(&mut first) {
                job.runtime.lock().start_cost = Some(spawned.elapsed());
            }
            segment_written(&job, &next, n);
            Metrics::inc(&metrics.segments_produced);
            made += 1;
            if made == STARTED_AFTER {
                // Started: background work goes on.
                drop(starting.take());
            }
        }
    }
}

/// FFmpeg finished segment `n`: it is ready, and the process is past it,
/// before anyone wakes. A woken request then never sees the process still
/// owing the segment just finished (and wait on it for nothing).
fn segment_written(job: &Job, next: &AtomicU32, n: u32) {
    job.mark_ready(n);
    next.store(n + 1, Relaxed);
    job.notify.notify_waiters();
}

async fn read_progress(
    job: Arc<Job>,
    stderr: ChildStderr,
    start_ms: i64,
    tail: Arc<Mutex<VecDeque<String>>>,
    pace: Arc<Mutex<Pace>>,
) {
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
                    let out_us = block.get("out_time_us").and_then(|v| v.trim().parse::<f64>().ok());
                    if let Some(us) = out_us.filter(|us| *us >= 0.0) {
                        pace.lock().record(Instant::now(), start_ms + (us / 1000.0) as i64);
                    }
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
            "video {} {}→{} {}x{}→{}x{}{}{}{}{}",
            if v.mode == VideoMode::Copy { "copy" } else { "transcode" },
            v.codec_in,
            v.codec_out,
            v.width_in.unwrap_or(0),
            v.height_in.unwrap_or(0),
            v.width_out.unwrap_or(0),
            v.height_out.unwrap_or(0),
            v.encoder.as_deref().map(|e| format!(" via {e}")).unwrap_or_default(),
            match (v.dynamic_range_out.filter(|r| r.is_hdr()), v.tonemap) {
                (Some(out), Some(t)) => format!(" {} to {}", t.as_str(), out.label()),
                (Some(out), None) => format!(" keeping {}", out.label()),
                (None, t) => t.map(|t| format!(" tonemap {}", t.as_str())).unwrap_or_default(),
            },
            v.dolby_vision.map(|m| format!(" dolby vision {m:?}").to_lowercase()).unwrap_or_default(),
            if v.hdr10plus_stripped { " without hdr10+" } else { "" },
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
        let p = Proc {
            start: 0,
            end: 20,
            started: Instant::now(),
            next: Arc::new(AtomicU32::new(10)),
            lookahead: 3,
            pace: Arc::default(),
            exited,
            kill: None,
        };
        assert!(!p.will_produce(9), "already passed (pruned or skipped): must restart");
        assert!(p.will_produce(10));
        assert!(p.will_produce(13));
        assert!(!p.will_produce(14), "too far ahead: restart there");
        p.next.store(18, Relaxed);
        assert!(p.will_produce(19));
        assert!(!p.will_produce(20), "outside the process window");
    }

    #[test]
    fn pace_is_measured_over_the_last_seconds() {
        let t = Instant::now();
        let at = |ms: u64| t + Duration::from_millis(ms);
        let mut pace = Pace::default();
        pace.record(at(0), 0);
        pace.record(at(1_000), 30_000);
        assert_eq!(pace.speed(), None, "too short to tell");
        // After a burst of 30 s in a second, twice real time.
        for s in 2..=10 {
            pace.record(at(s * 1_000), 30_000 + (s as i64 - 1) * 2_000);
        }
        let speed = pace.speed().unwrap();
        assert!((speed - 2.0).abs() < 0.001, "the burst is forgotten: {speed}");
    }

    #[test]
    fn seeks_just_past_a_throttled_process_restart_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut job = job(dir.path(), (0..=100).map(|i| i * 2_000).collect());
        job.plan.segment_seconds = 2.0;
        job.plan.throttle = true;
        let (_exit_tx, exited) = watch::channel(false);
        let p = Proc {
            start: 0,
            end: 100,
            started: Instant::now(),
            next: Arc::new(AtomicU32::new(40)),
            lookahead: 6,
            pace: Arc::default(),
            exited,
            kill: None,
        };
        let going = |speed: f64| {
            let at = Instant::now();
            let mut pace = p.pace.lock();
            *pace = Pace::default();
            pace.record(at, 80_000);
            pace.record(at + Duration::from_secs(3), 80_000 + (3_000.0 * speed) as i64);
        };
        going(2.0);
        assert_eq!(job.throttled_wait(&p, 42), None, "no restart measured yet");
        job.runtime.lock().start_cost = Some(Duration::from_millis(400));
        job.runtime.lock().stop_cost = Some(Duration::from_millis(100));

        // Two segments past the one being made: 2.5 segments of 2 s at twice
        // real time, against 0.5 s to restart.
        let wait = job.throttled_wait(&p, 42).unwrap();
        assert!((wait.as_secs_f64() - 2.5).abs() < 0.001, "{wait:?}");
        assert_eq!(job.throttled_wait(&p, 40), None, "half a segment: no quicker to restart");
        // (a) Still in its burst (40 s made of 60).
        p.next.store(20, Relaxed);
        assert_eq!(job.throttled_wait(&p, 22), None);
        p.next.store(40, Relaxed);
        // (b) Slower than the read rate: the encoder is what limits it.
        going(1.5);
        assert_eq!(job.throttled_wait(&p, 42), None);
        going(2.1);
        assert!(job.throttled_wait(&p, 42).is_some());
        // (c) A restart costing more than the wait.
        job.runtime.lock().start_cost = Some(Duration::from_secs(3));
        assert_eq!(job.throttled_wait(&p, 42), None);
        job.runtime.lock().start_cost = Some(Duration::from_millis(400));
        // (d) Another request waits for a segment the process is about to make.
        let other = Waiting::register(&job, 41);
        assert_eq!(job.throttled_wait(&p, 42), None);
        drop(other);
        let this = Waiting::register(&job, 42);
        assert!(job.throttled_wait(&p, 42).is_some(), "the request itself doesn't count");
        drop(this);
        // Not throttled at all.
        job.plan.throttle = false;
        assert_eq!(job.throttled_wait(&p, 42), None);
    }

    #[test]
    fn slots_go_back_however_their_holder_ends() {
        let slots = Arc::new(Slots::new(&Limits { max_jobs: 2, max_hw_jobs: 1 }));
        let (hardware, software) = (slots.take(true).unwrap(), slots.take(true).unwrap());
        assert_eq!((hardware.hw, software.hw), (true, false));
        assert!(slots.take(false).is_none());
        drop(hardware);
        assert_eq!(slots.counts(), (1, 0));
        drop(software);
        assert_eq!(slots.counts(), (0, 0));
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
                seek_at: boundaries.clone(),
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
        let ff = Ffmpeg {
            ffmpeg: dir.join("no-ffmpeg"),
            ffprobe: dir.join("no-ffprobe"),
            caps: pley_transcoder_api::Capabilities { ffmpeg_available: true, ..Default::default() },
            segment_times_relative: true,
        };
        manager_with(dir, ff)
    }

    fn manager_with(dir: &Path, ff: Ffmpeg) -> Arc<JobManager> {
        let ff = Arc::new(ff);
        let metrics = Arc::new(Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics.clone(), 1));
        let background = Background::new();
        let keyframes = KeyframeStore::new(ff.clone(), metrics.clone(), background.clone(), dir.join("keyframes"));
        let extractor = Arc::new(Extractor::new(ff.clone(), prober.clone(), metrics.clone(), dir.join("subtitles"), 1));
        let limits = Limits { max_jobs: 2, max_hw_jobs: 1 };
        let capabilities = Some(dir.join("capabilities.json"));
        JobManager::new(
            ff,
            prober,
            keyframes,
            extractor,
            metrics,
            dir.join("jobs"),
            None,
            capabilities,
            limits,
            background,
        )
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

    /// Background work waits while a job is created and until its FFmpeg has
    /// made its first segments. Needs FFmpeg on the PATH (skipped otherwise).
    #[tokio::test(flavor = "multi_thread")]
    async fn starting_jobs_hold_background_work() {
        use pley_transcoder_api::{VideoMode, VideoRequest};
        let ok =
            |tool: &str| std::process::Command::new(tool).arg("-version").output().is_ok_and(|o| o.status.success());
        if !(ok("ffmpeg") && ok("ffprobe")) {
            eprintln!("ffmpeg not found: skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("clip.mkv");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc2=size=320x180:rate=25:duration=30")
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "25"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(made.success());
        let ff = Ffmpeg {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            caps: pley_transcoder_api::Capabilities {
                ffmpeg_available: true,
                ffprobe_available: true,
                ..Default::default()
            },
            segment_times_relative: true,
        };
        let jobs = manager_with(dir.path(), ff);
        let background = jobs.background.clone();
        let seen = Arc::new(AtomicBool::new(false));
        let watch = tokio::spawn({
            let (background, seen) = (background.clone(), seen.clone());
            async move {
                loop {
                    if background.held() {
                        seen.store(true, Relaxed);
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        });
        let created = jobs
            .create(JobRequest {
                input: input.to_string_lossy().into_owned(),
                video: Some(VideoRequest { mode: VideoMode::Copy, ..Default::default() }),
                segment_seconds: Some(2.0),
                ..Default::default()
            })
            .await
            .unwrap();
        let job = jobs.get(&created.id).unwrap();
        jobs.segment(&job, STARTED_AFTER - 1).await.unwrap();
        watch.abort();
        assert!(seen.load(Relaxed), "held while the job started");
        let deadline = Instant::now() + Duration::from_secs(5);
        while background.held() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!background.held(), "free once its first segments are out");
        jobs.remove(&created.id).await;
    }

    /// A hardware encoder that fails before its first segment hands the job
    /// to the software one. Needs FFmpeg on the PATH (skipped otherwise).
    #[tokio::test(flavor = "multi_thread")]
    async fn a_hardware_encoder_that_fails_hands_the_job_to_software() {
        use pley_transcoder_api::{EncoderInfo, HwAccel, VideoRequest};
        let ok =
            |tool: &str| std::process::Command::new(tool).arg("-version").output().is_ok_and(|o| o.status.success());
        if !(ok("ffmpeg") && ok("ffprobe")) {
            eprintln!("ffmpeg not found: skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("clip.mkv");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc2=size=320x180:rate=25:duration=10")
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "25"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(made.success());
        let encoder = |name: &str, hardware: bool, accel: HwAccel| EncoderInfo {
            name: name.into(),
            codec: VideoCodec::H264,
            hardware,
            accel,
            main10: false,
        };
        let ff = Ffmpeg {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            caps: pley_transcoder_api::Capabilities {
                ffmpeg_available: true,
                ffprobe_available: true,
                // One FFmpeg doesn't know: it fails at once, as a broken
                // driver's would.
                encoders: vec![
                    encoder("h264_pley_broken", true, HwAccel::Amf),
                    encoder("libx264", false, HwAccel::None),
                ],
                ..Default::default()
            },
            segment_times_relative: true,
        };
        let jobs = manager_with(dir.path(), ff);
        let created = jobs
            .create(JobRequest {
                input: input.to_string_lossy().into_owned(),
                video: Some(VideoRequest::default()),
                segment_seconds: Some(2.0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(created.video.as_ref().and_then(|v| v.encoder.as_deref()), Some("h264_pley_broken"));
        let job = jobs.get(&created.id).unwrap();
        jobs.segment(&job, 0).await.expect("made in software");
        let status = job.status();
        assert_eq!(status.video.as_ref().and_then(|v| v.encoder.as_deref()), Some("libx264"));
        assert!(status.warnings.iter().any(|w| w.contains("hardware encoder failed")), "{:?}", status.warnings);
        assert_eq!(status.process.map(|p| p.hardware), Some(false));
        assert_eq!(jobs.hardware_failures.load(Relaxed), 1);
        jobs.remove(&created.id).await;
    }

    #[tokio::test]
    async fn hardware_failing_jobs_have_the_encoders_tested_again() {
        let dir = tempfile::tempdir().unwrap();
        let jobs = manager(dir.path());
        let cache = dir.path().join("capabilities.json");
        std::fs::write(&cache, "{}").unwrap();
        for _ in 1..HARDWARE_FAILURES {
            jobs.hardware_failed();
        }
        assert!(cache.exists(), "a job or two: the hardware may be busy");
        jobs.hardware_failed();
        assert!(!cache.exists(), "tested again at the next start");
    }

    #[test]
    fn only_a_hardware_process_failing_before_its_first_segment_hands_over() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000, 12_000]);
        let (metrics, tail) = (Metrics::default(), Mutex::new(VecDeque::new()));
        let exits = |hardware: bool, start: u32, next: u32, killed: bool| {
            job.runtime.lock().process = Some(ProcessStatus { hardware, start_segment: start, ..Default::default() });
            job.on_exit(0, 3, next, Ok(exit(1)), killed, &tail, &metrics);
            job.runtime.lock().hardware_failed
        };
        assert!(!exits(true, 1, 1, true), "stopped on purpose");
        assert!(!exits(true, 1, 2, false), "it made segment 1: the encoder works");
        assert!(!exits(false, 1, 1, false), "software");
        assert!(exits(true, 1, 1, false));
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

    #[tokio::test]
    async fn a_request_going_away_wakes_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000]);
        let first = Waiting::register(&job, 0);
        // Another request, held back while the first waits.
        let notified = job.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        drop(first);
        let woken = tokio::time::timeout(Duration::from_millis(100), notified).await;
        assert!(woken.is_ok(), "it looks again now, not at its next tick");
    }

    #[test]
    fn woken_requests_see_the_process_past_the_finished_segment() {
        use std::task::{Context, Wake, Waker};
        /// Where the process was when the request was woken.
        struct Seen {
            next: Arc<AtomicU32>,
            at_wake: Mutex<Option<u32>>,
        }
        impl Wake for Seen {
            fn wake(self: Arc<Self>) {
                *self.at_wake.lock() = Some(self.next.load(Relaxed));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000]);
        let next = Arc::new(AtomicU32::new(0));
        let seen = Arc::new(Seen { next: next.clone(), at_wake: Mutex::new(None) });
        let waker = Waker::from(seen.clone());
        let mut notified = std::pin::pin!(job.notify.notified());
        assert!(notified.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
        segment_written(&job, &next, 0);
        assert_eq!(*seen.at_wake.lock(), Some(1), "the process had moved on when the request woke");
        assert!(job.is_ready(0));
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
        job.on_exit(0, 2, 2, Ok(exit(0)), false, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Finished, "produced up to the end");
        job.on_exit(0, 1, 1, Ok(exit(0)), false, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Idle, "the window ended before the file did");
        job.on_exit(0, 2, 1, Ok(exit(1)), true, &tail, &metrics);
        assert_eq!(job.status().state, JobState::Idle, "stopped on purpose is no failure");
        assert_eq!(metrics.process_failures.load(Relaxed), 0);

        // A failure keeps the end of ffmpeg's output.
        tail.lock().extend(["one", "two", "three", "four", "five"].map(String::from));
        job.on_exit(0, 2, 1, Ok(exit(1)), false, &tail, &metrics);
        let status = job.status();
        assert_eq!(status.state, JobState::Failed);
        assert_eq!(status.last_error.as_deref(), Some("two | three | four | five"));
        job.on_exit(0, 2, 1, Err(std::io::Error::other("gone")), false, &Mutex::new(VecDeque::new()), &metrics);
        assert!(job.status().last_error.unwrap().contains("could not be awaited: gone"));
        assert_eq!(metrics.process_failures.load(Relaxed), 2);
        assert_eq!(metrics.jobs_failed.load(Relaxed), 1, "a failing job counts once");
        assert_eq!(job.runtime.lock().failures.get(&1), Some(&2), "failures per start segment");
    }

    #[test]
    fn a_process_stopped_long_ago_leaves_the_running_one_be() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), vec![0, 4_000, 8_000]);
        let (metrics, tail) = (Metrics::default(), Mutex::new(VecDeque::new()));
        {
            let mut rt = job.runtime.lock();
            rt.generation = 2;
            rt.state = JobState::Running;
            rt.process = Some(ProcessStatus { pid: Some(7), ..Default::default() });
        }
        // The first process, which outlived its stop by seconds...
        job.on_exit(1, 2, 1, Ok(exit(1)), true, &tail, &metrics);
        let status = job.status();
        assert_eq!((status.state, status.process.and_then(|p| p.pid)), (JobState::Running, Some(7)));
        // ...then the second, the running one.
        job.on_exit(2, 2, 1, Ok(exit(1)), true, &tail, &metrics);
        let status = job.status();
        assert_eq!((status.state, status.process), (JobState::Idle, None));
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
    fn segment_counted_limits_keep_their_length_in_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut job = job(dir.path(), vec![0, 4_000, 8_000]);
        assert_eq!((job.segments_in(LOOKAHEAD), job.segments_in(KEEP_BEHIND)), (3, 90));
        job.plan.segment_seconds = 2.0;
        assert_eq!((job.segments_in(LOOKAHEAD), job.segments_in(KEEP_BEHIND)), (6, 180));
        job.plan.segment_seconds = 10.0;
        assert_eq!((job.segments_in(LOOKAHEAD), job.segments_in(KEEP_BEHIND)), (1, 36));
        job.plan.segment_seconds = 30.0;
        assert_eq!(job.segments_in(LOOKAHEAD), 1, "at least the next one");
    }

    #[test]
    fn segments_far_behind_the_playhead_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let job = job(dir.path(), (0..=200).map(|i| i * 4_000).collect());
        for n in [0, 10, 59, 60, 100, 150] {
            std::fs::write(job.segment_path(n), b"ts").unwrap();
            job.mark_ready(n);
        }
        job.last_requested.store(90, Relaxed);
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
