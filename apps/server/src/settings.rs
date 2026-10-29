//! Runtime settings editable from the web UI, persisted as JSON in the
//! `settings` table and cached in memory.

use std::sync::Arc;

use parking_lot::RwLock;
use pley_transcoder_api::{
    Capabilities, HwAccel, TonemapMode, Tonemapper, TranscodeOptions, TranscoderInfo, VideoCodec,
};
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
    /// Keep transcodes' segments in memory where the transcoder can (a
    /// folder in RAM with room: Linux's `/dev/shm`), else on disk.
    pub transcode_in_memory: bool,
    pub scan_interval_minutes: u32,
    pub watch_libraries: bool,
    /// Find intros and credits (Skip Intro / Skip Credits) in the background.
    pub detect_markers: bool,
    /// Make seek-bar preview thumbnails in the background.
    pub seek_previews: bool,
    /// Make them for whole libraries after scans, not only when a title is opened.
    pub previews_in_background: bool,
    /// Apps by other publishers may sign in (their client metadata documents
    /// are read from the internet: see [`crate::oauth`]). pley's own apps
    /// always may. Off by default; turning it off signs them out.
    pub third_party_apps: bool,
    /// Addresses with a port that pley is reached at through a reverse
    /// proxy which leaves the port out of `Host` (nginx's `$host`, Nginx
    /// Proxy Manager's default): `https://media.example.com:9999`. A request
    /// whose `Host` has no port gets the port of the one with its scheme
    /// and host name back, so pley names itself as apps reached it.
    pub proxy_addresses: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
        Self {
            server_name: if host.is_empty() { "Pley".into() } else { host },
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
            transcode_in_memory: true,
            scan_interval_minutes: 360,
            watch_libraries: true,
            detect_markers: true,
            seek_previews: true,
            previews_in_background: true,
            third_party_apps: false,
            proxy_addresses: Vec::new(),
        }
    }
}

/// Proxy addresses kept at most.
const MAX_PROXY_ADDRESSES: usize = 16;

/// A proxy address as kept (`https://media.example.com:9999`): http or
/// https, a host and a port that isn't the scheme's own (without one,
/// `Host` names it right already), nothing else.
fn proxy_address(value: &str) -> Result<String, AppError> {
    let wrong = || AppError::bad_request(format!("{value} isn't an address like https://media.example.com:9999"));
    let url = url::Url::parse(value.trim()).map_err(|_| wrong())?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none();
    if !plain {
        return Err(wrong());
    }
    if url.port().is_none() {
        return Err(AppError::bad_request(format!(
            "{value} has no port of its own: Pley needs nothing to be reached there"
        )));
    }
    Ok(url.origin().ascii_serialization())
}

impl Settings {
    pub fn transcode_options(&self) -> TranscodeOptions {
        TranscodeOptions {
            hwaccel: self.hwaccel,
            hardware_decoding: self.hardware_decoding,
            tonemap: self.tonemapping,
            preset: Some(self.transcode_preset.clone()),
            throttle: self.throttle_transcodes,
            in_memory: self.transcode_in_memory,
        }
    }
}

/// The name Settings shows for a hardware acceleration.
fn accel_name(accel: HwAccel) -> &'static str {
    match accel {
        HwAccel::Auto => "Automatic",
        HwAccel::None => "Software encoding",
        HwAccel::Nvenc => "NVIDIA NVENC",
        HwAccel::Qsv => "Intel Quick Sync",
        HwAccel::Amf => "AMD AMF",
        HwAccel::Vaapi => "VAAPI",
        HwAccel::Videotoolbox => "VideoToolbox",
        HwAccel::Vulkan => "Vulkan",
    }
}

/// Why `hwaccel` can't work here, or `None` when it can (or nobody can
/// tell). `os` is the transcoder's operating system when known (a managed
/// transcoder runs on pley's), `caps` what it found. Without its encoder,
/// the transcoder encodes in software instead.
pub fn hwaccel_problem(hwaccel: HwAccel, os: Option<&str>, caps: Option<&Capabilities>) -> Option<String> {
    let name = accel_name(hwaccel);
    match (hwaccel, os) {
        (HwAccel::Auto | HwAccel::None, _) => return None,
        (HwAccel::Vaapi, Some(os)) if os != "linux" => return Some(format!("{name} works on Linux only")),
        (HwAccel::Videotoolbox, Some(os)) if os != "macos" => return Some(format!("{name} works on macOS only")),
        (HwAccel::Nvenc | HwAccel::Qsv | HwAccel::Amf, Some("macos")) => {
            return Some(format!("{name} doesn't work on macOS"));
        }
        _ => {}
    }
    // An FFmpeg that isn't there can't say what the GPU could do.
    let caps = caps.filter(|caps| caps.ffmpeg_available)?;
    let works = match hwaccel {
        HwAccel::Vulkan => caps.vulkan_encode,
        accel => caps.encoders.iter().any(|e| e.hardware && e.codec == VideoCodec::H264 && e.accel == accel),
    };
    let needs = match hwaccel {
        HwAccel::Nvenc => "an NVIDIA graphics card",
        HwAccel::Qsv => "an Intel graphics chip",
        HwAccel::Amf => "an AMD graphics card",
        HwAccel::Vaapi => "an Intel or AMD graphics chip",
        HwAccel::Videotoolbox => "an FFmpeg built with VideoToolbox",
        _ => "a GPU that FFmpeg encodes on through Vulkan",
    };
    (!works).then(|| format!("{name} isn't available here: it needs {needs}, and the transcoder found none"))
}

/// Why the tone mapper `mode` names can't work here, or `None` when it can
/// (or nobody can tell). Without it, the transcoder tone maps as Automatic
/// does.
pub fn tonemap_problem(mode: TonemapMode, caps: Option<&Capabilities>) -> Option<String> {
    let wanted = match mode {
        TonemapMode::Libplacebo => Tonemapper::Libplacebo,
        TonemapMode::Opencl => Tonemapper::Opencl,
        TonemapMode::Zscale => Tonemapper::Zscale,
        TonemapMode::Auto | TonemapMode::None => return None,
    };
    let caps = caps.filter(|caps| caps.ffmpeg_available)?;
    let why = match wanted {
        Tonemapper::Libplacebo => {
            "libplacebo isn't available here: it needs an FFmpeg with libplacebo and a GPU it reaches through Vulkan"
        }
        Tonemapper::Opencl => "OpenCL tone mapping isn't available here: it needs a GPU with OpenCL drivers",
        Tonemapper::Zscale => "zscale isn't available here: this FFmpeg was built without zimg",
    };
    (!caps.tonemappers.contains(&wanted)).then(|| why.to_owned())
}

/// Why transcodes' segments can't be kept in memory here, or `None` when
/// they can (or nobody can tell: no transcoder). Without it, they go to disk.
pub fn memory_problem(info: Option<&TranscoderInfo>) -> Option<String> {
    match &info?.memory {
        Some(memory) if memory.available => None,
        Some(memory) => Some(memory.reason.clone().unwrap_or_else(|| "not available here".into())),
        None => Some("needs a newer transcoder".into()),
    }
}

/// Whether some video is decoded on a GPU here, so that
/// [`Settings::hardware_decoding`] changes something: as the transcoder's
/// planner chooses, through a GPU pipeline, Vulkan (for libplacebo) or the
/// decoder that goes with a hardware encoder.
pub fn decodes_on_gpu(caps: &Capabilities) -> bool {
    let has = |name: &str| caps.hwaccels.iter().any(|h| h == name);
    !caps.gpu_pipelines.is_empty()
        || !caps.vulkan_decode.is_empty()
        || has("videotoolbox")
        || caps.encoders.iter().filter(|e| e.hardware).any(|e| match e.accel {
            HwAccel::Nvenc => has("cuda"),
            HwAccel::Vaapi => has("vaapi"),
            HwAccel::Qsv => has("d3d11va") || has("vaapi"),
            HwAccel::Amf => has("d3d11va"),
            _ => false,
        })
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
    pub transcode_in_memory: Option<bool>,
    pub scan_interval_minutes: Option<u32>,
    pub watch_libraries: Option<bool>,
    pub detect_markers: Option<bool>,
    pub seek_previews: Option<bool>,
    pub previews_in_background: Option<bool>,
    pub third_party_apps: Option<bool>,
    pub proxy_addresses: Option<Vec<String>>,
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
        if let Some(v) = patch.transcode_in_memory {
            next.transcode_in_memory = v;
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
        if let Some(v) = patch.third_party_apps {
            next.third_party_apps = v;
        }
        if let Some(v) = patch.proxy_addresses {
            let mut addresses: Vec<String> = Vec::new();
            for value in v.iter().filter(|value| !value.trim().is_empty()) {
                let address = proxy_address(value)?;
                // One port per scheme and host: which one a request came to
                // couldn't be told otherwise.
                let site =
                    |a: &str| url::Url::parse(a).map(|u| (u.scheme().to_owned(), u.host_str().map(str::to_owned)));
                if let Some(same) = addresses.iter().find(|a| site(a).ok() == site(&address).ok()) {
                    if *same != address {
                        return Err(AppError::bad_request(format!("{same} and {address} name the same host")));
                    }
                    continue;
                }
                addresses.push(address);
            }
            if addresses.len() > MAX_PROXY_ADDRESSES {
                return Err(AppError::bad_request(format!("at most {MAX_PROXY_ADDRESSES} proxy addresses")));
            }
            next.proxy_addresses = addresses;
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

    #[tokio::test]
    async fn apps_by_other_publishers_stay_off_until_allowed() {
        let (_dir, db, store) = store().await;
        assert!(!store.get().third_party_apps, "off on a new server");
        // Saved by a version that didn't have the setting.
        sqlx::query("INSERT INTO settings (key, value) VALUES ('server', '{\"server_name\":\"Attic\"}')")
            .execute(&db.write)
            .await
            .unwrap();
        assert!(!SettingsStore::load(&db).await.unwrap().get().third_party_apps, "off after an upgrade");
        let s = store.update(&db, patch(serde_json::json!({"third_party_apps": true}))).await.unwrap();
        assert!(s.third_party_apps);
        assert!(SettingsStore::load(&db).await.unwrap().get().third_party_apps, "kept");
        // Other changes leave it alone.
        let s = store.update(&db, patch(serde_json::json!({"server_name": "Den"}))).await.unwrap();
        assert!(s.third_party_apps);
        let s = store.update(&db, patch(serde_json::json!({"third_party_apps": false}))).await.unwrap();
        assert!(!s.third_party_apps);
    }

    fn encoder(name: &str, accel: HwAccel) -> pley_transcoder_api::EncoderInfo {
        pley_transcoder_api::EncoderInfo {
            name: name.into(),
            codec: if name.starts_with("hevc") { VideoCodec::Hevc } else { VideoCodec::H264 },
            hardware: accel != HwAccel::None,
            accel,
            main10: false,
        }
    }

    /// An FFmpeg on a computer with an Intel graphics chip, on Windows.
    fn intel() -> Capabilities {
        Capabilities {
            ffmpeg_available: true,
            encoders: vec![encoder("h264_qsv", HwAccel::Qsv), encoder("libx264", HwAccel::None)],
            hwaccels: ["cuda", "qsv", "d3d11va", "vulkan"].map(String::from).to_vec(),
            tonemappers: vec![Tonemapper::Opencl, Tonemapper::Zscale],
            ..Default::default()
        }
    }

    #[test]
    fn a_hardware_acceleration_needs_its_encoder_and_system() {
        let caps = intel();
        let problem = |accel, os| hwaccel_problem(accel, os, Some(&caps));
        for fine in [HwAccel::Auto, HwAccel::None, HwAccel::Qsv] {
            assert_eq!(problem(fine, Some("windows")), None, "{fine:?}");
        }
        let nvenc = problem(HwAccel::Nvenc, Some("windows")).unwrap();
        assert!(nvenc.contains("NVIDIA graphics card"), "{nvenc}");
        assert!(problem(HwAccel::Amf, Some("windows")).unwrap().contains("AMD graphics card"));
        assert_eq!(problem(HwAccel::Vaapi, Some("windows")).unwrap(), "VAAPI works on Linux only");
        assert_eq!(problem(HwAccel::Videotoolbox, Some("linux")).unwrap(), "VideoToolbox works on macOS only");
        assert_eq!(problem(HwAccel::Qsv, Some("macos")).unwrap(), "Intel Quick Sync doesn't work on macOS");
        // A transcoder elsewhere (its own system unknown) is judged by its encoders only.
        assert!(problem(HwAccel::Vaapi, None).unwrap().contains("Intel or AMD graphics chip"));
        assert!(problem(HwAccel::Vulkan, None).is_some());
        let vulkan = Capabilities { vulkan_encode: true, ..intel() };
        assert_eq!(hwaccel_problem(HwAccel::Vulkan, None, Some(&vulkan)), None);
        // HEVC alone doesn't make H.264 transcodes: they'd be encoded in software.
        let hevc = Capabilities { encoders: vec![encoder("hevc_nvenc", HwAccel::Nvenc)], ..intel() };
        assert!(hwaccel_problem(HwAccel::Nvenc, Some("linux"), Some(&hevc)).is_some());
        // Without the transcoder or its FFmpeg, nothing is known but the system.
        assert_eq!(hwaccel_problem(HwAccel::Nvenc, Some("windows"), None), None);
        let missing = Capabilities { ffmpeg_available: false, ..Default::default() };
        assert_eq!(hwaccel_problem(HwAccel::Nvenc, Some("windows"), Some(&missing)), None);
        assert!(hwaccel_problem(HwAccel::Vaapi, Some("windows"), None).is_some());
    }

    #[test]
    fn a_tone_mapper_needs_to_work() {
        let caps = intel();
        for fine in [TonemapMode::Auto, TonemapMode::None, TonemapMode::Opencl, TonemapMode::Zscale] {
            assert_eq!(tonemap_problem(fine, Some(&caps)), None, "{fine:?}");
        }
        assert!(tonemap_problem(TonemapMode::Libplacebo, Some(&caps)).unwrap().contains("Vulkan"));
        assert_eq!(tonemap_problem(TonemapMode::Libplacebo, None), None);
        let none = Capabilities { tonemappers: vec![], ..intel() };
        assert!(tonemap_problem(TonemapMode::Zscale, Some(&none)).unwrap().contains("zimg"));
    }

    #[test]
    fn memory_needs_a_transcoder_that_has_it() {
        use pley_transcoder_api::MemoryInfo;
        assert_eq!(memory_problem(None), None, "no transcoder: nobody can tell");
        let info = |memory| TranscoderInfo { memory, ..Default::default() };
        let linux = MemoryInfo { available: true, path: Some("/dev/shm".into()), budget_bytes: 1 << 32, reason: None };
        assert_eq!(memory_problem(Some(&info(Some(linux)))), None);
        let windows = MemoryInfo { available: false, reason: Some("works on Linux only".into()), ..Default::default() };
        assert_eq!(memory_problem(Some(&info(Some(windows)))).as_deref(), Some("works on Linux only"));
        assert_eq!(memory_problem(Some(&info(None))).as_deref(), Some("needs a newer transcoder"));
        // On by default, and in every job's request.
        assert!(Settings::default().transcode_in_memory && Settings::default().transcode_options().in_memory);
        let off = Settings { transcode_in_memory: false, ..Settings::default() };
        assert!(!off.transcode_options().in_memory);
    }

    #[test]
    fn decoding_on_a_gpu_goes_with_an_encoder_or_a_pipeline() {
        assert!(decodes_on_gpu(&intel()), "Quick Sync decodes through D3D11");
        let software = Capabilities { encoders: vec![encoder("libx264", HwAccel::None)], ..intel() };
        assert!(!decodes_on_gpu(&software), "FFmpeg lists cuda, but there's no NVIDIA card");
        let vulkan = Capabilities { vulkan_decode: vec!["hevc10".into()], ..software.clone() };
        assert!(decodes_on_gpu(&vulkan));
        let mac = Capabilities { hwaccels: vec!["videotoolbox".into()], ..software };
        assert!(decodes_on_gpu(&mac));
    }

    #[tokio::test]
    async fn old_single_sign_on_keys_are_ignored() {
        let (_dir, db, _) = store().await;
        // A row as single sign-on left it (upgrading removes these keys too).
        let row = serde_json::json!({
            "server_name": "Attic",
            "public_url": "https://pley.example.com",
            "sso_enabled": true,
            "sso_issuer": "https://idp.example.com",
            "sso_client_secret": "s3cret",
        });
        sqlx::query("INSERT INTO settings (key, value) VALUES ('server', ?)")
            .bind(row.to_string())
            .execute(&db.write)
            .await
            .unwrap();
        let store = SettingsStore::load(&db).await.unwrap();
        assert_eq!(store.get().server_name, "Attic", "the rest of the row loads");

        // A web app from before still sends them.
        let old = serde_json::json!({
            "server_name": "Den",
            "public_url": "https://pley.example.com",
            "sso_issuer": "https://idp.example.com",
            "sso_client_secret": "s3cret",
        });
        let s = store.update(&db, patch(old)).await.unwrap();
        assert_eq!(s.server_name, "Den");
        let saved: String =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'server'").fetch_one(&db.read).await.unwrap();
        for gone in ["public_url", "sso_", "s3cret"] {
            assert!(!saved.contains(gone), "{gone} is still saved: {saved}");
        }
        assert_eq!(*SettingsStore::load(&db).await.unwrap().get(), *s);
    }
}
