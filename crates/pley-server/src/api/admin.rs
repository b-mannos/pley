use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use pley_transcoder_api::Counters;
use serde::{Deserialize, Serialize};

use super::auth::UserDto;
use crate::auth::{AdminUser, hash_password, validate_password, validate_username};
use crate::error::{ApiResult, AppError};
use crate::playback::{TranscodeInfo, transcode_info};
use crate::settings::{Settings, SettingsPatch};
use crate::state::AppState;
use crate::util::now;

// ------------------------------------------------------------------ users

pub async fn list_users(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Vec<UserDto>>> {
    let users = sqlx::query_as("SELECT id, username, is_admin, created_at FROM users ORDER BY username COLLATE NOCASE")
        .fetch_all(&state.db.read)
        .await?;
    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    #[serde(default)]
    is_admin: bool,
}

pub async fn create_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<NewUser>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    let username = body.username.trim().to_owned();
    validate_username(&username)?;
    validate_password(&body.password)?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&state.db.read)
        .await?;
    if exists.is_some() {
        return Err(AppError::conflict(format!("user '{username}' already exists")));
    }
    let hash = hash_password(body.password).await?;
    let user: UserDto = sqlx::query_as(
        "INSERT INTO users (username, password_hash, is_admin, created_at) VALUES (?, ?, ?, ?)
         RETURNING id, username, is_admin, created_at",
    )
    .bind(&username)
    .bind(hash)
    .bind(body.is_admin)
    .bind(now())
    .fetch_one(&state.db.write)
    .await?;
    Ok((StatusCode::CREATED, Json(user)))
}

#[derive(Deserialize)]
pub struct UserPatch {
    password: Option<String>,
    is_admin: Option<bool>,
}

pub async fn update_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    UrlPath(id): UrlPath<i64>,
    Json(body): Json<UserPatch>,
) -> ApiResult<Json<UserDto>> {
    if let Some(password) = body.password {
        validate_password(&password)?;
        let hash = hash_password(password).await?;
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&state.db.write)
            .await?;
        // Sign the user out everywhere else.
        if id != admin.id {
            sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&state.db.write).await?;
        }
    }
    if let Some(is_admin) = body.is_admin {
        if id == admin.id && !is_admin {
            return Err(AppError::bad_request("you cannot remove your own administrator rights"));
        }
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(is_admin)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    state.auth.forget_user(id);
    let user = sqlx::query_as("SELECT id, username, is_admin, created_at FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db.read)
        .await?
        .ok_or_else(|| AppError::not_found("user not found"))?;
    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    UrlPath(id): UrlPath<i64>,
) -> ApiResult<StatusCode> {
    if id == admin.id {
        return Err(AppError::bad_request("you cannot delete your own account"));
    }
    let result = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&state.db.write).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("user not found"));
    }
    state.auth.forget_user(id);
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ settings

pub async fn get_settings(State(state): State<AppState>, _admin: AdminUser) -> Json<Settings> {
    Json((*state.settings.get()).clone())
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
    if before.tmdb_api_key != after.tmdb_api_key && !after.tmdb_api_key.is_empty() {
        // New key: fetch metadata for everything that has none yet.
        state.scanner.enqueue_all(false).await?;
    }
    Ok(Json((*after).clone()))
}

// ------------------------------------------------------------------ system

#[derive(Serialize)]
pub struct TranscoderSummary {
    available: bool,
    managed: bool,
    url: Option<String>,
    version: Option<String>,
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
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "uptime_secs": state.started_at.elapsed().as_secs(),
        "data_dir": state.config.data_dir.to_string_lossy(),
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
