//! The connections to IPTV providers: a *tuner* pulls one channel, once,
//! however many watch it, and hands what comes to each of them.
//!
//! - A source plays no more channels at once than its provider allows
//!   (`max_connections`). A channel nobody watches any more is let go after
//!   [`LINGER`], or at once when another channel needs its connection.
//! - Players watch through a *viewer*: an HLS one gets the channel from the
//!   transcoder (one live job per tuner, which every HLS viewer of it reads),
//!   a stream one the provider's MPEG-TS as it comes. Starting to watch on a
//!   device (`client_id`) ends what that device watched before, so changing
//!   channels on a one-connection account just works.
//! - Channels served as HLS by the provider are read segment by segment and
//!   handed on as one MPEG-TS stream.
//! - A connection that breaks is made again, a few times, while the channel
//!   is watched.
//! - Whether a channel played is told ([`Played`]): one the provider gives
//!   nothing for is offline until it plays again.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use parking_lot::Mutex;
use pley_transcoder_api::{HwAccel, LivePlan, LiveRequest};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use super::{ChannelRow, LiveSettings};
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::events::{EventBus, ServerEvent};
use crate::transcoder::Transcoder;
use crate::util::now;

/// A channel nobody watches is let go this long after the last viewer left
/// (unless its connection is wanted sooner): coming back to it is instant.
const LINGER: Duration = Duration::from_secs(20);
/// An HLS viewer that asked for nothing this long left (players ask for the
/// playlist every few seconds while they play; a paused one too).
const VIEWER_GONE: Duration = Duration::from_secs(45);
/// What the provider last sent, given to whoever joins first: a quick start.
const BACKLOG_BYTES: usize = 2 << 20;
/// Chunks a slow reader may fall behind before it skips some.
const CHANNEL_CHUNKS: usize = 2048;
/// The provider sending nothing this long breaks the connection.
const STALL: Duration = Duration::from_secs(15);
/// Connection attempts in a row before a channel is given up on.
const ATTEMPTS: u32 = 5;
/// Bytes after which a connection counts as having worked (its failures
/// start again from 0).
const WORKED_BYTES: u64 = 4 << 20;

/// Whether a channel played: it sent something, or the provider gave
/// nothing for it however often it was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Played {
    pub channel_id: i64,
    pub played: bool,
}

/// How a provider refusing the account (not the channel) begins.
const REFUSED: &str = "the provider refused it";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewerKind {
    /// Live HLS from the transcoder.
    Hls,
    /// The provider's MPEG-TS as it is.
    Stream,
}

#[derive(Debug, Clone)]
struct Viewer {
    id: String,
    user_id: i64,
    username: String,
    client_id: Option<String>,
    channel_id: i64,
    started_at: i64,
    last_seen: Instant,
    kind: ViewerKind,
}

/// Someone watching, as administrators see it.
#[derive(Debug, Clone, Serialize)]
pub struct ViewerStatus {
    pub session_id: String,
    pub user_id: i64,
    pub username: String,
    pub kind: ViewerKind,
    pub started_at: i64,
}

/// A connection in use, as administrators see it.
#[derive(Debug, Clone, Serialize)]
pub struct TunerStatus {
    pub channel_id: i64,
    pub channel_name: String,
    pub source_id: i64,
    pub source_name: String,
    pub started_at: i64,
    /// The provider is sending.
    pub connected: bool,
    pub error: Option<String>,
    pub bytes_in: u64,
    /// What the provider sends, kbit/s (over the last seconds).
    pub kbps: u64,
    pub viewers: Vec<ViewerStatus>,
    /// Nobody watches: it goes soon.
    pub idle: bool,
    /// What the transcoder makes of it, for HLS viewers.
    pub plan: Option<LivePlan>,
}

/// What a player watches with.
#[derive(Debug, Clone, Serialize)]
pub struct Watching {
    pub session_id: String,
    /// The live HLS playlist (HLS viewers).
    pub url: String,
    pub kind: ViewerKind,
}

struct Feed {
    backlog: VecDeque<Bytes>,
    backlog_bytes: usize,
    tx: Option<broadcast::Sender<Bytes>>,
}

struct TunerState {
    connected: bool,
    error: Option<String>,
    bytes_in: u64,
    /// Bytes counted at the start of the current rate window.
    rate: (Instant, u64, u64),
    idle_since: Option<Instant>,
    /// The provider was given up on.
    ended: bool,
    /// The transcoder's job failed (and why): not made again for this tuner.
    hls_failed: Option<String>,
    plan: Option<LivePlan>,
}

impl Default for TunerState {
    fn default() -> Self {
        Self {
            connected: false,
            error: None,
            bytes_in: 0,
            rate: (Instant::now(), 0, 0),
            idle_since: None,
            ended: false,
            hls_failed: None,
            plan: None,
        }
    }
}

struct Tuner {
    channel_id: i64,
    channel_name: String,
    source_id: i64,
    source_name: String,
    url: String,
    user_agent: String,
    started_at: i64,
    feed: Mutex<Feed>,
    state: Mutex<TunerState>,
    stop: CancellationToken,
    /// The transcoder's live job, once an HLS viewer came.
    hls: tokio::sync::Mutex<Option<String>>,
    played: mpsc::UnboundedSender<Played>,
}

impl Tuner {
    fn is_live(&self) -> bool {
        !self.stop.is_cancelled() && !self.state.lock().ended
    }

    fn push(&self, chunk: Bytes) {
        {
            let mut state = self.state.lock();
            if state.bytes_in == 0 {
                let _ = self.played.send(Played { channel_id: self.channel_id, played: true });
            }
            state.bytes_in += chunk.len() as u64;
            state.connected = true;
            state.error = None;
        }
        let mut feed = self.feed.lock();
        feed.backlog_bytes += chunk.len();
        feed.backlog.push_back(chunk.clone());
        while feed.backlog_bytes > BACKLOG_BYTES {
            let Some(old) = feed.backlog.pop_front() else { break };
            feed.backlog_bytes -= old.len();
        }
        if let Some(tx) = &feed.tx {
            let _ = tx.send(chunk);
        }
    }

    /// What came lately and what comes from now on, as one stream; `None`
    /// once the tuner stopped.
    fn subscribe(&self) -> Option<impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static + use<>> {
        let feed = self.feed.lock();
        let rx = feed.tx.as_ref()?.subscribe();
        let backlog: Vec<Bytes> = feed.backlog.iter().cloned().collect();
        drop(feed);
        let live = futures::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(chunk) => return Some((Ok(chunk), rx)),
                    // A reader that fell behind skips what it missed: the
                    // MPEG-TS demuxer finds its way again.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        });
        Some(futures::stream::iter(backlog.into_iter().map(Ok)).chain(live))
    }

    fn close(&self) {
        self.stop.cancel();
        self.feed.lock().tx = None;
    }

    fn kbps(&self) -> u64 {
        let mut state = self.state.lock();
        let (since, at_start, last) = state.rate;
        let elapsed = since.elapsed().as_secs_f64();
        if elapsed >= 10.0 {
            let rate = ((state.bytes_in - at_start) as f64 * 8.0 / 1000.0 / elapsed) as u64;
            state.rate = (Instant::now(), state.bytes_in, rate);
            rate
        } else if last == 0 && elapsed > 1.0 {
            ((state.bytes_in - at_start) as f64 * 8.0 / 1000.0 / elapsed) as u64
        } else {
            last
        }
    }
}

pub struct Tuners {
    transcoder: Arc<Transcoder>,
    events: EventBus,
    http: reqwest::Client,
    tuners: Mutex<HashMap<i64, Arc<Tuner>>>,
    viewers: Mutex<HashMap<String, Viewer>>,
    /// Decisions to start a tuner, one at a time (they count connections).
    starting: tokio::sync::Mutex<()>,
    played: mpsc::UnboundedSender<Played>,
}

/// What a tuner needs to know of its channel.
pub struct Tune<'a> {
    pub channel: &'a ChannelRow,
    pub source_name: String,
    pub max_connections: i64,
    pub user_agent: String,
}

impl Tuners {
    pub fn new(
        transcoder: Arc<Transcoder>,
        events: EventBus,
        http: reqwest::Client,
        played: mpsc::UnboundedSender<Played>,
    ) -> Arc<Self> {
        Arc::new(Self {
            transcoder,
            events,
            http,
            tuners: Mutex::default(),
            viewers: Mutex::default(),
            starting: tokio::sync::Mutex::new(()),
            played,
        })
    }

    /// Whether any channel of `source_id` is being pulled.
    pub fn source_busy(&self, source_id: i64) -> bool {
        self.tuners.lock().values().any(|t| t.source_id == source_id && t.is_live())
    }

    /// Lets go of channels nobody watches, every few seconds.
    pub fn start(self: &Arc<Self>, stopping: CancellationToken) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    () = stopping.cancelled() => break,
                    _ = tick.tick() => {}
                }
                this.reap();
            }
            let all: Vec<Arc<Tuner>> = this.tuners.lock().drain().map(|(_, t)| t).collect();
            for tuner in all {
                this.stop_tuner(&tuner);
            }
        });
    }

    fn reap(&self) {
        let mut changed = false;
        {
            let mut viewers = self.viewers.lock();
            let before = viewers.len();
            viewers.retain(|_, v| v.kind == ViewerKind::Stream || v.last_seen.elapsed() < VIEWER_GONE);
            changed |= viewers.len() != before;
        }
        let watched: HashMap<i64, usize> = self.viewers.lock().values().fold(HashMap::new(), |mut m, v| {
            *m.entry(v.channel_id).or_insert(0) += 1;
            m
        });
        let tuners: Vec<Arc<Tuner>> = self.tuners.lock().values().cloned().collect();
        for tuner in tuners {
            let watching = watched.get(&tuner.channel_id).copied().unwrap_or(0);
            let gone = {
                let mut state = tuner.state.lock();
                if watching > 0 {
                    state.idle_since = None;
                    false
                } else {
                    let since = *state.idle_since.get_or_insert_with(Instant::now);
                    state.ended || since.elapsed() >= LINGER
                }
            };
            if gone || tuner.stop.is_cancelled() {
                self.remove_tuner(&tuner);
                changed = true;
            }
        }
        if changed {
            self.events.send(ServerEvent::LiveTunersChanged);
        }
    }

    fn remove_tuner(&self, tuner: &Arc<Tuner>) {
        {
            let mut tuners = self.tuners.lock();
            if tuners.get(&tuner.channel_id).is_some_and(|t| Arc::ptr_eq(t, tuner)) {
                tuners.remove(&tuner.channel_id);
            }
        }
        self.stop_tuner(tuner);
    }

    fn stop_tuner(&self, tuner: &Arc<Tuner>) {
        if !tuner.stop.is_cancelled() {
            tracing::info!(channel = tuner.channel_id, name = %tuner.channel_name, "live channel let go");
        }
        tuner.close();
        let (transcoder, tuner) = (self.transcoder.clone(), tuner.clone());
        tokio::spawn(async move {
            if let Some(job) = tuner.hls.lock().await.take() {
                let _ = transcoder.delete_live(&job).await;
            }
        });
    }

    /// Channels pulled now, by source.
    pub fn in_use(&self) -> HashMap<i64, usize> {
        let mut out = HashMap::new();
        for t in self.tuners.lock().values().filter(|t| t.is_live()) {
            *out.entry(t.source_id).or_insert(0) += 1;
        }
        out
    }

    pub async fn stop_source(&self, source_id: i64) {
        let tuners: Vec<Arc<Tuner>> =
            self.tuners.lock().values().filter(|t| t.source_id == source_id).cloned().collect();
        for t in &tuners {
            self.end_viewers_of(t.channel_id);
            self.remove_tuner(t);
        }
        if !tuners.is_empty() {
            self.events.send(ServerEvent::LiveTunersChanged);
        }
    }

    pub async fn stop_channel(&self, channel_id: i64) {
        let tuner = self.tuners.lock().get(&channel_id).cloned();
        if let Some(t) = tuner {
            self.end_viewers_of(channel_id);
            self.remove_tuner(&t);
            self.events.send(ServerEvent::LiveTunersChanged);
        }
    }

    fn end_viewers_of(&self, channel_id: i64) {
        self.viewers.lock().retain(|_, v| v.channel_id != channel_id);
    }

    /// Starts `user` watching `tune.channel`: through the tuner already
    /// pulling it, or a new one when its source has a connection free.
    pub async fn watch(
        self: &Arc<Self>,
        user: &AuthUser,
        tune: Tune<'_>,
        client_id: Option<String>,
        kind: ViewerKind,
    ) -> Result<Watching, AppError> {
        let channel_id = tune.channel.id;
        // What this device watched before ends: its connection is free.
        if let Some(client) = client_id.as_deref().filter(|c| !c.is_empty()) {
            self.viewers.lock().retain(|_, v| !(v.user_id == user.id && v.client_id.as_deref() == Some(client)));
        }
        let tuner = {
            let _one = self.starting.lock().await;
            // Not in the match: its guard would live through start_tuner's own lock.
            let pulling = self.tuners.lock().get(&channel_id).filter(|t| t.is_live()).cloned();
            match pulling {
                Some(t) => t,
                None => self.start_tuner(user, &tune)?,
            }
        };
        let session_id = uuid::Uuid::new_v4().simple().to_string();
        self.viewers.lock().insert(
            session_id.clone(),
            Viewer {
                id: session_id.clone(),
                user_id: user.id,
                username: user.username.clone(),
                client_id,
                channel_id,
                started_at: now(),
                last_seen: Instant::now(),
                kind,
            },
        );
        tuner.state.lock().idle_since = None;
        self.events.send(ServerEvent::LiveTunersChanged);
        Ok(Watching { url: format!("/api/live/sessions/{session_id}/index.m3u8"), session_id, kind })
    }

    fn start_tuner(self: &Arc<Self>, user: &AuthUser, tune: &Tune<'_>) -> Result<Arc<Tuner>, AppError> {
        let watched: HashMap<i64, usize> = self.viewers.lock().values().fold(HashMap::new(), |mut m, v| {
            *m.entry(v.channel_id).or_insert(0) += 1;
            m
        });
        let on_source: Vec<Arc<Tuner>> = self
            .tuners
            .lock()
            .values()
            .filter(|t| t.source_id == tune.channel.source_id && t.is_live())
            .cloned()
            .collect();
        if tune.max_connections > 0 && on_source.len() as i64 >= tune.max_connections {
            // A channel nobody watches gives its connection up.
            match on_source.iter().find(|t| !watched.contains_key(&t.channel_id)) {
                Some(idle) => self.remove_tuner(idle),
                None => return Err(self.all_in_use(user, tune, &on_source)),
            }
        }
        let tuner = Arc::new(Tuner {
            channel_id: tune.channel.id,
            channel_name: tune.channel.display_name().to_owned(),
            source_id: tune.channel.source_id,
            source_name: tune.source_name.clone(),
            url: tune.channel.stream_url.clone(),
            user_agent: tune.user_agent.clone(),
            started_at: now(),
            feed: Mutex::new(Feed {
                backlog: VecDeque::new(),
                backlog_bytes: 0,
                tx: Some(broadcast::channel(CHANNEL_CHUNKS).0),
            }),
            state: Mutex::new(TunerState::default()),
            stop: CancellationToken::new(),
            hls: tokio::sync::Mutex::new(None),
            played: self.played.clone(),
        });
        tracing::info!(channel = tuner.channel_id, name = %tuner.channel_name, source = %tuner.source_name, "live channel tuned");
        self.tuners.lock().insert(tuner.channel_id, tuner.clone());
        let (http, t) = (self.http.clone(), tuner.clone());
        tokio::spawn(async move { pull(t, http).await });
        Ok(tuner)
    }

    fn all_in_use(&self, user: &AuthUser, tune: &Tune<'_>, on_source: &[Arc<Tuner>]) -> AppError {
        let viewers = self.viewers.lock();
        let names: Vec<String> = on_source
            .iter()
            .map(|t| {
                let mut who: Vec<&str> =
                    viewers.values().filter(|v| v.channel_id == t.channel_id).map(|v| v.username.as_str()).collect();
                // One person on two devices is named once.
                who.sort_unstable();
                who.dedup();
                if user.is_admin && !who.is_empty() {
                    format!("{} ({})", t.channel_name, who.join(", "))
                } else {
                    t.channel_name.clone()
                }
            })
            .collect();
        let n = tune.max_connections;
        AppError::Conflict(format!(
            "{} of {} {} in use, watching {}. A channel already being watched can be joined, or one of these stopped.",
            if n == 1 { "The one connection".to_owned() } else { format!("All {n} connections") },
            tune.source_name,
            if n == 1 { "is" } else { "are" },
            names.join(", ")
        ))
    }

    fn viewer(&self, session_id: &str, user: &AuthUser) -> Result<Viewer, AppError> {
        let mut viewers = self.viewers.lock();
        let v = viewers
            .get_mut(session_id)
            .filter(|v| v.user_id == user.id)
            .ok_or_else(|| AppError::SessionStopped(Some("this channel isn't being watched any more".into())))?;
        v.last_seen = Instant::now();
        Ok(v.clone())
    }

    fn tuner_of(&self, viewer: &Viewer) -> Result<Arc<Tuner>, AppError> {
        self.tuners
            .lock()
            .get(&viewer.channel_id)
            .cloned()
            .ok_or_else(|| AppError::SessionStopped(Some("the channel was stopped".into())))
    }

    pub fn leave(&self, session_id: &str, user: &AuthUser) -> bool {
        let removed = {
            let mut viewers = self.viewers.lock();
            match viewers.get(session_id) {
                Some(v) if v.user_id == user.id || user.is_admin => viewers.remove(session_id).is_some(),
                _ => false,
            }
        };
        if removed {
            self.events.send(ServerEvent::LiveTunersChanged);
        }
        removed
    }

    /// The live playlist of an HLS viewer's channel, the transcoder's job
    /// made when it is the first.
    pub async fn playlist(
        &self,
        session_id: &str,
        user: &AuthUser,
        settings: &LiveSettings,
        hwaccel: HwAccel,
    ) -> Result<String, AppError> {
        let viewer = self.viewer(session_id, user)?;
        let tuner = self.tuner_of(&viewer)?;
        for attempt in 0..2 {
            let job = self.hls_job(&tuner, settings, hwaccel).await?;
            match self.transcoder.live_playlist(&job).await {
                Ok(text) => return Ok(text),
                Err(err) if err.is_job_gone() && attempt == 0 => {
                    // The transcoder restarted, or gave up on it: once more.
                    let mut hls = tuner.hls.lock().await;
                    if hls.as_deref() == Some(job.as_str()) {
                        *hls = None;
                    }
                }
                Err(err) => return Err(self.why_not(&tuner, err.into())),
            }
        }
        Err(self.why_not(&tuner, AppError::Unavailable("the channel didn't start".into())))
    }

    /// The error a player gets: what the provider did, when it is to blame.
    fn why_not(&self, tuner: &Tuner, fallback: AppError) -> AppError {
        let state = tuner.state.lock();
        match (&state.error, state.connected, &state.hls_failed) {
            (Some(why), false, _) => AppError::Unavailable(format!("{} isn't playing: {why}", tuner.channel_name)),
            (_, _, Some(why)) => AppError::Unavailable(format!("{} can't be shown: {why}", tuner.channel_name)),
            _ => fallback,
        }
    }

    async fn hls_job(&self, tuner: &Arc<Tuner>, settings: &LiveSettings, hwaccel: HwAccel) -> Result<String, AppError> {
        let mut hls = tuner.hls.lock().await;
        if let Some(job) = hls.clone() {
            return Ok(job);
        }
        if let Some(why) = tuner.state.lock().hls_failed.clone() {
            return Err(AppError::Unavailable(format!("{} can't be shown: {why}", tuner.channel_name)));
        }
        if !tuner.is_live() {
            return Err(self.why_not(tuner, AppError::Unavailable("the channel stopped".into())));
        }
        if !self.transcoder.is_available() {
            return Err(AppError::Unavailable("the transcoder isn't running".into()));
        }
        if !self.transcoder.makes_live() {
            return Err(AppError::Unavailable("this transcoder doesn't make live channels: update it".into()));
        }
        let created = self
            .transcoder
            .create_live(&LiveRequest {
                label: Some(tuner.channel_name.clone()),
                owner: Some("Live TV".into()),
                deinterlace: settings.deinterlace,
                force_transcode: false,
                max_height: settings.max_height,
                max_bitrate_kbps: None,
                hwaccel,
                window_seconds: Some(settings.timeshift_minutes * 60),
            })
            .await?;
        let input = tuner.subscribe().ok_or_else(|| AppError::Unavailable("the channel stopped".into()))?;
        let (transcoder, t, job) = (self.transcoder.clone(), tuner.clone(), created.id.clone());
        tokio::spawn(async move {
            let result = transcoder.feed_live(&job, input).await;
            if t.stop.is_cancelled() {
                return;
            }
            // The transcoder stopped making it: say why, unless the provider is to blame.
            if let Ok(status) = transcoder.live_status(&job).await {
                t.state.lock().plan = status.plan;
                let connected = t.state.lock().connected;
                if let Some(why) = status.error
                    && connected
                {
                    tracing::warn!(channel = t.channel_id, "the transcoder gave up on a live channel: {why}");
                    t.state.lock().hls_failed = Some(why);
                }
            } else if let Err(err) = result {
                tracing::warn!(channel = t.channel_id, "sending a live channel to the transcoder failed: {err}");
            }
            let mut hls = t.hls.lock().await;
            if hls.as_deref() == Some(job.as_str()) {
                *hls = None;
            }
        });
        // What it is made into, for the administrators' view, once it is known.
        let (transcoder, t, job) = (self.transcoder.clone(), tuner.clone(), created.id.clone());
        tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_secs(2)).await;
                match transcoder.live_status(&job).await {
                    Ok(status) if status.plan.is_some() => {
                        t.state.lock().plan = status.plan;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });
        *hls = Some(created.id.clone());
        Ok(created.id)
    }

    /// A segment of an HLS viewer's channel.
    pub async fn segment(&self, session_id: &str, user: &AuthUser, name: &str) -> Result<reqwest::Response, AppError> {
        if !is_segment_name(name) {
            return Err(AppError::not_found(format!("no file {name}")));
        }
        let viewer = self.viewer(session_id, user)?;
        let tuner = self.tuner_of(&viewer)?;
        let job = tuner.hls.lock().await.clone().ok_or_else(|| AppError::not_found("the channel is starting again"))?;
        Ok(self.transcoder.live_segment(&job, name).await?)
    }

    /// The provider's MPEG-TS of a stream viewer's channel, as it comes,
    /// for as long as the response is read.
    pub fn stream(
        self: &Arc<Self>,
        session_id: &str,
        user: &AuthUser,
    ) -> Result<impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static + use<>, AppError> {
        let viewer = self.viewer(session_id, user)?;
        let tuner = self.tuner_of(&viewer)?;
        let stream = tuner.subscribe().ok_or_else(|| AppError::Unavailable("the channel stopped".into()))?;
        let guard = Leaves { tuners: self.clone(), session_id: session_id.to_owned() };
        Ok(stream.map(move |chunk| {
            let _watching = &guard;
            chunk
        }))
    }

    /// The connections in use, for administrators.
    pub async fn status(&self) -> Vec<TunerStatus> {
        let tuners: Vec<Arc<Tuner>> = self.tuners.lock().values().filter(|t| !t.stop.is_cancelled()).cloned().collect();
        let viewers: Vec<Viewer> = self.viewers.lock().values().cloned().collect();
        let mut out: Vec<TunerStatus> = tuners
            .iter()
            .map(|t| {
                let watching: Vec<ViewerStatus> = viewers
                    .iter()
                    .filter(|v| v.channel_id == t.channel_id)
                    .map(|v| ViewerStatus {
                        session_id: v.id.clone(),
                        user_id: v.user_id,
                        username: v.username.clone(),
                        kind: v.kind,
                        started_at: v.started_at,
                    })
                    .collect();
                let kbps = t.kbps();
                let state = t.state.lock();
                TunerStatus {
                    channel_id: t.channel_id,
                    channel_name: t.channel_name.clone(),
                    source_id: t.source_id,
                    source_name: t.source_name.clone(),
                    started_at: t.started_at,
                    connected: state.connected,
                    error: state.error.clone().or_else(|| state.hls_failed.clone()),
                    bytes_in: state.bytes_in,
                    kbps,
                    idle: watching.is_empty(),
                    viewers: watching,
                    plan: state.plan.clone(),
                }
            })
            .collect();
        out.sort_by_key(|t| (t.source_id, t.started_at));
        out
    }
}

/// Ends a stream viewer when its response is dropped.
struct Leaves {
    tuners: Arc<Tuners>,
    session_id: String,
}

impl Drop for Leaves {
    fn drop(&mut self) {
        if self.tuners.viewers.lock().remove(&self.session_id).is_some() {
            self.tuners.events.send(ServerEvent::LiveTunersChanged);
        }
    }
}

/// `s{n}.ts`, the transcoder's live segments.
fn is_segment_name(name: &str) -> bool {
    name.strip_prefix('s')
        .and_then(|n| n.strip_suffix(".ts"))
        .is_some_and(|n| !n.is_empty() && n.len() <= 12 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Pulls the channel while the tuner lives, connecting again when it breaks.
async fn pull(tuner: Arc<Tuner>, http: reqwest::Client) {
    let mut failures = 0u32;
    let mut refused = false;
    while !tuner.stop.is_cancelled() {
        let before = tuner.state.lock().bytes_in;
        let result = tokio::select! {
            result = pull_once(&tuner, &http) => result,
            () = tuner.stop.cancelled() => break,
        };
        let got = tuner.state.lock().bytes_in - before;
        if got >= WORKED_BYTES {
            failures = 0;
        }
        failures += 1;
        let why = match result {
            Ok(()) => "the provider ended the stream".to_owned(),
            Err(why) => why,
        };
        refused = why.starts_with(REFUSED);
        tracing::info!(channel = tuner.channel_id, attempt = failures, "live channel's connection broke: {why}");
        {
            let mut state = tuner.state.lock();
            state.error = Some(why);
            state.connected = false;
        }
        if failures >= ATTEMPTS {
            break;
        }
        let wait = Duration::from_millis(500 * (1 << failures.min(4)));
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = tuner.stop.cancelled() => break,
        }
    }
    let gave_nothing = tuner.state.lock().bytes_in == 0;
    // Nothing ever came for it, and not for want of a connection: offline.
    if gave_nothing && !refused && !tuner.stop.is_cancelled() {
        let _ = tuner.played.send(Played { channel_id: tuner.channel_id, played: false });
    }
    tuner.state.lock().ended = true;
    tuner.feed.lock().tx = None;
}

/// One connection to the provider, until it ends: `Err` says how it failed.
async fn pull_once(tuner: &Tuner, http: &reqwest::Client) -> Result<(), String> {
    let mut req = http.get(&tuner.url);
    if !tuner.user_agent.is_empty() {
        req = req.header(reqwest::header::USER_AGENT, &tuner.user_agent);
    }
    let mut resp = tokio::time::timeout(Duration::from_secs(15), req.send())
        .await
        .map_err(|_| "the provider didn't answer".to_owned())?
        .map_err(|e| super::describe(&e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 | 403 | 458 | 509 => {
                format!("{REFUSED} ({status}): the account's connections may be in use elsewhere, or it expired")
            }
            404 => "the provider doesn't have this channel now (404)".into(),
            _ => format!("the provider answered {status}"),
        });
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let url = resp.url().clone();
    if content_type.contains("mpegurl") || url.path().to_ascii_lowercase().ends_with(".m3u8") {
        let text = read_text(resp).await?;
        return pull_hls(tuner, http, url, text).await;
    }
    loop {
        match tokio::time::timeout(STALL, resp.chunk()).await {
            Ok(Ok(Some(chunk))) => {
                // An HLS playlist served as something else.
                if tuner.state.lock().bytes_in == 0 && chunk.starts_with(b"#EXTM3U") {
                    let mut text = String::from_utf8_lossy(&chunk).into_owned();
                    text.push_str(&read_text(resp).await?);
                    return pull_hls(tuner, http, url, text).await;
                }
                tuner.push(chunk);
            }
            Ok(Ok(None)) => return Ok(()),
            Ok(Err(e)) => return Err(super::describe(&e)),
            Err(_) => return Err(format!("the provider sent nothing for {} s", STALL.as_secs())),
        }
    }
}

async fn read_text(resp: reqwest::Response) -> Result<String, String> {
    let bytes = tokio::time::timeout(Duration::from_secs(10), resp.bytes())
        .await
        .map_err(|_| "the provider's playlist didn't come".to_owned())?
        .map_err(|e| super::describe(&e))?;
    if bytes.len() > 2 << 20 {
        return Err("the provider's playlist is too large".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_text(tuner: &Tuner, http: &reqwest::Client, url: &url::Url) -> Result<String, String> {
    let mut req = http.get(url.clone()).timeout(Duration::from_secs(10));
    if !tuner.user_agent.is_empty() {
        req = req.header(reqwest::header::USER_AGENT, &tuner.user_agent);
    }
    let resp = req.send().await.map_err(|e| super::describe(&e))?;
    if !resp.status().is_success() {
        return Err(format!("the provider answered {} for its playlist", resp.status()));
    }
    read_text(resp).await
}

/// A channel the provider serves as HLS: its media playlist followed,
/// each new segment handed on as it comes.
async fn pull_hls(tuner: &Tuner, http: &reqwest::Client, mut url: url::Url, mut text: String) -> Result<(), String> {
    if let Some(variant) = best_variant(&text) {
        url = url.join(&variant).map_err(|_| "the provider's playlist names a bad address".to_owned())?;
        text = get_text(tuner, http, &url).await?;
    }
    let mut next: Option<u64> = None;
    loop {
        let playlist = media_playlist(&text)?;
        // Join a few segments behind the live edge, as players do.
        let first = next.unwrap_or_else(|| playlist.last_sequence().saturating_sub(2));
        for (sequence, uri) in playlist.segments.iter().filter(|(s, _)| *s >= first) {
            let segment = url.join(uri).map_err(|_| "the provider's playlist names a bad address".to_owned())?;
            let mut req = http.get(segment).timeout(Duration::from_secs(20));
            if !tuner.user_agent.is_empty() {
                req = req.header(reqwest::header::USER_AGENT, &tuner.user_agent);
            }
            let mut resp = req.send().await.map_err(|e| super::describe(&e))?;
            if !resp.status().is_success() {
                return Err(format!("the provider answered {} for a segment", resp.status()));
            }
            while let Some(chunk) = resp.chunk().await.map_err(|e| super::describe(&e))? {
                tuner.push(chunk);
            }
            next = Some(sequence + 1);
        }
        if playlist.ended {
            return Ok(());
        }
        let wait = Duration::from_secs_f64((playlist.target_duration / 2.0).clamp(1.0, 5.0));
        tokio::time::sleep(wait).await;
        text = get_text(tuner, http, &url).await?;
    }
}

struct MediaPlaylist {
    target_duration: f64,
    segments: Vec<(u64, String)>,
    ended: bool,
}

impl MediaPlaylist {
    fn last_sequence(&self) -> u64 {
        self.segments.last().map_or(0, |(s, _)| *s)
    }
}

/// The variant with the most bits of a multivariant playlist.
fn best_variant(text: &str) -> Option<String> {
    let mut best: Option<(u64, String)> = None;
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let bandwidth = attrs
                .split(',')
                .find_map(|a| a.trim().strip_prefix("BANDWIDTH="))
                .and_then(|b| b.parse().ok())
                .unwrap_or(0);
            if let Some(uri) = lines.by_ref().find(|l| !l.is_empty() && !l.starts_with('#'))
                && best.as_ref().is_none_or(|(b, _)| bandwidth > *b)
            {
                best = Some((bandwidth, uri.to_owned()));
            }
        }
    }
    best.map(|(_, uri)| uri)
}

fn media_playlist(text: &str) -> Result<MediaPlaylist, String> {
    let mut sequence = 0u64;
    let mut target_duration = 6.0;
    let mut segments = Vec::new();
    let mut ended = false;
    let mut in_segment = false;
    for line in text.lines().map(str::trim) {
        if let Some(n) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = n.parse().unwrap_or(0);
        } else if let Some(d) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            target_duration = d.parse().unwrap_or(6.0);
        } else if line.starts_with("#EXT-X-MAP") {
            return Err("the provider sends fragmented MP4, which isn't read yet".into());
        } else if let Some(key) = line.strip_prefix("#EXT-X-KEY:") {
            if !key.contains("METHOD=NONE") {
                return Err("the provider's stream is encrypted".into());
            }
        } else if line.starts_with("#EXT-X-ENDLIST") {
            ended = true;
        } else if line.starts_with("#EXTINF") {
            in_segment = true;
        } else if !line.is_empty() && !line.starts_with('#') && in_segment {
            segments.push((sequence + segments.len() as u64, line.to_owned()));
            in_segment = false;
        }
    }
    if segments.is_empty() && !ended {
        return Err("the provider's playlist has no segments".into());
    }
    Ok(MediaPlaylist { target_duration, segments, ended })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_variant_is_the_one_with_most_bits() {
        let text = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\nlow.m3u8\n\
                    #EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080\nhigh/index.m3u8\n";
        assert_eq!(best_variant(text).as_deref(), Some("high/index.m3u8"));
        assert_eq!(best_variant("#EXTM3U\n#EXTINF:2,\na.ts\n"), None);
    }

    #[test]
    fn media_playlists_number_their_segments() {
        let text =
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:100\n#EXTINF:4.0,\na.ts\n#EXTINF:4.0,\nb.ts\n";
        let p = media_playlist(text).unwrap();
        assert_eq!(p.segments, [(100, "a.ts".to_owned()), (101, "b.ts".to_owned())]);
        assert_eq!((p.target_duration, p.ended, p.last_sequence()), (4.0, false, 101));
        assert!(media_playlist("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\n#EXTINF:4,\na.ts\n").is_err());
        assert!(media_playlist("#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n").is_err());
    }

    #[test]
    fn segment_names_are_checked() {
        assert!(is_segment_name("s5.ts"));
        assert!(!is_segment_name("../s5.ts"));
        assert!(!is_segment_name("index.m3u8"));
    }
}
