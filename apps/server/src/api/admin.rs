use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Query, State};
use pley_transcoder_api::Counters;
use serde::{Deserialize, Serialize};

use crate::auth::AdminUser;
use crate::error::{ApiResult, AppError};
use crate::network::{self, Adapter, Fixed, Listen, Listener};
use crate::playback::{TranscodeInfo, transcode_info};
use crate::providers::{Check, ProviderError};
use crate::settings::{Settings, SettingsPatch};
use crate::state::AppState;

// ------------------------------------------------------------------ settings

pub async fn get_settings(State(state): State<AppState>, _admin: AdminUser) -> Json<Settings> {
    Json(Settings::clone(&state.settings.get()))
}

pub async fn put_settings(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(patch): Json<SettingsPatch>,
) -> ApiResult<Json<Settings>> {
    let before = state.settings.get();
    let after = state.settings.update(&state.db, patch).await?;
    if before.watch_libraries != after.watch_libraries {
        state.watcher.reload().await;
    }
    // Apps by other publishers turned off: they are signed out at once.
    if before.third_party_apps && !after.third_party_apps {
        let ended = state.auth.end_grants_except_clients(crate::oauth::clients::BUILTIN_IDS).await?;
        tracing::info!("apps by other publishers were turned off: {} signed out", ended.len());
    }
    // A new key or source: fetch metadata for everything that has none yet
    // (shows are fetched again from a new TV source).
    if crate::providers::settings_changed(&state.db, &before, &after).await? {
        state.scanner.enqueue_all(false).await?;
    }
    Ok(Json(Settings::clone(&after)))
}

/// A service the settings can test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Tmdb,
    Tvdb,
    Tvmaze,
    Musicbrainz,
    Transcoder,
}

impl Service {
    fn name(self) -> &'static str {
        match self {
            Service::Tmdb => "TMDB",
            Service::Tvdb => "TheTVDB",
            Service::Tvmaze => "TVmaze",
            Service::Musicbrainz => "MusicBrainz",
            Service::Transcoder => "The transcoder",
        }
    }
}

#[derive(Deserialize)]
pub struct TestRequest {
    service: Service,
    /// A key typed in the form, maybe not saved yet; the saved one otherwise.
    #[serde(default)]
    api_key: Option<String>,
    /// TheTVDB's subscriber PIN, likewise.
    #[serde(default)]
    pin: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    ok: bool,
    message: String,
    /// How long the answer took.
    ms: u64,
}

/// How long a test waits for an answer.
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// `POST /api/settings/test`: whether a metadata service accepts a key (the
/// one typed in, before it is saved) and answers, or whether the transcoder
/// does. Each test sends one request to the service's usual address.
pub async fn test_service(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(req): Json<TestRequest>,
) -> Json<TestResult> {
    let settings = state.settings.get();
    let saved = if req.service == Service::Tvdb { &settings.tvdb_api_key } else { &settings.tmdb_api_key };
    let key = req.api_key.as_deref().unwrap_or(saved).trim().to_owned();
    let pin = req.pin.as_deref().unwrap_or(&settings.tvdb_pin).trim().to_owned();
    let name = req.service.name();
    let fail = |message: String, ms: u64| Json(TestResult { ok: false, message, ms });
    if matches!(req.service, Service::Tmdb | Service::Tvdb) && key.is_empty() {
        return fail(format!("Enter a {name} API key first."), 0);
    }

    let started = Instant::now();
    let outcome = tokio::time::timeout(TEST_TIMEOUT, async {
        match req.service {
            Service::Tmdb => state.metadata.check_tmdb(&key).await.map(|()| None),
            Service::Tvdb => state.metadata.check_provider(Check::Tvdb { key: &key, pin: &pin }).await.map(|()| None),
            Service::Tvmaze => state.metadata.check_provider(Check::Tvmaze).await.map(|()| None),
            Service::Musicbrainz => state.metadata.check_provider(Check::MusicBrainz).await.map(|()| None),
            Service::Transcoder => match state.transcoder.status().await {
                Ok(status) => Ok(Some(match status.ffmpeg_version {
                    Some(ffmpeg) => format!("pley-transcoder {} with ffmpeg {ffmpeg}", status.version),
                    None => format!("pley-transcoder {}", status.version),
                })),
                Err(crate::transcoder::TranscoderError::Unavailable(why)) => Err(ProviderError::Other(
                    anyhow::anyhow!("it isn't running{}", why.map(|w| format!(" ({w})")).unwrap_or_default()),
                )),
                Err(err) => Err(ProviderError::Other(anyhow::anyhow!("{err}"))),
            },
        }
    })
    .await;
    let ms = started.elapsed().as_millis() as u64;
    match outcome {
        Ok(Ok(detail)) => Json(TestResult {
            ok: true,
            message: match (req.service, detail) {
                (Service::Tmdb, _) => "TMDB accepted the key.".into(),
                (Service::Tvdb, _) if !pin.is_empty() => "TheTVDB accepted the key and PIN.".into(),
                (Service::Tvdb, _) => "TheTVDB accepted the key.".into(),
                (_, Some(detail)) => format!("{name} answered: {detail}."),
                (_, None) => format!("{name} answered."),
            },
            ms,
        }),
        Ok(Err(ProviderError::Unauthorized(_))) => fail(
            match req.service {
                Service::Tvdb if pin.is_empty() => {
                    "TheTVDB rejected this key. A user-supported key also needs your subscriber PIN.".into()
                }
                Service::Tvdb => "TheTVDB rejected this key and PIN.".into(),
                _ => format!(
                    "{name} rejected this key. Use the API key or the API read access token from themoviedb.org/settings/api."
                ),
            },
            ms,
        ),
        Ok(Err(ProviderError::NotFound)) => fail(format!("{name} answered, but not as expected (HTTP 404)."), ms),
        Ok(Err(ProviderError::Other(err))) => fail(format!("Couldn't reach {}: {err:#}", object(name)), ms),
        Err(_) => fail(format!("{name} didn't answer within {} seconds.", TEST_TIMEOUT.as_secs()), ms),
    }
}

/// "The transcoder" in the middle of a sentence.
fn object(name: &str) -> String {
    match name.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => name.to_owned(),
    }
}

// ------------------------------------------------------------------ network

/// Where pley listens, and could: Settings → Network.
#[derive(Debug, Serialize)]
pub struct NetworkInfo {
    #[serde(flatten)]
    listen: Listen,
    fixed: Fixed,
    listeners: Vec<Listener>,
    adapters: Vec<Adapter>,
    /// Where a change is saved.
    config_file: String,
    /// Windows Firewall decides which kinds of network reach pley.
    windows: bool,
}

async fn network_info(state: &AppState) -> ApiResult<NetworkInfo> {
    let adapters = tokio::task::spawn_blocking(network::adapters).await.map_err(anyhow::Error::from)?;
    Ok(NetworkInfo {
        listen: state.network.wanted(),
        fixed: state.network.fixed,
        listeners: state.network.listeners(),
        adapters,
        config_file: state.network.config_file.display().to_string(),
        windows: cfg!(windows),
    })
}

pub async fn get_network(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<NetworkInfo>> {
    Ok(Json(network_info(&state).await?))
}

/// Listens as `listen` says from now on, and from the next start on.
pub async fn put_network(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Json(mut listen): Json<Listen>,
) -> ApiResult<Json<NetworkInfo>> {
    // This computer's address first: pley must listen on the first as it
    // starts, and an adapter's may be gone by then (DHCP, a VPN off).
    listen.web.sort_by_key(|addr| !addr.ip().is_loopback());
    listen.check().map_err(AppError::bad_request)?;
    let (now, fixed) = (state.network.wanted(), state.network.fixed);
    if (fixed.web && listen.web != now.web)
        || (fixed.plex && (listen.plex != now.plex || listen.plex_on_web != now.plex_on_web))
        || (fixed.discovery && listen.discovery != now.discovery)
    {
        return Err(AppError::bad_request("the command line or the environment sets that: change it there"));
    }
    if listen != now {
        let addresses =
            |list: &[std::net::SocketAddr]| list.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
        tracing::info!(
            "{} changed where pley listens: {} (the Plex-compatible API: {}; discovery {})",
            user.username,
            addresses(&listen.web),
            if listen.plex_on_web {
                "on pley's port".to_owned()
            } else if listen.plex.is_empty() {
                "off".to_owned()
            } else {
                addresses(&listen.plex)
            },
            if listen.discovery { "on" } else { "off" },
        );
        state.network.change(listen).await.map_err(|err| AppError::Unavailable(format!("{err:#}")))?;
    }
    Ok(Json(network_info(&state).await?))
}

// ------------------------------------------------------------------ system

#[derive(Serialize)]
pub struct TranscoderSummary {
    available: bool,
    managed: bool,
    url: Option<String>,
    version: Option<String>,
    commit: Option<String>,
    license: Option<String>,
    ffmpeg_version: Option<String>,
    encoders: Vec<String>,
    active_encoder: Option<String>,
    hwaccels: Vec<String>,
    tonemapping: Vec<String>,
    dolby_vision: bool,
    max_jobs: u32,
    active_jobs: u32,
    error: Option<String>,
}

pub async fn system(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<serde_json::Value>> {
    let info = state.transcoder.info();
    let settings = state.settings.get();
    let active_jobs = match &info {
        Some(_) => state.transcoder.status().await.map(|s| s.running_processes).unwrap_or(0),
        None => 0,
    };
    let transcoder = TranscoderSummary {
        available: info.is_some(),
        managed: state.transcoder.is_managed(),
        url: state.transcoder.base_url(),
        version: info.as_ref().map(|i| i.version.clone()),
        commit: info.as_ref().and_then(|i| i.commit.clone()),
        license: info.as_ref().map(|i| i.license.clone()),
        ffmpeg_version: info.as_ref().and_then(|i| i.capabilities.ffmpeg_version.clone()),
        encoders: info
            .as_ref()
            .map(|i| i.capabilities.encoders.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default(),
        active_encoder: info.as_ref().and_then(|i| {
            let caps = &i.capabilities;
            let wanted = settings.hwaccel;
            let hardware = caps.encoders.iter().find(|e| {
                e.codec == pley_transcoder_api::VideoCodec::H264
                    && e.hardware
                    && (wanted == pley_transcoder_api::HwAccel::Auto || e.accel == wanted)
            });
            let software =
                caps.encoders.iter().find(|e| e.codec == pley_transcoder_api::VideoCodec::H264 && !e.hardware);
            match wanted {
                pley_transcoder_api::HwAccel::None => software,
                _ => hardware.or(software),
            }
            .map(|e| e.name.clone())
        }),
        hwaccels: info.as_ref().map(|i| i.capabilities.hwaccels.clone()).unwrap_or_default(),
        tonemapping: info
            .as_ref()
            .map(|i| i.capabilities.tonemappers.iter().map(|t| t.as_str().to_owned()).collect())
            .unwrap_or_default(),
        dolby_vision: info.as_ref().is_some_and(|i| i.capabilities.dolby_vision),
        max_jobs: info.as_ref().map(|i| i.limits.max_jobs).unwrap_or(0),
        active_jobs,
        error: state.transcoder.last_error(),
    };

    let count = |sql: &'static str| {
        let db = state.db.clone();
        async move { sqlx::query_scalar::<_, i64>(sql).fetch_one(&db.read).await }
    };
    let counts = serde_json::json!({
        "libraries": count("SELECT COUNT(*) FROM libraries").await?,
        "movies": count("SELECT COUNT(*) FROM items WHERE kind = 'movie'").await?,
        "shows": count("SELECT COUNT(*) FROM items WHERE kind = 'show'").await?,
        "episodes": count("SELECT COUNT(*) FROM items WHERE kind = 'episode'").await?,
        "artists": count("SELECT COUNT(*) FROM items WHERE kind = 'artist'").await?,
        "albums": count("SELECT COUNT(*) FROM items WHERE kind = 'album'").await?,
        "tracks": count("SELECT COUNT(*) FROM items WHERE kind = 'track'").await?,
        "files": count("SELECT COUNT(*) FROM media_files").await?,
        "users": count("SELECT COUNT(*) FROM users").await?,
    });
    Ok(Json(serde_json::json!({
        "name": settings.server_name,
        "version": crate::VERSION,
        "commit": crate::COMMIT,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "uptime_secs": state.started_at.elapsed().as_secs(),
        "data_dir": state.config.data_dir.to_string_lossy(),
        "database": state.backups.health(),
        "transcoder": transcoder,
        "counts": counts,
    })))
}

#[derive(Serialize)]
pub struct TranscoderJob {
    id: String,
    label: Option<String>,
    owner: Option<String>,
    state: pley_transcoder_api::JobState,
    created_at_ms: i64,
    last_access_ms: i64,
    info: TranscodeInfo,
    restarts: u32,
    served_bytes: u64,
    warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct TranscoderStatusDto {
    available: bool,
    version: Option<String>,
    uptime_secs: u64,
    max_jobs: u32,
    running_processes: u32,
    hw_processes: u32,
    max_hw_jobs: u32,
    work_dir_bytes: u64,
    counters: Counters,
    jobs: Vec<TranscoderJob>,
}

#[derive(Serialize)]
pub struct BackupsDto {
    health: crate::backup::Health,
    backups: Vec<crate::backup::BackupInfo>,
}

/// The database's health and its backups, newest first.
pub async fn backups(State(state): State<AppState>, _admin: AdminUser) -> Json<BackupsDto> {
    Json(BackupsDto { health: state.backups.health(), backups: state.backups.list().await })
}

/// Back the database up now.
pub async fn create_backup(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ApiResult<Json<crate::backup::BackupInfo>> {
    Ok(Json(state.backups.create().await.map_err(AppError::Internal)?))
}

#[derive(Deserialize)]
pub struct ResourcesQuery {
    /// Only the samples taken after this (Unix milliseconds).
    since: Option<i64>,
}

/// How busy the machine's processors and memory are, and how much of them
/// pley takes: the last five minutes, or what came after `since`.
pub async fn resources(
    State(state): State<AppState>,
    _admin: AdminUser,
    Query(query): Query<ResourcesQuery>,
) -> Json<crate::resources::Resources> {
    Json(state.resources.resources(query.since))
}

/// Space used by stored pictures, generated media files and the cache.
pub async fn storage(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<crate::store::Usage>> {
    Ok(Json(state.store.usage().await.map_err(AppError::Internal)?))
}

/// Remove pictures and generated files nothing uses any more.
pub async fn clean_storage(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ApiResult<Json<crate::store::CleanReport>> {
    Ok(Json(state.store.clean().await.map_err(AppError::Internal)?))
}

pub async fn transcoder_status(State(state): State<AppState>, _admin: AdminUser) -> Json<TranscoderStatusDto> {
    match state.transcoder.status().await {
        Ok(status) => Json(TranscoderStatusDto {
            available: true,
            version: Some(status.version.clone()),
            uptime_secs: status.uptime_secs,
            max_jobs: status.max_jobs,
            running_processes: status.running_processes,
            hw_processes: status.hw_processes,
            max_hw_jobs: status.max_hw_jobs,
            work_dir_bytes: status.work_dir_bytes,
            counters: status.counters.clone(),
            jobs: status
                .jobs
                .iter()
                .map(|j| TranscoderJob {
                    id: j.id.clone(),
                    label: j.label.clone(),
                    owner: j.owner.clone(),
                    state: j.state,
                    created_at_ms: j.created_at_ms,
                    last_access_ms: j.last_access_ms,
                    info: transcode_info(j),
                    restarts: j.restarts,
                    served_bytes: j.served_bytes,
                    warnings: j.warnings.clone(),
                })
                .collect(),
        }),
        Err(_) => Json(TranscoderStatusDto {
            available: false,
            version: None,
            uptime_secs: 0,
            max_jobs: 0,
            running_processes: 0,
            hw_processes: 0,
            max_hw_jobs: 0,
            work_dir_bytes: 0,
            counters: Counters::default(),
            jobs: Vec::new(),
        }),
    }
}

// ------------------------------------------------------------------ folder browser

#[derive(Serialize)]
pub struct DirEntry {
    name: String,
    path: String,
}

#[derive(Serialize)]
pub struct Listing {
    path: Option<String>,
    parent: Option<String>,
    entries: Vec<DirEntry>,
}

fn roots() -> Vec<DirEntry> {
    let mut out = Vec::new();
    if cfg!(windows) {
        for letter in b'A'..=b'Z' {
            let root = format!("{}:\\", letter as char);
            if Path::new(&root).exists() {
                out.push(DirEntry { name: format!("{}:", letter as char), path: root });
            }
        }
    } else {
        out.push(DirEntry { name: "/".into(), path: "/".into() });
        for candidate in ["/mnt", "/media", "/srv", "/Volumes"] {
            if Path::new(candidate).is_dir() {
                out.push(DirEntry { name: candidate.into(), path: candidate.into() });
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        out.push(DirEntry { name: "Home".into(), path: home.to_string_lossy().into_owned() });
    }
    out
}

pub async fn browse_fs(_admin: AdminUser, Query(query): Query<HashMap<String, String>>) -> ApiResult<Json<Listing>> {
    let Some(raw) = query.get("path").map(|p| p.trim()).filter(|p| !p.is_empty()) else {
        return Ok(Json(Listing { path: None, parent: None, entries: roots() }));
    };
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::bad_request("path must be absolute"));
    }
    let listing = crate::util::blocking(move || {
        let mut entries = Vec::new();
        let read = std::fs::read_dir(&path).map_err(|e| anyhow::anyhow!("cannot open {}: {e}", path.display()))?;
        for entry in read.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let is_dir = kind.is_dir() || (kind.is_symlink() && entry.path().is_dir());
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_dir || name.starts_with('.') || name.starts_with('$') || name == "System Volume Information" {
                continue;
            }
            entries.push(DirEntry { path: entry.path().to_string_lossy().into_owned(), name });
        }
        entries.sort_by_key(|e| e.name.to_lowercase());
        Ok(Listing {
            parent: path.parent().map(|p| p.to_string_lossy().into_owned()),
            path: Some(path.to_string_lossy().into_owned()),
            entries,
        })
    })
    .await
    .map_err(|e| AppError::bad_request(e.to_string()))?;
    Ok(Json(listing))
}
