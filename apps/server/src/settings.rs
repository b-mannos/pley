//! Runtime settings editable from the web UI, persisted as JSON in the
//! `settings` table and cached in memory.

use std::sync::Arc;

use parking_lot::RwLock;
use pley_transcoder_api::{HwAccel, TonemapMode, TranscodeOptions};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::AppError;

const KEY: &str = "server";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub server_name: String,
    pub tmdb_api_key: String,
    pub metadata_language: String,
    pub hwaccel: HwAccel,
    pub hardware_decoding: bool,
    pub tonemapping: TonemapMode,
    pub max_transcodes: u32,
    pub transcode_preset: String,
    pub throttle_transcodes: bool,
    pub scan_interval_minutes: u32,
    pub watch_libraries: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
        Self {
            server_name: if host.is_empty() { "pley".into() } else { host },
            tmdb_api_key: String::new(),
            metadata_language: "en-US".into(),
            hwaccel: HwAccel::Auto,
            hardware_decoding: true,
            tonemapping: TonemapMode::Auto,
            max_transcodes: 4,
            transcode_preset: "veryfast".into(),
            throttle_transcodes: true,
            scan_interval_minutes: 360,
            watch_libraries: true,
        }
    }
}

impl Settings {
    pub fn transcode_options(&self) -> TranscodeOptions {
        TranscodeOptions {
            hwaccel: self.hwaccel,
            hardware_decoding: self.hardware_decoding,
            tonemap: self.tonemapping,
            preset: Some(self.transcode_preset.clone()),
            throttle: self.throttle_transcodes,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SettingsPatch {
    pub server_name: Option<String>,
    pub tmdb_api_key: Option<String>,
    pub metadata_language: Option<String>,
    pub hwaccel: Option<HwAccel>,
    pub hardware_decoding: Option<bool>,
    pub tonemapping: Option<TonemapMode>,
    pub max_transcodes: Option<u32>,
    pub transcode_preset: Option<String>,
    pub throttle_transcodes: Option<bool>,
    pub scan_interval_minutes: Option<u32>,
    pub watch_libraries: Option<bool>,
}

pub struct SettingsStore {
    current: RwLock<Arc<Settings>>,
}

impl SettingsStore {
    pub async fn load(db: &Db) -> anyhow::Result<Self> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = ?").bind(KEY).fetch_optional(&db.read).await?;
        let settings = stored.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
        Ok(Self { current: RwLock::new(Arc::new(settings)) })
    }

    pub fn get(&self) -> Arc<Settings> {
        self.current.read().clone()
    }

    pub async fn update(&self, db: &Db, patch: SettingsPatch) -> Result<Arc<Settings>, AppError> {
        let mut next = (*self.get()).clone();
        if let Some(v) = patch.server_name {
            let v = v.trim().to_owned();
            if v.is_empty() || v.chars().count() > 64 {
                return Err(AppError::bad_request("server name must be 1-64 characters"));
            }
            next.server_name = v;
        }
        if let Some(v) = patch.tmdb_api_key {
            next.tmdb_api_key = v.trim().to_owned();
        }
        if let Some(v) = patch.metadata_language {
            let v = v.trim().to_owned();
            if !(2..=10).contains(&v.len()) {
                return Err(AppError::bad_request("metadata language must look like 'en-US'"));
            }
            next.metadata_language = v;
        }
        if let Some(v) = patch.hwaccel {
            next.hwaccel = v;
        }
        if let Some(v) = patch.hardware_decoding {
            next.hardware_decoding = v;
        }
        if let Some(v) = patch.tonemapping {
            next.tonemapping = v;
        }
        if let Some(v) = patch.max_transcodes {
            if !(1..=64).contains(&v) {
                return Err(AppError::bad_request("max transcodes must be between 1 and 64"));
            }
            next.max_transcodes = v;
        }
        if let Some(v) = patch.transcode_preset {
            if !matches!(v.as_str(), "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium") {
                return Err(AppError::bad_request("unknown transcode preset"));
            }
            next.transcode_preset = v;
        }
        if let Some(v) = patch.throttle_transcodes {
            next.throttle_transcodes = v;
        }
        if let Some(v) = patch.scan_interval_minutes {
            next.scan_interval_minutes = v.min(7 * 24 * 60);
        }
        if let Some(v) = patch.watch_libraries {
            next.watch_libraries = v;
        }
        let json = serde_json::to_string(&next).map_err(|e| AppError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(KEY)
        .bind(json)
        .execute(&db.write)
        .await?;
        let next = Arc::new(next);
        *self.current.write() = next.clone();
        Ok(next)
    }
}
