//! Runtime settings editable from the web UI, persisted as JSON in the
//! `settings` table and cached in memory.

use std::sync::Arc;

use parking_lot::RwLock;
use pley_transcoder_api::{HwAccel, TonemapMode, TranscodeOptions};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::AppError;

const KEY: &str = "server";

/// Where TV show metadata comes from.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TvMetadata {
    /// TMDB when it has a key, else TheTVDB when it has one, else TVmaze.
    #[default]
    Auto,
    Tmdb,
    Tvdb,
    /// No key needed.
    Tvmaze,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub server_name: String,
    pub tmdb_api_key: String,
    pub metadata_language: String,
    pub tv_metadata: TvMetadata,
    pub tvdb_api_key: String,
    /// Subscriber PIN, for TheTVDB keys that need one.
    pub tvdb_pin: String,
    /// Look artists and albums up on MusicBrainz (covers from the Cover Art Archive).
    pub music_metadata: bool,
    pub hwaccel: HwAccel,
    pub hardware_decoding: bool,
    pub tonemapping: TonemapMode,
    pub max_transcodes: u32,
    pub transcode_preset: String,
    pub throttle_transcodes: bool,
    pub scan_interval_minutes: u32,
    pub watch_libraries: bool,
    /// Find intros and credits (Skip Intro / Skip Credits) in the background.
    pub detect_markers: bool,
    /// Make seek-bar preview thumbnails in the background.
    pub seek_previews: bool,
    /// Make them for whole libraries after scans, not only when a title is opened.
    pub previews_in_background: bool,
    /// Where people reach pley (`https://pley.example.com`): single sign-on
    /// runs there.
    pub public_url: String,
    /// Sign-in with an OpenID Connect provider (see [`crate::sso`]).
    pub sso_enabled: bool,
    /// The provider's name, on the sign-in button.
    pub sso_name: String,
    /// The provider's issuer URL (its discovery document is below it).
    pub sso_issuer: String,
    /// Empty: pley's client metadata document is its client id (CIMD), for
    /// providers that take those.
    pub sso_client_id: String,
    /// Never sent back by the API (see [`Settings::for_admins`]).
    pub sso_client_secret: String,
    /// People the provider knows but pley doesn't get an account, which
    /// starts without libraries until an administrator gives it some.
    pub sso_new_accounts: bool,
    /// Only members of this group at the provider may sign in (the ID
    /// token's `groups` claim); empty: everyone the provider lets through.
    pub sso_group: String,
}

impl Default for Settings {
    fn default() -> Self {
        let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
        Self {
            server_name: if host.is_empty() { "pley".into() } else { host },
            tmdb_api_key: String::new(),
            metadata_language: "en-US".into(),
            tv_metadata: TvMetadata::Auto,
            tvdb_api_key: String::new(),
            tvdb_pin: String::new(),
            music_metadata: true,
            hwaccel: HwAccel::Auto,
            hardware_decoding: true,
            tonemapping: TonemapMode::Auto,
            max_transcodes: 4,
            // Chosen by the transcoder from its load and lead (crate::transcoder).
            transcode_preset: "auto".into(),
            throttle_transcodes: true,
            scan_interval_minutes: 360,
            watch_libraries: true,
            detect_markers: true,
            seek_previews: true,
            previews_in_background: true,
            public_url: String::new(),
            sso_enabled: false,
            sso_name: String::new(),
            sso_issuer: String::new(),
            sso_client_id: String::new(),
            sso_client_secret: String::new(),
            sso_new_accounts: false,
            sso_group: String::new(),
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

    /// The settings as administrators see them: the client secret stays on
    /// the server (only whether there is one is shown).
    pub fn for_admins(&self) -> serde_json::Value {
        let mut json = serde_json::to_value(self).unwrap_or_default();
        json["sso_client_secret"] = serde_json::Value::String(String::new());
        json["sso_client_secret_set"] = serde_json::Value::Bool(!self.sso_client_secret.is_empty());
        json
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SettingsPatch {
    pub server_name: Option<String>,
    pub tmdb_api_key: Option<String>,
    pub metadata_language: Option<String>,
    pub tv_metadata: Option<TvMetadata>,
    pub tvdb_api_key: Option<String>,
    pub tvdb_pin: Option<String>,
    pub music_metadata: Option<bool>,
    pub hwaccel: Option<HwAccel>,
    pub hardware_decoding: Option<bool>,
    pub tonemapping: Option<TonemapMode>,
    pub max_transcodes: Option<u32>,
    pub transcode_preset: Option<String>,
    pub throttle_transcodes: Option<bool>,
    pub scan_interval_minutes: Option<u32>,
    pub watch_libraries: Option<bool>,
    pub detect_markers: Option<bool>,
    pub seek_previews: Option<bool>,
    pub previews_in_background: Option<bool>,
    pub public_url: Option<String>,
    pub sso_enabled: Option<bool>,
    pub sso_name: Option<String>,
    pub sso_issuer: Option<String>,
    pub sso_client_id: Option<String>,
    pub sso_client_secret: Option<String>,
    pub sso_new_accounts: Option<bool>,
    pub sso_group: Option<String>,
}

/// `https://pley.example.com/` → `https://pley.example.com`: an http(s)
/// origin, or `None` when `value` isn't one.
fn web_origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || url.path() != "/" {
        return None;
    }
    if url.query().is_some() || url.fragment().is_some() || !url.username().is_empty() {
        return None;
    }
    Some(url.origin().ascii_serialization())
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
        if let Some(v) = patch.tv_metadata {
            next.tv_metadata = v;
        }
        if let Some(v) = patch.tvdb_api_key {
            next.tvdb_api_key = v.trim().to_owned();
        }
        if let Some(v) = patch.tvdb_pin {
            next.tvdb_pin = v.trim().to_owned();
        }
        if let Some(v) = patch.music_metadata {
            next.music_metadata = v;
        }
        match next.tv_metadata {
            TvMetadata::Tmdb if next.tmdb_api_key.is_empty() => {
                return Err(AppError::bad_request("TMDB needs an API key to be the TV source"));
            }
            TvMetadata::Tvdb if next.tvdb_api_key.is_empty() => {
                return Err(AppError::bad_request("TheTVDB needs an API key to be the TV source"));
            }
            _ => {}
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
            if !matches!(v.as_str(), "auto" | "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium") {
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
        if let Some(v) = patch.detect_markers {
            next.detect_markers = v;
        }
        if let Some(v) = patch.seek_previews {
            next.seek_previews = v;
        }
        if let Some(v) = patch.previews_in_background {
            next.previews_in_background = v;
        }
        if let Some(v) = patch.public_url {
            let v = v.trim().trim_end_matches('/');
            next.public_url = if v.is_empty() {
                String::new()
            } else {
                web_origin(v).ok_or_else(|| {
                    AppError::bad_request("the public address must look like https://pley.example.com")
                })?
            };
        }
        if let Some(v) = patch.sso_enabled {
            next.sso_enabled = v;
        }
        if let Some(v) = patch.sso_name {
            let v = v.trim().to_owned();
            if v.chars().count() > 40 {
                return Err(AppError::bad_request("the provider's name must be at most 40 characters"));
            }
            next.sso_name = v;
        }
        if let Some(v) = patch.sso_issuer {
            let v = v.trim().to_owned();
            let ok = v.is_empty()
                || url::Url::parse(&v).is_ok_and(|u| {
                    matches!(u.scheme(), "http" | "https") && u.host_str().is_some() && u.query().is_none()
                });
            if !ok {
                return Err(AppError::bad_request("the issuer must look like https://auth.example.com"));
            }
            next.sso_issuer = v;
        }
        if let Some(v) = patch.sso_client_id {
            next.sso_client_id = v.trim().to_owned();
        }
        if let Some(v) = patch.sso_client_secret {
            next.sso_client_secret = v.trim().to_owned();
        }
        if let Some(v) = patch.sso_new_accounts {
            next.sso_new_accounts = v;
        }
        if let Some(v) = patch.sso_group {
            let v = v.trim().to_owned();
            if v.chars().count() > 100 {
                return Err(AppError::bad_request("the group must be at most 100 characters"));
            }
            next.sso_group = v;
        }
        if next.sso_enabled {
            if next.sso_issuer.is_empty() {
                return Err(AppError::bad_request("single sign-on needs the provider's issuer"));
            }
            // The provider sends people back to one address, which it may
            // have to know in advance: never one guessed from a request.
            if next.public_url.is_empty() {
                return Err(AppError::bad_request("single sign-on needs pley's public address"));
            }
            // Without a registered client, the provider reads pley's client
            // metadata document, which only works at a public https address.
            if next.sso_client_id.is_empty() && !next.public_url.starts_with("https://") {
                return Err(AppError::bad_request(
                    "without a client ID, single sign-on needs pley's public https address",
                ));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> (tempfile::TempDir, Db, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("pley.db")).await.unwrap();
        let store = SettingsStore::load(&db).await.unwrap();
        (dir, db, store)
    }

    fn patch(json: serde_json::Value) -> SettingsPatch {
        serde_json::from_value(json).unwrap()
    }

    #[tokio::test]
    async fn a_rejected_patch_changes_nothing() {
        let (_dir, db, store) = store().await;
        let before = store.get();
        let err = store
            .update(
                &db,
                patch(serde_json::json!({"max_transcodes": 2, "server_name": "Den", "transcode_preset": "slow"})),
            )
            .await;
        assert!(matches!(err, Err(AppError::BadRequest(_))));
        assert_eq!(*store.get(), *before);
        // Nor was anything saved.
        assert_eq!(*SettingsStore::load(&db).await.unwrap().get(), *before);
    }

    #[tokio::test]
    async fn values_are_checked_and_normalized() {
        let (_dir, db, store) = store().await;
        let bad = |json: serde_json::Value| {
            let (db, store) = (&db, &store);
            async move { store.update(db, patch(json)).await.is_err() }
        };
        assert!(bad(serde_json::json!({"server_name": "   "})).await);
        assert!(bad(serde_json::json!({"server_name": "x".repeat(65)})).await);
        assert!(bad(serde_json::json!({"metadata_language": "e"})).await);
        assert!(bad(serde_json::json!({"metadata_language": "en-US-x-twelve"})).await);
        assert!(bad(serde_json::json!({"max_transcodes": 0})).await);
        assert!(bad(serde_json::json!({"max_transcodes": 65})).await);
        assert!(bad(serde_json::json!({"transcode_preset": "placebo"})).await);

        let s = store
            .update(
                &db,
                patch(serde_json::json!({
                    "server_name": "  Den  ",
                    "tmdb_api_key": "  abc  ",
                    "metadata_language": " fr-FR ",
                    "max_transcodes": 64,
                    "transcode_preset": "medium",
                    "scan_interval_minutes": 1_000_000,
                    "seek_previews": false,
                })),
            )
            .await
            .unwrap();
        assert_eq!(s.server_name, "Den");
        assert_eq!(s.tmdb_api_key, "abc");
        assert_eq!(s.metadata_language, "fr-FR");
        assert_eq!((s.max_transcodes, s.transcode_preset.as_str()), (64, "medium"));
        assert_eq!(s.scan_interval_minutes, 7 * 24 * 60, "at most a week");
        assert_eq!(Settings::default().transcode_preset, "auto", "the transcoder chooses by default");
        assert!(!s.seek_previews);
        // Fields left out keep their values.
        let s = store.update(&db, patch(serde_json::json!({"watch_libraries": false}))).await.unwrap();
        assert_eq!((s.server_name.as_str(), s.max_transcodes), ("Den", 64));
        assert!(!s.seek_previews && !s.watch_libraries);
        // ...and survive a restart.
        assert_eq!(*SettingsStore::load(&db).await.unwrap().get(), *s);
    }

    #[tokio::test]
    async fn single_sign_on_needs_a_provider_and_an_address() {
        let (_dir, db, store) = store().await;
        let set = |json: serde_json::Value| store.update(&db, patch(json));
        assert!(set(serde_json::json!({"sso_enabled": true})).await.is_err(), "no issuer");
        assert!(set(serde_json::json!({"sso_issuer": "ftp://auth"})).await.is_err());
        assert!(set(serde_json::json!({"public_url": "pley.example.com"})).await.is_err());
        assert!(set(serde_json::json!({"public_url": "https://pley.example.com/library"})).await.is_err());
        // Without a client id: the client metadata document, at an https address only.
        let cimd = serde_json::json!({"sso_enabled": true, "sso_issuer": "https://auth.example.com"});
        assert!(set(cimd.clone()).await.is_err(), "no address");
        assert!(set(serde_json::json!({"public_url": "http://pley.lan:7539"})).await.is_ok());
        assert!(set(cimd.clone()).await.is_err(), "http isn't enough");
        let s = set(serde_json::json!({"public_url": " https://pley.example.com/ "})).await.unwrap();
        assert_eq!(s.public_url, "https://pley.example.com");
        let s = set(cimd).await.unwrap();
        assert!(s.sso_enabled && s.sso_client_id.is_empty());
        // A registered client works at an http address, but needs one.
        let registered = serde_json::json!({"sso_client_id": " pley ", "sso_client_secret": " s3cret "});
        let s = set(registered).await.unwrap();
        assert_eq!((s.sso_client_id.as_str(), s.sso_client_secret.as_str()), ("pley", "s3cret"));
        assert!(set(serde_json::json!({"public_url": "http://pley.lan:7539"})).await.is_ok());
        assert!(set(serde_json::json!({"public_url": ""})).await.is_err(), "no address");
        assert!(!s.sso_new_accounts && s.sso_group.is_empty(), "new accounts stay off unless asked for");
        let s = set(serde_json::json!({"sso_group": " pley-users "})).await.unwrap();
        assert_eq!(s.sso_group, "pley-users");
        // Administrators never see the secret again, only that there is one.
        let shown = s.for_admins();
        assert_eq!(
            (shown["sso_client_secret"].as_str(), shown["sso_client_secret_set"].as_bool()),
            (Some(""), Some(true))
        );
    }

    #[tokio::test]
    async fn a_tv_source_needs_its_key() {
        let (_dir, db, store) = store().await;
        let set = |json: serde_json::Value| store.update(&db, patch(json));
        assert!(set(serde_json::json!({"tv_metadata": "tmdb"})).await.is_err());
        assert!(set(serde_json::json!({"tv_metadata": "tvdb"})).await.is_err());
        assert_eq!(set(serde_json::json!({"tv_metadata": "tvmaze"})).await.unwrap().tv_metadata, TvMetadata::Tvmaze);
        // A key and the source that needs it can come in one patch.
        let s = set(serde_json::json!({"tv_metadata": "tvdb", "tvdb_api_key": "key", "tvdb_pin": " 1234 "})).await;
        let s = s.unwrap();
        assert_eq!((s.tv_metadata, s.tvdb_pin.as_str()), (TvMetadata::Tvdb, "1234"));
        // Removing the key of the source in use is refused.
        assert!(set(serde_json::json!({"tvdb_api_key": "  "})).await.is_err());
        assert_eq!(store.get().tvdb_api_key, "key");
        // Auto never needs a key.
        let s = set(serde_json::json!({"tv_metadata": "auto", "tvdb_api_key": ""})).await.unwrap();
        assert_eq!((s.tv_metadata, s.tvdb_api_key.as_str()), (TvMetadata::Auto, ""));
    }

    #[tokio::test]
    async fn stored_settings_from_older_versions_get_defaults() {
        let (_dir, db, _) = store().await;
        // Saved before the TV sources and previews existed.
        sqlx::query("INSERT INTO settings (key, value) VALUES ('server', ?)")
            .bind(r#"{"server_name":"Attic","max_transcodes":2,"hwaccel":"nvenc"}"#)
            .execute(&db.write)
            .await
            .unwrap();
        let s = SettingsStore::load(&db).await.unwrap().get();
        assert_eq!((s.server_name.as_str(), s.max_transcodes, s.hwaccel), ("Attic", 2, HwAccel::Nvenc));
        assert_eq!(s.tv_metadata, TvMetadata::Auto);
        assert!(s.music_metadata && s.seek_previews && s.previews_in_background && s.detect_markers);

        // Unreadable settings fall back to the defaults instead of failing start-up.
        sqlx::query("UPDATE settings SET value = 'not json' WHERE key = 'server'").execute(&db.write).await.unwrap();
        let s = SettingsStore::load(&db).await.unwrap().get();
        assert_eq!(s.max_transcodes, Settings::default().max_transcodes);
    }
}
