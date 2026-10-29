//! Live TV (IPTV): channels from M3U playlists and Xtream Codes accounts,
//! their guide from XMLTV, watched through this server.
//!
//! The server holds the connections to the provider ([`tuner`]): a channel
//! is pulled once however many watch it, and a source plays no more channels
//! at once than its provider allows (`max_connections`). Players get the
//! channel from the transcoder as live HLS (H.264 and AAC), or as the
//! provider's MPEG-TS untouched (`stream.ts`, for players that read it).
//! Logos and programme pictures come through the server too: the web app
//! loads nothing from elsewhere.

pub mod m3u;
pub mod tuner;
pub mod xmltv;
pub mod xtream;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::auth::AuthUser;
use crate::db::Db;
use crate::error::AppError;
use crate::events::{EventBus, ServerEvent};
use crate::transcoder::Transcoder;
use crate::util::now;

pub use tuner::Tuners;

/// The settings table's key for [`LiveSettings`].
const SETTINGS_KEY: &str = "live_tv";
/// A playlist larger than this isn't read (they run to tens of MB with films).
const MAX_PLAYLIST_BYTES: usize = 200 << 20;
/// A guide larger than this isn't read (gzipped ones are counted as they come).
const MAX_GUIDE_BYTES: u64 = 3 << 30;
const MAX_PICTURE_BYTES: usize = 4 << 20;
/// A channel its source stopped listing is forgotten after this long.
const MISSING_KEPT: i64 = 14 * 86_400;
/// Programmes kept behind now (the guide shows the past few hours).
const GUIDE_BEHIND: i64 = 6 * 3600;
/// After a failed refresh, the next try waits this long.
const RETRY_AFTER: Duration = Duration::from_secs(30 * 60);
/// A picture that failed to download isn't asked for again for this long.
const PICTURE_RETRY: Duration = Duration::from_secs(3600);
/// Rows inserted per write transaction (the database has one writer).
const BATCH: usize = 2000;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// Everyone with an account (see [`LiveSettings::age_limited`]).
    #[default]
    Everyone,
    /// Administrators only.
    Admins,
}

/// Live TV's settings (administrators', in Settings → Live TV).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LiveSettings {
    pub access: Access,
    /// Profiles with an age limit see Live TV too (channels have no ratings
    /// to hold them to it).
    pub age_limited: bool,
    /// Interlaced channels are made progressive (transcoded) rather than
    /// sent as they are, which players show combed.
    pub deinterlace: bool,
    /// The most lines a transcoded channel has; `None`: as many as it has.
    pub max_height: Option<u32>,
    /// How far back a channel can be paused and rewound, in minutes.
    pub timeshift_minutes: u32,
    /// Channels and guides are read again this often, in hours.
    pub refresh_hours: u32,
    /// Days of guide kept ahead.
    pub guide_days: u32,
}

impl Default for LiveSettings {
    fn default() -> Self {
        Self {
            access: Access::Everyone,
            age_limited: false,
            deinterlace: true,
            max_height: None,
            timeshift_minutes: 5,
            refresh_hours: 12,
            guide_days: 7,
        }
    }
}

impl LiveSettings {
    fn check(&self) -> Result<(), AppError> {
        if !(1..=60).contains(&self.timeshift_minutes) {
            return Err(AppError::bad_request("timeshift_minutes must be between 1 and 60"));
        }
        if !(1..=168).contains(&self.refresh_hours) {
            return Err(AppError::bad_request("refresh_hours must be between 1 and 168"));
        }
        if !(1..=14).contains(&self.guide_days) {
            return Err(AppError::bad_request("guide_days must be between 1 and 14"));
        }
        if self.max_height.is_some_and(|h| !(240..=2160).contains(&h)) {
            return Err(AppError::bad_request("max_height must be between 240 and 2160"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    M3u,
    Xtream,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::M3u => "m3u",
            SourceKind::Xtream => "xtream",
        }
    }
}

/// A source as administrators see it (never its password).
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub id: i64,
    pub name: String,
    pub kind: SourceKind,
    pub url: String,
    pub username: String,
    pub password_set: bool,
    pub guide_url: String,
    pub user_agent: String,
    pub max_connections: i64,
    pub enabled: bool,
    pub created_at: i64,
    pub channels_at: Option<i64>,
    pub channels_error: Option<String>,
    pub guide_at: Option<i64>,
    pub guide_error: Option<String>,
    pub account: Option<xtream::Account>,
    /// Channels it lists now, and of those hidden.
    pub channel_count: i64,
    pub hidden_count: i64,
    /// Channels with a guide.
    pub guided_count: i64,
    /// Connections in use now.
    pub connections_in_use: usize,
    /// Being read now.
    pub refreshing: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceInput {
    pub name: String,
    pub kind: SourceKind,
    pub url: String,
    #[serde(default)]
    pub username: String,
    /// `None` keeps the one saved.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub guide_url: String,
    #[serde(default)]
    pub user_agent: String,
    #[serde(default = "one")]
    pub max_connections: i64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn one() -> i64 {
    1
}

fn yes() -> bool {
    true
}

/// What a test of a source's details found.
#[derive(Debug, Clone, Serialize)]
pub struct SourceTest {
    pub channels: usize,
    pub groups: usize,
    pub guide_url: Option<String>,
    pub account: Option<xtream::Account>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct SourceRow {
    id: i64,
    name: String,
    kind: String,
    url: String,
    username: String,
    password: String,
    guide_url: String,
    user_agent: String,
    max_connections: i64,
    enabled: bool,
    created_at: i64,
    channels_at: Option<i64>,
    channels_error: Option<String>,
    guide_at: Option<i64>,
    guide_error: Option<String>,
    account: Option<String>,
}

impl SourceRow {
    fn kind(&self) -> SourceKind {
        if self.kind == "xtream" { SourceKind::Xtream } else { SourceKind::M3u }
    }

    /// The guide read for it: its own, or an Xtream account's.
    fn effective_guide_url(&self) -> Option<String> {
        if !self.guide_url.trim().is_empty() {
            return Some(self.guide_url.trim().to_owned());
        }
        (self.kind() == SourceKind::Xtream)
            .then(|| xtream::guide_url(&xtream::base(&self.url), &self.username, &self.password))
    }
}

/// A programme, as apps get it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Program {
    pub channel_id: i64,
    pub start: i64,
    pub stop: i64,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub categories: Vec<String>,
    /// `/api/live/channels/{id}/programs/{start}/image`, when it has one.
    pub image: Option<String>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub is_new: bool,
    pub is_live: bool,
    pub rating: Option<String>,
}

/// A channel, as apps get it.
#[derive(Debug, Clone, Serialize)]
pub struct Channel {
    pub id: i64,
    pub source_id: i64,
    pub name: String,
    pub number: Option<String>,
    pub group: String,
    /// `/api/live/channels/{id}/logo`, when it has one.
    pub logo: Option<String>,
    pub favorite: bool,
    /// Whether the guide has programmes for it.
    pub has_guide: bool,
    pub now: Option<Program>,
    pub next: Option<Program>,
}

/// A channel as administrators see it: as the source has it and as set.
#[derive(Debug, Clone, Serialize)]
pub struct AdminChannel {
    pub id: i64,
    pub source_id: i64,
    pub name: String,
    pub number: Option<String>,
    pub group: String,
    pub logo: Option<String>,
    pub guide_id: String,
    pub hidden: bool,
    pub source_name: String,
    pub source_number: Option<String>,
    pub source_guide_id: String,
    /// The guide's channel of its name, found when it names none.
    pub matched_guide_id: Option<String>,
    pub custom_name: Option<String>,
    pub custom_number: Option<String>,
    pub custom_guide_id: Option<String>,
    pub has_guide: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChannelPatch {
    #[serde(default)]
    pub hidden: Option<bool>,
    /// `""` clears it (back to the source's).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub guide_id: Option<String>,
}

/// The guide for a span of time.
#[derive(Debug, Clone, Serialize)]
pub struct Guide {
    pub from: i64,
    pub to: i64,
    /// Programmes by channel id.
    pub programs: HashMap<i64, Vec<Program>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChannelRow {
    pub id: i64,
    pub source_id: i64,
    name: String,
    number: Option<String>,
    group_name: String,
    logo_url: String,
    guide_id: String,
    matched_guide_id: Option<String>,
    pub stream_url: String,
    user_agent: Option<String>,
    position: i64,
    hidden: bool,
    custom_name: Option<String>,
    custom_number: Option<String>,
    custom_guide_id: Option<String>,
}

impl ChannelRow {
    pub fn display_name(&self) -> &str {
        self.custom_name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&self.name)
    }

    fn display_number(&self) -> Option<String> {
        self.custom_number.clone().filter(|n| !n.is_empty()).or_else(|| self.number.clone())
    }

    /// The guide's channel it takes its programmes from.
    fn guide_key(&self) -> Option<&str> {
        [self.custom_guide_id.as_deref(), Some(self.guide_id.as_str()), self.matched_guide_id.as_deref()]
            .into_iter()
            .flatten()
            .find(|id| !id.is_empty())
    }
}

const CHANNEL_COLUMNS: &str = "c.id, c.source_id, c.name, c.number, c.group_name, c.logo_url, c.guide_id, \
     c.matched_guide_id, c.stream_url, c.user_agent, c.position, c.hidden, c.custom_name, c.custom_number, \
     c.custom_guide_id";

#[derive(Debug, Clone, sqlx::FromRow)]
struct ProgramRow {
    source_id: i64,
    guide_id: String,
    start: i64,
    stop: i64,
    title: String,
    subtitle: Option<String>,
    description: Option<String>,
    categories: Option<String>,
    image: Option<String>,
    season: Option<i64>,
    episode: Option<i64>,
    is_new: bool,
    is_live: bool,
    rating: Option<String>,
}

impl ProgramRow {
    fn for_channel(&self, channel_id: i64) -> Program {
        Program {
            channel_id,
            start: self.start,
            stop: self.stop,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            description: self.description.clone(),
            categories: self.categories.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or_default(),
            image: self
                .image
                .as_ref()
                .map(|_| format!("/api/live/channels/{channel_id}/programs/{}/image", self.start)),
            season: self.season,
            episode: self.episode,
            is_new: self.is_new,
            is_live: self.is_live,
            rating: self.rating.clone(),
        }
    }
}

pub struct LiveTv {
    db: Db,
    events: EventBus,
    settings: RwLock<Arc<LiveSettings>>,
    http: reqwest::Client,
    pub tuners: Arc<Tuners>,
    /// Logos and programme pictures, downloaded once.
    pictures: PathBuf,
    /// Pictures whose download failed lately (by URL).
    failed_pictures: Mutex<HashMap<String, Instant>>,
    /// Guides are downloaded here before they are read.
    downloads: PathBuf,
    /// Sources being read now, and when each was last tried.
    refreshing: Mutex<HashSet<i64>>,
    tried: Mutex<HashMap<i64, Instant>>,
    refresh_lock: tokio::sync::Mutex<()>,
}

impl LiveTv {
    pub async fn new(
        db: Db,
        events: EventBus,
        transcoder: Arc<Transcoder>,
        cache_dir: PathBuf,
    ) -> anyhow::Result<Arc<Self>> {
        let stored: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(SETTINGS_KEY)
            .fetch_optional(&db.read)
            .await?;
        let settings: LiveSettings = stored.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let http = reqwest::Client::builder()
            .user_agent(concat!("pley/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        let settings = RwLock::new(Arc::new(settings));
        let tuners = Tuners::new(transcoder, events.clone(), http.clone());
        let pictures = cache_dir.join("live").join("pictures");
        let downloads = cache_dir.join("live").join("downloads");
        let _ = std::fs::remove_dir_all(&downloads);
        Ok(Arc::new(Self {
            db,
            events,
            settings,
            http,
            tuners,
            pictures,
            failed_pictures: Mutex::default(),
            downloads,
            refreshing: Mutex::default(),
            tried: Mutex::default(),
            refresh_lock: tokio::sync::Mutex::new(()),
        }))
    }

    pub fn settings(&self) -> Arc<LiveSettings> {
        self.settings.read().clone()
    }

    pub async fn save_settings(&self, settings: LiveSettings) -> Result<Arc<LiveSettings>, AppError> {
        settings.check()?;
        let json = serde_json::to_string(&settings).map_err(|e| AppError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(SETTINGS_KEY)
        .bind(json)
        .execute(&self.db.write)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        let settings = Arc::new(settings);
        *self.settings.write() = settings.clone();
        self.events.send(ServerEvent::LiveChannelsChanged);
        Ok(settings)
    }

    /// Whether `user` may watch Live TV.
    pub fn may_watch(&self, user: &AuthUser) -> bool {
        let settings = self.settings();
        match settings.access {
            Access::Admins => user.is_admin,
            Access::Everyone => user.is_admin || user.max_age.is_none() || settings.age_limited,
        }
    }

    pub fn check_access(&self, user: &AuthUser) -> Result<(), AppError> {
        if self.may_watch(user) { Ok(()) } else { Err(AppError::forbidden("Live TV isn't open to this account")) }
    }

    /// Reads sources that are due, every minute, until `stopping`.
    pub fn start(self: &Arc<Self>, stopping: CancellationToken) {
        self.tuners.start(stopping.clone());
        let this = self.clone();
        tokio::spawn(async move {
            // Leave start-up to the libraries first.
            tokio::select! {
                () = stopping.cancelled() => return,
                () = tokio::time::sleep(Duration::from_secs(20)) => {}
            }
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = stopping.cancelled() => break,
                    _ = tick.tick() => {}
                }
                this.refresh_due().await;
            }
        });
    }

    async fn refresh_due(&self) {
        let every = i64::from(self.settings().refresh_hours) * 3600;
        let Ok(sources) = self.source_rows().await else { return };
        for source in sources.into_iter().filter(|s| s.enabled) {
            let tried_lately = self.tried.lock().get(&source.id).is_some_and(|t| t.elapsed() < RETRY_AFTER);
            let channels_due = source.channels_at.is_none_or(|t| now() - t >= every);
            let guide_due =
                source.effective_guide_url().is_some() && source.guide_at.is_none_or(|t| now() - t >= every);
            if (channels_due || guide_due) && !tried_lately {
                self.refresh(source.id, channels_due, true).await;
            }
        }
        let _ = sqlx::query("DELETE FROM live_programs WHERE stop < ?")
            .bind(now() - GUIDE_BEHIND)
            .execute(&self.db.write)
            .await;
    }

    // ------------------------------------------------------------ sources

    async fn source_rows(&self) -> sqlx::Result<Vec<SourceRow>> {
        sqlx::query_as("SELECT * FROM live_sources ORDER BY id").fetch_all(&self.db.read).await
    }

    async fn source_row(&self, id: i64) -> Result<SourceRow, AppError> {
        sqlx::query_as("SELECT * FROM live_sources WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db.read)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .ok_or_else(|| AppError::not_found(format!("live TV source {id} not found")))
    }

    pub async fn sources(&self) -> Result<Vec<Source>, AppError> {
        let rows = self.source_rows().await.map_err(|e| AppError::Internal(e.into()))?;
        let counts: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT source_id, COUNT(*), SUM(hidden) FROM live_channels WHERE missing_since IS NULL GROUP BY source_id",
        )
        .fetch_all(&self.db.read)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        let guided = self.guided_counts().await?;
        let in_use = self.tuners.in_use();
        let refreshing = self.refreshing.lock().clone();
        Ok(rows
            .into_iter()
            .map(|r| {
                let (count, hidden) =
                    counts.iter().find(|(id, ..)| *id == r.id).map(|(_, c, h)| (*c, *h)).unwrap_or((0, 0));
                Source {
                    kind: r.kind(),
                    account: r.account.as_deref().and_then(|a| serde_json::from_str(a).ok()),
                    password_set: !r.password.is_empty(),
                    channel_count: count,
                    hidden_count: hidden,
                    guided_count: guided.get(&r.id).copied().unwrap_or(0),
                    connections_in_use: in_use.get(&r.id).copied().unwrap_or(0),
                    refreshing: refreshing.contains(&r.id),
                    id: r.id,
                    name: r.name,
                    url: r.url,
                    username: r.username,
                    guide_url: r.guide_url,
                    user_agent: r.user_agent,
                    max_connections: r.max_connections,
                    enabled: r.enabled,
                    created_at: r.created_at,
                    channels_at: r.channels_at,
                    channels_error: r.channels_error,
                    guide_at: r.guide_at,
                    guide_error: r.guide_error,
                }
            })
            .collect())
    }

    /// Channels with programmes now or ahead, by source.
    async fn guided_counts(&self) -> Result<HashMap<i64, i64>, AppError> {
        let channels = self.channel_rows(None, true).await?;
        let keys: HashSet<(i64, String)> =
            sqlx::query_as::<_, (i64, String)>("SELECT DISTINCT source_id, guide_id FROM live_programs WHERE stop > ?")
                .bind(now())
                .fetch_all(&self.db.read)
                .await
                .map_err(|e| AppError::Internal(e.into()))?
                .into_iter()
                .collect();
        let mut out = HashMap::new();
        for c in channels {
            if c.guide_key().is_some_and(|k| keys.contains(&(c.source_id, k.to_owned()))) {
                *out.entry(c.source_id).or_insert(0) += 1;
            }
        }
        Ok(out)
    }

    fn check_input(input: &SourceInput) -> Result<(), AppError> {
        if input.name.trim().is_empty() {
            return Err(AppError::bad_request("a source needs a name"));
        }
        let url = url::Url::parse(input.url.trim()).map_err(|_| AppError::bad_request("the address isn't a URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::bad_request("the address must start with http:// or https://"));
        }
        if input.kind == SourceKind::Xtream && input.username.trim().is_empty() {
            return Err(AppError::bad_request("an Xtream Codes account needs its user name"));
        }
        if !input.guide_url.trim().is_empty()
            && url::Url::parse(input.guide_url.trim()).is_err_and(|_| !input.guide_url.trim().starts_with('/'))
        {
            return Err(AppError::bad_request("the guide's address isn't a URL"));
        }
        if !(0..=100).contains(&input.max_connections) {
            return Err(AppError::bad_request("connections must be between 0 (no limit) and 100"));
        }
        Ok(())
    }

    pub async fn add_source(self: &Arc<Self>, input: SourceInput) -> Result<Source, AppError> {
        Self::check_input(&input)?;
        let url = match input.kind {
            SourceKind::Xtream => xtream::base(&input.url),
            SourceKind::M3u => input.url.trim().to_owned(),
        };
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO live_sources (name, kind, url, username, password, guide_url, user_agent, max_connections, \
             enabled, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(input.name.trim())
        .bind(input.kind.as_str())
        .bind(url)
        .bind(input.username.trim())
        .bind(input.password.unwrap_or_default())
        .bind(input.guide_url.trim())
        .bind(input.user_agent.trim())
        .bind(input.max_connections)
        .bind(input.enabled)
        .bind(now())
        .fetch_one(&self.db.write)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        self.refresh_soon(id);
        self.source(id).await
    }

    pub async fn update_source(self: &Arc<Self>, id: i64, input: SourceInput) -> Result<Source, AppError> {
        Self::check_input(&input)?;
        let before = self.source_row(id).await?;
        let url = match input.kind {
            SourceKind::Xtream => xtream::base(&input.url),
            SourceKind::M3u => input.url.trim().to_owned(),
        };
        let password = input.password.clone().unwrap_or(before.password.clone());
        let reread = url != before.url
            || input.kind != before.kind()
            || input.username.trim() != before.username
            || password != before.password
            || input.guide_url.trim() != before.guide_url
            || (input.enabled && !before.enabled);
        sqlx::query(
            "UPDATE live_sources SET name = ?, kind = ?, url = ?, username = ?, password = ?, guide_url = ?, \
             user_agent = ?, max_connections = ?, enabled = ? WHERE id = ?",
        )
        .bind(input.name.trim())
        .bind(input.kind.as_str())
        .bind(url)
        .bind(input.username.trim())
        .bind(password)
        .bind(input.guide_url.trim())
        .bind(input.user_agent.trim())
        .bind(input.max_connections)
        .bind(input.enabled)
        .bind(id)
        .execute(&self.db.write)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        if !input.enabled {
            self.tuners.stop_source(id).await;
        }
        if reread {
            self.refresh_soon(id);
        }
        self.events.send(ServerEvent::LiveChannelsChanged);
        self.source(id).await
    }

    pub async fn delete_source(&self, id: i64) -> Result<(), AppError> {
        self.tuners.stop_source(id).await;
        let done = sqlx::query("DELETE FROM live_sources WHERE id = ?")
            .bind(id)
            .execute(&self.db.write)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if done.rows_affected() == 0 {
            return Err(AppError::not_found(format!("live TV source {id} not found")));
        }
        self.events.send(ServerEvent::LiveChannelsChanged);
        Ok(())
    }

    pub async fn source(&self, id: i64) -> Result<Source, AppError> {
        self.sources()
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::not_found(format!("live TV source {id} not found")))
    }

    /// Reads a source again in the background.
    pub fn refresh_soon(self: &Arc<Self>, id: i64) {
        let this = self.clone();
        tokio::spawn(async move { this.refresh(id, true, true).await });
    }

    /// Reads a source's details without saving them: what an administrator
    /// is about to add.
    pub async fn test_source(&self, input: SourceInput, saved: Option<i64>) -> Result<SourceTest, AppError> {
        Self::check_input(&input)?;
        let password = match (input.password.clone(), saved) {
            (Some(p), _) => p,
            (None, Some(id)) => self.source_row(id).await?.password,
            (None, None) => String::new(),
        };
        let row = SourceRow {
            id: 0,
            name: input.name.clone(),
            kind: input.kind.as_str().into(),
            url: if input.kind == SourceKind::Xtream { xtream::base(&input.url) } else { input.url.trim().into() },
            username: input.username.trim().into(),
            password,
            guide_url: input.guide_url.trim().into(),
            user_agent: input.user_agent.trim().into(),
            max_connections: input.max_connections,
            enabled: true,
            created_at: now(),
            channels_at: None,
            channels_error: None,
            guide_at: None,
            guide_error: None,
            account: None,
        };
        let listed = self.list_channels(&row).await.map_err(|e| AppError::BadRequest(format!("{e:#}")))?;
        let groups: HashSet<&str> = listed.entries.iter().map(|e| e.group.as_str()).collect();
        Ok(SourceTest {
            channels: listed.entries.len(),
            groups: groups.len(),
            guide_url: listed.guide_url.or_else(|| row.effective_guide_url()).map(|u| redact(&u, &row.password)),
            account: listed.account,
        })
    }

    /// Reads a source's channels (when `channels`) and its guide (when
    /// `guide`), saying how it went in the source.
    pub async fn refresh(&self, id: i64, channels: bool, guide: bool) {
        if !self.refreshing.lock().insert(id) {
            return;
        }
        self.tried.lock().insert(id, Instant::now());
        self.events.send(ServerEvent::LiveChannelsChanged);
        // One source at a time: guides are large.
        let _one = self.refresh_lock.lock().await;
        if channels {
            let result = self.refresh_channels(id).await;
            if let Err(err) = &result {
                tracing::warn!(source = id, "reading the live TV channels failed: {err:#}");
            }
            let _ = sqlx::query("UPDATE live_sources SET channels_error = ? WHERE id = ?")
                .bind(result.err().map(|e| format!("{e:#}")))
                .bind(id)
                .execute(&self.db.write)
                .await;
            self.events.send(ServerEvent::LiveChannelsChanged);
        }
        if guide {
            let result = self.refresh_guide(id).await;
            match &result {
                Ok(Some(n)) => tracing::info!(source = id, programs = n, "live TV guide read"),
                Ok(None) => {}
                Err(err) => tracing::warn!(source = id, "reading the live TV guide failed: {err:#}"),
            }
            if let Err(err) = result {
                let _ = sqlx::query("UPDATE live_sources SET guide_error = ? WHERE id = ?")
                    .bind(format!("{err:#}"))
                    .bind(id)
                    .execute(&self.db.write)
                    .await;
            }
        }
        self.refreshing.lock().remove(&id);
        self.events.send(ServerEvent::LiveChannelsChanged);
    }

    async fn list_channels(&self, source: &SourceRow) -> anyhow::Result<Listed> {
        match source.kind() {
            SourceKind::M3u => {
                let bytes = self.download(&source.url, &source.user_agent, MAX_PLAYLIST_BYTES).await?;
                let text = String::from_utf8_lossy(&bytes);
                if !text.trim_start_matches('\u{feff}').trim_start().starts_with("#EXTM3U") && !text.contains("#EXTINF")
                {
                    bail!("the address doesn't give an M3U playlist");
                }
                let playlist = m3u::parse(&text);
                if playlist.entries.is_empty() {
                    bail!("the playlist lists no channels");
                }
                Ok(Listed { entries: playlist.entries, guide_url: playlist.guide_url, account: None })
            }
            SourceKind::Xtream => {
                let base = xtream::base(&source.url);
                let get = |action: Option<&'static str>| {
                    let url = xtream::api_url(&base, &source.username, &source.password, action);
                    async move {
                        let bytes = self
                            .download(&url, &source.user_agent, MAX_PLAYLIST_BYTES)
                            .await
                            .map_err(|e| anyhow::anyhow!(redact(&format!("{e:#}"), &source.password)))?;
                        serde_json::from_slice::<serde_json::Value>(&bytes)
                            .context("the server didn't answer as an Xtream Codes server does")
                    }
                };
                let account = xtream::account(&get(None).await?).map_err(|e| anyhow::anyhow!(e))?;
                if !account.status.eq_ignore_ascii_case("active") {
                    bail!("the provider says the account is {}", account.status.to_lowercase());
                }
                let categories = xtream::categories(&get(Some("get_live_categories")).await.unwrap_or_default());
                let streams = xtream::streams(&get(Some("get_live_streams")).await?, &categories);
                if streams.is_empty() {
                    bail!("the account has no live channels");
                }
                let entries = streams
                    .into_iter()
                    .map(|s| m3u::Entry {
                        key: s.id.to_string(),
                        url: xtream::stream_url(&base, &source.username, &source.password, s.id),
                        name: s.name,
                        number: s.number,
                        group: s.category,
                        logo: s.logo,
                        guide_id: s.guide_id,
                        user_agent: None,
                    })
                    .collect();
                Ok(Listed { entries, guide_url: None, account: Some(account) })
            }
        }
    }

    async fn refresh_channels(&self, id: i64) -> anyhow::Result<()> {
        let source = self.source_row(id).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        let listed = self.list_channels(&source).await?;
        let at = now();
        let mut tx = self.db.write.begin().await?;
        sqlx::query("UPDATE live_channels SET missing_since = COALESCE(missing_since, ?) WHERE source_id = ?")
            .bind(at)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for (position, e) in listed.entries.iter().enumerate() {
            sqlx::query(
                "INSERT INTO live_channels (source_id, key, name, number, group_name, logo_url, guide_id, stream_url, \
                 user_agent, position) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(source_id, key) DO UPDATE SET name = excluded.name, number = excluded.number, \
                 group_name = excluded.group_name, logo_url = excluded.logo_url, guide_id = excluded.guide_id, \
                 stream_url = excluded.stream_url, user_agent = excluded.user_agent, position = excluded.position, \
                 missing_since = NULL",
            )
            .bind(id)
            .bind(&e.key)
            .bind(&e.name)
            .bind(&e.number)
            .bind(&e.group)
            .bind(&e.logo)
            .bind(&e.guide_id)
            .bind(&e.url)
            .bind(&e.user_agent)
            .bind(position as i64)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("DELETE FROM live_channels WHERE source_id = ? AND missing_since < ?")
            .bind(id)
            .bind(at - MISSING_KEPT)
            .execute(&mut *tx)
            .await?;
        let mut set = String::from("UPDATE live_sources SET channels_at = ?, channels_error = NULL");
        if source.guide_url.is_empty() && listed.guide_url.is_some() {
            set.push_str(", guide_url = ?");
        }
        if listed.account.is_some() {
            set.push_str(", account = ?");
        }
        set.push_str(" WHERE id = ?");
        let mut query = sqlx::query(sqlx::AssertSqlSafe(set.as_str())).bind(at);
        if source.guide_url.is_empty()
            && let Some(guide) = &listed.guide_url
        {
            query = query.bind(guide);
        }
        if let Some(account) = &listed.account {
            query = query.bind(serde_json::to_string(account)?);
        }
        query.bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        tracing::info!(source = id, channels = listed.entries.len(), "live TV channels read");
        Ok(())
    }

    /// Reads the source's guide: the number of programmes kept, `None` when
    /// it has no guide.
    async fn refresh_guide(&self, id: i64) -> anyhow::Result<Option<usize>> {
        let source = self.source_row(id).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        let Some(url) = source.effective_guide_url() else { return Ok(None) };
        let path = self.download_guide(&url, &source).await?;
        let channels = self.channel_rows(Some(id), true).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        let days = i64::from(self.settings().guide_days);
        let (from, to) = (now() - GUIDE_BEHIND, now() + days * 86_400);
        let read_path = path.clone();
        let channels_for_match = channels.clone();
        let (guide, matches) = crate::util::blocking(move || {
            let file = std::fs::File::open(&read_path)?;
            let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
            let gzipped = std::io::BufRead::fill_buf(&mut reader)?.starts_with(&[0x1f, 0x8b]);
            let input: Box<dyn std::io::BufRead> = if gzipped {
                Box::new(std::io::BufReader::new(flate2::bufread::MultiGzDecoder::new(reader)))
            } else {
                Box::new(reader)
            };
            let mut matches: HashMap<i64, String> = HashMap::new();
            let guide = xmltv::read(input, from, to, |guide_channels| {
                let ids: HashSet<&str> = guide_channels.iter().map(|c| c.id.as_str()).collect();
                let mut by_name: HashMap<String, &str> = HashMap::new();
                for c in guide_channels {
                    for name in &c.names {
                        by_name.entry(normalize(name)).or_insert(&c.id);
                    }
                }
                let mut wanted = HashSet::new();
                for c in &channels_for_match {
                    for id in [c.custom_guide_id.as_deref(), Some(c.guide_id.as_str())].into_iter().flatten() {
                        if ids.contains(id) {
                            wanted.insert(id.to_owned());
                        }
                    }
                    let named = [c.custom_guide_id.as_deref(), Some(c.guide_id.as_str())]
                        .into_iter()
                        .flatten()
                        .any(|id| ids.contains(id));
                    if !named && let Some(id) = by_name.get(&normalize(c.display_name())) {
                        matches.insert(c.id, (*id).to_owned());
                        wanted.insert((*id).to_owned());
                    }
                }
                wanted
            })
            .context("the guide isn't XMLTV")?;
            Ok((guide, matches))
        })
        .await?;
        let _ = tokio::fs::remove_file(&path).await;
        if guide.channels.is_empty() {
            bail!("the guide lists no channels");
        }
        let count = guide.programs.len();
        // The old programmes go, then the new ones come in batches: the
        // database has one writer, which others wait for meanwhile.
        let mut tx = self.db.write.begin().await?;
        sqlx::query("DELETE FROM live_programs WHERE source_id = ?").bind(id).execute(&mut *tx).await?;
        for c in &channels {
            sqlx::query("UPDATE live_channels SET matched_guide_id = ? WHERE id = ?")
                .bind(matches.get(&c.id))
                .bind(c.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        for batch in guide.programs.chunks(BATCH) {
            let mut tx = self.db.write.begin().await?;
            for p in batch {
                sqlx::query(
                    "INSERT OR REPLACE INTO live_programs (source_id, guide_id, start, stop, title, subtitle, description, \
                     categories, image, season, episode, is_new, is_live, rating) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(id)
                .bind(&p.channel)
                .bind(p.start)
                .bind(p.stop)
                .bind(&p.title)
                .bind(&p.subtitle)
                .bind(&p.description)
                .bind((!p.categories.is_empty()).then(|| serde_json::to_string(&p.categories).unwrap_or_default()))
                .bind(&p.image)
                .bind(p.season)
                .bind(p.episode)
                .bind(p.is_new)
                .bind(p.is_live)
                .bind(&p.rating)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
        sqlx::query("UPDATE live_sources SET guide_at = ?, guide_error = NULL WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(&self.db.write)
            .await?;
        Ok(Some(count))
    }

    async fn download(&self, url: &str, user_agent: &str, max: usize) -> anyhow::Result<Vec<u8>> {
        let mut req = self.http.get(url).timeout(Duration::from_secs(120));
        if !user_agent.is_empty() {
            req = req.header(reqwest::header::USER_AGENT, user_agent);
        }
        let mut resp = req.send().await.map_err(|e| anyhow::anyhow!("{}", describe(&e)))?;
        if !resp.status().is_success() {
            bail!("the provider answered {}", resp.status());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            anyhow::ensure!(bytes.len() + chunk.len() <= max, "the answer is larger than {} MB", max >> 20);
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    /// The guide at `url` (or a file on this computer) in a file to read.
    async fn download_guide(&self, url: &str, source: &SourceRow) -> anyhow::Result<PathBuf> {
        use tokio::io::AsyncWriteExt;
        tokio::fs::create_dir_all(&self.downloads).await?;
        let path = self.downloads.join(format!("{}.xml", uuid::Uuid::new_v4().simple()));
        if let Some(local) = url.strip_prefix("file://").or_else(|| url.starts_with('/').then_some(url)) {
            tokio::fs::copy(local, &path).await.with_context(|| format!("{local} can't be read"))?;
            return Ok(path);
        }
        let mut req = self.http.get(url).timeout(Duration::from_secs(15 * 60));
        if !source.user_agent.is_empty() {
            req = req.header(reqwest::header::USER_AGENT, &source.user_agent);
        }
        let mut resp = req.send().await.map_err(|e| anyhow::anyhow!("{}", redact(&describe(&e), &source.password)))?;
        if !resp.status().is_success() {
            bail!("the guide's server answered {}", resp.status());
        }
        let mut file = tokio::fs::File::create(&path).await?;
        let mut written = 0u64;
        while let Some(chunk) = resp.chunk().await? {
            written += chunk.len() as u64;
            if written > MAX_GUIDE_BYTES {
                drop(file);
                let _ = tokio::fs::remove_file(&path).await;
                bail!("the guide is larger than {} GB", MAX_GUIDE_BYTES >> 30);
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(path)
    }

    // ----------------------------------------------------------- channels

    /// Channels of `source` (every source's when `None`) its list has now;
    /// hidden ones too when `hidden`. Enabled sources' only.
    pub(crate) async fn channel_rows(&self, source: Option<i64>, hidden: bool) -> Result<Vec<ChannelRow>, AppError> {
        let sql = format!(
            "SELECT {CHANNEL_COLUMNS} FROM live_channels c JOIN live_sources s ON s.id = c.source_id \
             WHERE c.missing_since IS NULL AND s.enabled = 1 AND (?1 IS NULL OR c.source_id = ?1) \
             AND (?2 = 1 OR c.hidden = 0) ORDER BY c.source_id, c.position"
        );
        let mut rows: Vec<ChannelRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(source)
            .bind(hidden)
            .fetch_all(&self.db.read)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        sort_channels(&mut rows);
        Ok(rows)
    }

    pub(crate) async fn channel_row(&self, id: i64) -> Result<ChannelRow, AppError> {
        let sql = format!(
            "SELECT {CHANNEL_COLUMNS} FROM live_channels c JOIN live_sources s ON s.id = c.source_id \
             WHERE c.id = ? AND c.missing_since IS NULL AND s.enabled = 1"
        );
        sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(id)
            .fetch_optional(&self.db.read)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .ok_or_else(|| AppError::not_found(format!("channel {id} not found")))
    }

    /// The channel's source: its connection limit and user agent.
    pub(crate) async fn source_of(&self, channel: &ChannelRow) -> Result<(String, i64, String), AppError> {
        let s = self.source_row(channel.source_id).await?;
        let agent = channel.user_agent.clone().filter(|a| !a.is_empty()).unwrap_or(s.user_agent);
        Ok((s.name, s.max_connections, agent))
    }

    async fn favorites(&self, user_id: i64) -> Result<HashSet<i64>, AppError> {
        Ok(sqlx::query_scalar::<_, i64>("SELECT channel_id FROM live_favorites WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&self.db.read)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
            .into_iter()
            .collect())
    }

    /// Programmes between `from` and `to` of `channels`, by channel.
    async fn programs_of(
        &self,
        channels: &[ChannelRow],
        from: i64,
        to: i64,
    ) -> Result<HashMap<i64, Vec<Program>>, AppError> {
        let rows: Vec<ProgramRow> =
            sqlx::query_as("SELECT * FROM live_programs WHERE stop > ? AND start < ? ORDER BY start")
                .bind(from)
                .bind(to)
                .fetch_all(&self.db.read)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        let mut by_key: HashMap<(i64, &str), Vec<&ProgramRow>> = HashMap::new();
        for r in &rows {
            by_key.entry((r.source_id, r.guide_id.as_str())).or_default().push(r);
        }
        let mut out = HashMap::new();
        for c in channels {
            if let Some(list) = c.guide_key().and_then(|k| by_key.get(&(c.source_id, k))) {
                out.insert(c.id, list.iter().map(|r| r.for_channel(c.id)).collect());
            }
        }
        Ok(out)
    }

    fn channel_out(c: &ChannelRow, favorites: &HashSet<i64>, programs: Option<&Vec<Program>>) -> Channel {
        let at = now();
        let mut upcoming = programs.into_iter().flatten().filter(|p| p.stop > at);
        let first = upcoming.next().cloned();
        let (now_on, next) = match first {
            Some(p) if p.start <= at => (Some(p), upcoming.next().cloned()),
            Some(p) => (None, Some(p)),
            None => (None, None),
        };
        Channel {
            id: c.id,
            source_id: c.source_id,
            name: c.display_name().to_owned(),
            number: c.display_number(),
            group: c.group_name.clone(),
            logo: (!c.logo_url.is_empty()).then(|| format!("/api/live/channels/{}/logo", c.id)),
            favorite: favorites.contains(&c.id),
            has_guide: programs.is_some_and(|p| !p.is_empty()),
            now: now_on,
            next,
        }
    }

    /// Every channel `user` sees, with what's on now and next.
    pub async fn channels(&self, user: &AuthUser) -> Result<Vec<Channel>, AppError> {
        let rows = self.channel_rows(None, false).await?;
        let favorites = self.favorites(user.id).await?;
        let programs = self.programs_of(&rows, now(), now() + 12 * 3600).await?;
        Ok(rows.iter().map(|c| Self::channel_out(c, &favorites, programs.get(&c.id))).collect())
    }

    pub async fn channel(&self, user: &AuthUser, id: i64) -> Result<Channel, AppError> {
        let row = self.channel_row(id).await?;
        if row.hidden {
            return Err(AppError::not_found(format!("channel {id} not found")));
        }
        let favorites = self.favorites(user.id).await?;
        let programs = self.programs_of(std::slice::from_ref(&row), now(), now() + 12 * 3600).await?;
        Ok(Self::channel_out(&row, &favorites, programs.get(&row.id)))
    }

    /// The channels `user` watched last, newest first.
    pub async fn recent(&self, user: &AuthUser, limit: i64) -> Result<Vec<i64>, AppError> {
        sqlx::query_scalar(
            "SELECT w.channel_id FROM live_watched w JOIN live_channels c ON c.id = w.channel_id \
             WHERE w.user_id = ? AND c.hidden = 0 AND c.missing_since IS NULL ORDER BY w.watched_at DESC LIMIT ?",
        )
        .bind(user.id)
        .bind(limit)
        .fetch_all(&self.db.read)
        .await
        .map_err(|e| AppError::Internal(e.into()))
    }

    pub(crate) async fn watched(&self, user_id: i64, channel_id: i64) {
        let _ = sqlx::query(
            "INSERT INTO live_watched (user_id, channel_id, watched_at) VALUES (?, ?, ?) \
             ON CONFLICT(user_id, channel_id) DO UPDATE SET watched_at = excluded.watched_at",
        )
        .bind(user_id)
        .bind(channel_id)
        .bind(now())
        .execute(&self.db.write)
        .await;
    }

    pub async fn set_favorite(&self, user: &AuthUser, id: i64, favorite: bool) -> Result<(), AppError> {
        self.channel_row(id).await?;
        let query = if favorite {
            sqlx::query("INSERT OR IGNORE INTO live_favorites (user_id, channel_id, added_at) VALUES (?, ?, ?)")
                .bind(user.id)
                .bind(id)
                .bind(now())
        } else {
            sqlx::query("DELETE FROM live_favorites WHERE user_id = ? AND channel_id = ?").bind(user.id).bind(id)
        };
        query.execute(&self.db.write).await.map_err(|e| AppError::Internal(e.into()))?;
        Ok(())
    }

    /// The guide from `from` to `to` (at most two days) for every channel `user` sees.
    pub async fn guide(&self, _user: &AuthUser, from: i64, to: i64) -> Result<Guide, AppError> {
        if to <= from || to - from > 2 * 86_400 {
            return Err(AppError::bad_request("the guide is asked for two days at most, `to` after `from`"));
        }
        let rows = self.channel_rows(None, false).await?;
        let programs = self.programs_of(&rows, from, to).await?;
        Ok(Guide { from, to, programs })
    }

    pub async fn admin_channels(&self, source: Option<i64>) -> Result<Vec<AdminChannel>, AppError> {
        let rows = self.channel_rows(source, true).await?;
        let keys: HashSet<(i64, String)> =
            sqlx::query_as::<_, (i64, String)>("SELECT DISTINCT source_id, guide_id FROM live_programs WHERE stop > ?")
                .bind(now())
                .fetch_all(&self.db.read)
                .await
                .map_err(|e| AppError::Internal(e.into()))?
                .into_iter()
                .collect();
        Ok(rows
            .into_iter()
            .map(|c| AdminChannel {
                id: c.id,
                source_id: c.source_id,
                name: c.display_name().to_owned(),
                number: c.display_number(),
                group: c.group_name.clone(),
                logo: (!c.logo_url.is_empty()).then(|| format!("/api/live/channels/{}/logo", c.id)),
                guide_id: c.guide_key().unwrap_or_default().to_owned(),
                has_guide: c.guide_key().is_some_and(|k| keys.contains(&(c.source_id, k.to_owned()))),
                hidden: c.hidden,
                source_name: c.name.clone(),
                source_number: c.number.clone(),
                source_guide_id: c.guide_id.clone(),
                matched_guide_id: c.matched_guide_id.clone(),
                custom_name: c.custom_name.clone(),
                custom_number: c.custom_number.clone(),
                custom_guide_id: c.custom_guide_id.clone(),
            })
            .collect())
    }

    /// Changes channels `ids` as `patch` says.
    pub async fn patch_channels(&self, ids: &[i64], patch: &ChannelPatch) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        let clean = |v: &Option<String>| v.as_ref().map(|s| s.trim().to_owned()).map(|s| (!s.is_empty()).then_some(s));
        let (name, number, guide) = (clean(&patch.name), clean(&patch.number), clean(&patch.guide_id));
        let mut tx = self.db.write.begin().await.map_err(|e| AppError::Internal(e.into()))?;
        for id in ids {
            let mut sets = Vec::new();
            if patch.hidden.is_some() {
                sets.push("hidden = ?");
            }
            if name.is_some() {
                sets.push("custom_name = ?");
            }
            if number.is_some() {
                sets.push("custom_number = ?");
            }
            if guide.is_some() {
                sets.push("custom_guide_id = ?");
            }
            if sets.is_empty() {
                break;
            }
            let sql = format!("UPDATE live_channels SET {} WHERE id = ?", sets.join(", "));
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
            if let Some(hidden) = patch.hidden {
                q = q.bind(hidden);
            }
            for value in [&name, &number, &guide].into_iter().flatten() {
                q = q.bind(value.clone());
            }
            q.bind(id).execute(&mut *tx).await.map_err(|e| AppError::Internal(e.into()))?;
        }
        tx.commit().await.map_err(|e| AppError::Internal(e.into()))?;
        if patch.hidden == Some(true) {
            for id in ids {
                self.tuners.stop_channel(*id).await;
            }
        }
        self.events.send(ServerEvent::LiveChannelsChanged);
        Ok(())
    }

    // ----------------------------------------------------------- pictures

    /// A channel's logo, downloaded the first time.
    pub async fn logo(&self, id: i64) -> Result<PathBuf, AppError> {
        let row = self.channel_row(id).await?;
        if row.logo_url.is_empty() {
            return Err(AppError::not_found("the channel has no logo"));
        }
        self.picture(&row.logo_url).await
    }

    /// A programme's picture, downloaded the first time.
    pub async fn program_image(&self, channel: i64, start: i64) -> Result<PathBuf, AppError> {
        let row = self.channel_row(channel).await?;
        let key = row.guide_key().ok_or_else(|| AppError::not_found("the channel has no guide"))?;
        let url: Option<String> =
            sqlx::query_scalar("SELECT image FROM live_programs WHERE source_id = ? AND guide_id = ? AND start = ?")
                .bind(row.source_id)
                .bind(key)
                .bind(start)
                .fetch_optional(&self.db.read)
                .await
                .map_err(|e| AppError::Internal(e.into()))?
                .flatten();
        self.picture(&url.ok_or_else(|| AppError::not_found("the programme has no picture"))?).await
    }

    async fn picture(&self, url: &str) -> Result<PathBuf, AppError> {
        let hash = hex::encode(&Sha256::digest(url.as_bytes())[..16]);
        let dir = self.pictures.join(&hash[..2]);
        for ext in ["png", "jpg", "webp", "gif"] {
            let path = dir.join(format!("{hash}.{ext}"));
            if path.is_file() {
                return Ok(path);
            }
        }
        if self.failed_pictures.lock().get(url).is_some_and(|t| t.elapsed() < PICTURE_RETRY) {
            return Err(AppError::not_found("the picture can't be downloaded"));
        }
        let fetched = async {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                bail!("not a web address");
            }
            let bytes = self.download(url, "", MAX_PICTURE_BYTES).await?;
            // SVG isn't served: a picture opened from this server's address
            // must not run scripts there.
            let ext = crate::store::picture_ext(&bytes).context("not a picture")?;
            tokio::fs::create_dir_all(&dir).await?;
            let path = dir.join(format!("{hash}.{ext}"));
            let tmp = dir.join(format!(".{hash}.tmp"));
            tokio::fs::write(&tmp, &bytes).await?;
            tokio::fs::rename(&tmp, &path).await?;
            Ok::<_, anyhow::Error>(path)
        };
        match tokio::time::timeout(Duration::from_secs(20), fetched).await {
            Ok(Ok(path)) => Ok(path),
            Ok(Err(err)) => {
                tracing::debug!(url, "live TV picture not downloaded: {err:#}");
                self.failed_pictures.lock().insert(url.to_owned(), Instant::now());
                Err(AppError::not_found("the picture can't be downloaded"))
            }
            Err(_) => {
                self.failed_pictures.lock().insert(url.to_owned(), Instant::now());
                Err(AppError::not_found("the picture took too long to download"))
            }
        }
    }
}

struct Listed {
    entries: Vec<m3u::Entry>,
    guide_url: Option<String>,
    account: Option<xtream::Account>,
}

/// Channels in their order: by number where they have one (numerically),
/// then as their sources list them.
fn sort_channels(rows: &mut [ChannelRow]) {
    let key = |c: &ChannelRow| {
        let number = c.display_number().and_then(|n| n.trim().parse::<f64>().ok());
        (number.is_none(), number.unwrap_or(0.0), c.source_id, c.position)
    };
    rows.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal));
}

/// A channel name as compared with the guide's: lower case, letters and
/// digits only, and a trailing quality said or not (`HD`, `FHD`, `4K`).
fn normalize(name: &str) -> String {
    let mut s: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    for suffix in ["uhd", "fhd", "hd", "sd", "4k"] {
        if s.len() > suffix.len() + 1 && s.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }
    s
}

/// `text` with `secret` (a password in a URL) blanked out.
fn redact(text: &str, secret: &str) -> String {
    if secret.len() < 2 { text.to_owned() } else { text.replace(secret, "••••") }
}

/// What went wrong with a request, in words.
fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "the provider didn't answer in time".into()
    } else if e.is_connect() {
        format!("the provider can't be reached ({})", e.url().and_then(|u| u.host_str()).unwrap_or("?"))
    } else {
        // Without the URL, which may carry a password.
        let text = e.to_string();
        match e.url() {
            Some(url) => text.replace(&format!(" ({url})"), "").replace(url.as_str(), "…"),
            None => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_guide_whatever_their_quality() {
        assert_eq!(normalize("BBC One HD"), normalize("bbc one"));
        assert_eq!(normalize("Sky Sports F1 FHD"), "skysportsf1");
        assert_eq!(normalize("HD"), "hd");
        assert_eq!(redact("http://x/live/me/secret/1.ts", "secret"), "http://x/live/me/••••/1.ts");
    }

    #[test]
    fn settings_are_checked() {
        assert!(LiveSettings::default().check().is_ok());
        assert!(LiveSettings { timeshift_minutes: 0, ..Default::default() }.check().is_err());
        assert!(LiveSettings { max_height: Some(100), ..Default::default() }.check().is_err());
    }
}
