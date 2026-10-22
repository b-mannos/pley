//! Playback decisions and sessions.
//!
//! For every play request the server compares the file with what the client
//! can decode and picks **direct play** (the raw file over HTTP range requests),
//! **remux** (HLS with the video stream copied) or **transcode** (HLS produced by
//! the transcoder). Sessions are tracked for "now playing" and resume.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use pley_transcoder_api::{
    AudioMode, AudioRequest, JobRequest, JobStatus, MediaInfo, MediaStream, StreamKind, SubtitleBurnRequest,
    VideoCodec, VideoMode, VideoRequest,
};
use serde::{Deserialize, Serialize};

use crate::catalog::FileRow;
use crate::events::{EventBus, ServerEvent};
use crate::transcoder::Transcoder;
use crate::util::now;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Caps {
    #[serde(default)]
    pub containers: Vec<String>,
    #[serde(default)]
    pub video_codecs: Vec<String>,
    #[serde(default)]
    pub audio_codecs: Vec<String>,
    /// Client can display HDR10 without tone mapping.
    #[serde(default)]
    pub hdr: bool,
}

impl Caps {
    fn container(&self, c: &str) -> bool {
        self.containers.iter().any(|x| x.eq_ignore_ascii_case(c))
    }
    fn video(&self, c: &str) -> bool {
        self.video_codecs.iter().any(|x| x.eq_ignore_ascii_case(c))
    }
    fn audio(&self, c: &str) -> bool {
        self.audio_codecs.iter().any(|x| x.eq_ignore_ascii_case(c))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaybackRequest {
    pub item_id: i64,
    #[serde(default)]
    pub file_id: Option<i64>,
    #[serde(default)]
    pub audio_index: Option<i64>,
    #[serde(default)]
    pub subtitle_index: Option<i64>,
    #[serde(default)]
    pub max_bitrate: Option<u32>,
    #[serde(default)]
    pub max_height: Option<u32>,
    #[serde(default)]
    pub start_ms: Option<i64>,
    #[serde(default)]
    pub caps: Caps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Direct,
    Remux,
    Transcode,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleTrack {
    pub index: i64,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
    pub external: bool,
    pub text_based: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackInfo {
    pub session_id: String,
    pub method: Method,
    pub url: String,
    pub hls: bool,
    pub file_id: i64,
    pub duration_ms: i64,
    pub audio_index: Option<i64>,
    pub subtitle_index: Option<i64>,
    pub burn_subtitles: bool,
    pub subtitles: Vec<SubtitleTrack>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub method: Method,
    pub video: Option<VideoRequest>,
    pub audio: Option<AudioRequest>,
    pub subtitle: Option<SubtitleBurnRequest>,
    pub audio_index: Option<i64>,
    pub burn_index: Option<i64>,
    pub reasons: Vec<String>,
}

fn range_label(stream: &MediaStream) -> String {
    let range = stream.dynamic_range();
    match (range, stream.dv_profile) {
        (pley_transcoder_api::DynamicRange::DolbyVision, Some(p)) => format!("Dolby Vision P{p}"),
        _ => range.label().to_owned(),
    }
}

/// Containers the browser can play natively (used when a file was never probed).
fn web_container(container: &str) -> bool {
    matches!(container, "mp4" | "m4v" | "webm" | "mp3" | "m4a" | "flac" | "ogg" | "wav" | "aac")
}

pub fn decide(file: &FileRow, info: Option<&MediaInfo>, streams: &[MediaStream], req: &PlaybackRequest) -> Decision {
    let caps = &req.caps;
    let container = file.container.as_deref().unwrap_or_default();
    let mut reasons = Vec::new();

    let Some(info) = info else {
        // Never probed (transcoder was unavailable during the scan).
        let direct = caps.container(container) || web_container(container);
        return Decision {
            method: if direct { Method::Direct } else { Method::Transcode },
            video: (!direct).then(VideoRequest::default),
            audio: (!direct).then(AudioRequest::default),
            subtitle: None,
            audio_index: None,
            burn_index: None,
            reasons: if direct { vec![] } else { vec!["file has not been analyzed yet".into()] },
        };
    };

    let video = info.video();
    let default_audio = info.default_audio();
    let audio = req.audio_index.and_then(|i| info.stream(i).filter(|s| s.kind == StreamKind::Audio)).or(default_audio);

    let burn_stream =
        req.subtitle_index.and_then(|i| streams.iter().find(|s| s.index == i && s.kind == StreamKind::Subtitle));
    let burn = burn_stream.filter(|s| !s.text_based.unwrap_or(true) && video.is_some());
    if let Some(s) = burn {
        reasons.push(format!("{} subtitles are images and must be burned in", s.codec));
    }

    let container_ok = caps.container(container);
    if !container_ok {
        reasons.push(format!("the {container} container is not supported by this player"));
    }
    let video_ok = match video {
        None => true,
        Some(v) => {
            let mut ok = true;
            if !caps.video(&v.codec) {
                reasons.push(format!("{} video is not supported by this player", v.codec));
                ok = false;
            }
            if v.dynamic_range().is_hdr() && !caps.hdr {
                reasons.push(format!("{} video is tone-mapped to SDR", range_label(v)));
                ok = false;
            }
            if v.codec == "h264" && v.bit_depth.unwrap_or(8) > 8 {
                reasons.push("10-bit H.264 is not supported by browsers".into());
                ok = false;
            }
            ok
        }
    };
    let audio_ok = match audio {
        None => true,
        Some(a) if caps.audio(&a.codec) => true,
        Some(a) => {
            reasons.push(format!("{} audio is not supported by this player", a.codec));
            false
        }
    };
    let audio_is_default = match (audio, default_audio) {
        (Some(a), Some(d)) => a.index == d.index,
        _ => true,
    };
    if !audio_is_default {
        reasons.push("an alternate audio track was selected".into());
    }
    let bitrate_ok = req.max_bitrate.is_none_or(|max| file.bitrate.unwrap_or(0) <= max as i64 * 1000);
    if !bitrate_ok {
        reasons.push("the file's bitrate is above the selected quality".into());
    }
    let size_ok = req.max_height.is_none_or(|max| video.and_then(|v| v.height).unwrap_or(0) <= max as i64);
    if !size_ok {
        reasons.push("the video resolution is above the selected quality".into());
    }

    let audio_index = audio.map(|a| a.index);
    if container_ok && video_ok && audio_ok && audio_is_default && bitrate_ok && size_ok && burn.is_none() {
        return Decision {
            method: Method::Direct,
            video: None,
            audio: None,
            subtitle: None,
            audio_index,
            burn_index: None,
            reasons: Vec::new(),
        };
    }

    let video_req = video.map(|v| {
        let copy = v.codec == "h264"
            && caps.video("h264")
            && v.bit_depth.unwrap_or(8) <= 8
            && !v.dynamic_range().is_hdr()
            && v.interlaced != Some(true)
            && bitrate_ok
            && size_ok
            && burn.is_none();
        VideoRequest {
            stream_index: Some(v.index),
            mode: if copy { VideoMode::Copy } else { VideoMode::Transcode },
            codec: VideoCodec::H264,
            max_width: None,
            max_height: req.max_height,
            max_bitrate_kbps: req.max_bitrate.map(|b| b.saturating_sub(192).max(500)),
        }
    });
    let audio_req = audio.map(|a| {
        let copy = matches!(a.codec.as_str(), "aac" | "mp3") && caps.audio(&a.codec);
        AudioRequest {
            stream_index: Some(a.index),
            mode: if copy { AudioMode::Copy } else { AudioMode::Transcode },
            channels: Some(2),
            bitrate_kbps: None,
        }
    });
    let remux = match &video_req {
        Some(v) => v.mode == VideoMode::Copy,
        None => audio_req.as_ref().is_some_and(|a| a.mode == AudioMode::Copy),
    };
    let subtitle = burn.map(|s| {
        if s.external {
            let path = file.external_subs().into_iter().find(|e| e.index == s.index).map(|e| e.path);
            SubtitleBurnRequest { stream_index: None, external_path: path }
        } else {
            SubtitleBurnRequest { stream_index: Some(s.index), external_path: None }
        }
    });
    Decision {
        method: if remux { Method::Remux } else { Method::Transcode },
        video: video_req,
        audio: audio_req,
        burn_index: burn.map(|s| s.index),
        subtitle,
        audio_index,
        reasons,
    }
}

pub fn subtitle_tracks(file_id: i64, streams: &[MediaStream]) -> Vec<SubtitleTrack> {
    streams
        .iter()
        .filter(|s| s.kind == StreamKind::Subtitle)
        .map(|s| {
            let text = s.text_based.unwrap_or(true);
            SubtitleTrack {
                index: s.index,
                codec: s.codec.clone(),
                language: s.language.clone(),
                title: s.title.clone(),
                default: s.default,
                forced: s.forced,
                external: s.external,
                text_based: text,
                url: text.then(|| format!("/api/files/{file_id}/subtitles/{}.vtt", s.index)),
            }
        })
        .collect()
}

// ------------------------------------------------------------------ sessions

#[derive(Debug, Clone, Serialize)]
pub struct PlayState {
    pub position_ms: i64,
    pub state: String,
    pub updated_at: i64,
}

pub struct Session {
    pub id: String,
    pub user_id: i64,
    pub username: String,
    pub item_id: i64,
    pub file_id: i64,
    pub method: Method,
    pub duration_ms: i64,
    pub job_request: Option<JobRequest>,
    pub job_id: Mutex<Option<String>>,
    pub started_at: i64,
    pub play: Mutex<PlayState>,
    /// Serialises job re-creation when the transcoder lost the job.
    pub recreate: tokio::sync::Mutex<()>,
    last_transcoder_touch: AtomicI64,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: i64,
        username: String,
        item_id: i64,
        file_id: i64,
        method: Method,
        duration_ms: i64,
        job_request: Option<JobRequest>,
        job_id: Option<String>,
        start_ms: i64,
    ) -> Self {
        let now = now();
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            user_id,
            username,
            item_id,
            file_id,
            method,
            duration_ms,
            job_request,
            job_id: Mutex::new(job_id),
            started_at: now,
            play: Mutex::new(PlayState { position_ms: start_ms, state: "playing".into(), updated_at: now }),
            recreate: tokio::sync::Mutex::new(()),
            last_transcoder_touch: AtomicI64::new(now),
        }
    }

    pub fn job_id(&self) -> Option<String> {
        self.job_id.lock().clone()
    }

    pub fn touch(&self) {
        self.play.lock().updated_at = now();
    }

    pub fn update(&self, position_ms: i64, state: &str) {
        let mut play = self.play.lock();
        play.position_ms = position_ms;
        play.state = state.to_owned();
        play.updated_at = now();
    }

    /// Keep the transcoder job alive while the player reports progress (paused players).
    pub fn should_touch_transcoder(&self) -> bool {
        let now = now();
        let last = self.last_transcoder_touch.load(Ordering::Relaxed);
        if now - last >= 30 {
            self.last_transcoder_touch.store(now, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<String, Arc<Session>>>,
}

const SESSION_IDLE_SECS: i64 = 15 * 60;
const STOPPED_LINGER_SECS: i64 = 60;
const ACTIVE_WINDOW_SECS: i64 = 90;

impl Sessions {
    pub fn insert(&self, session: Session) -> Arc<Session> {
        let session = Arc::new(session);
        self.map.lock().insert(session.id.clone(), session.clone());
        session
    }

    pub fn get(&self, id: &str) -> Option<Arc<Session>> {
        self.map.lock().get(id).cloned()
    }

    pub fn remove(&self, id: &str) -> Option<Arc<Session>> {
        self.map.lock().remove(id)
    }

    pub fn list(&self) -> Vec<Arc<Session>> {
        let mut list: Vec<_> = self.map.lock().values().cloned().collect();
        list.sort_by_key(|s| s.started_at);
        list
    }

    /// Transcode sessions with recent activity (segment requests or progress
    /// reports). Abandoned sessions (closed tabs) stop counting quickly even
    /// though they are only reaped later.
    pub fn active_transcodes(&self) -> usize {
        let cutoff = now() - ACTIVE_WINDOW_SECS;
        self.map
            .lock()
            .values()
            .filter(|s| {
                let play = s.play.lock();
                s.job_request.is_some() && play.state != "stopped" && play.updated_at >= cutoff
            })
            .count()
    }

    pub fn spawn_reaper(self: &Arc<Self>, transcoder: Arc<Transcoder>, events: EventBus) {
        let sessions = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                let Some(sessions) = sessions.upgrade() else { break };
                let now = now();
                let expired: Vec<Arc<Session>> = sessions
                    .list()
                    .into_iter()
                    .filter(|s| {
                        let play = s.play.lock();
                        now - play.updated_at > SESSION_IDLE_SECS
                            || (play.state == "stopped" && now - play.updated_at > STOPPED_LINGER_SECS)
                    })
                    .collect();
                for session in &expired {
                    sessions.remove(&session.id);
                    if let Some(job) = session.job_id() {
                        let _ = transcoder.delete_job(&job).await;
                    }
                }
                if !expired.is_empty() {
                    events.send(ServerEvent::SessionsChanged);
                }
            }
        });
    }
}

// ------------------------------------------------------------------ status mapping

#[derive(Debug, Clone, Serialize)]
pub struct TranscodeInfo {
    pub job_id: String,
    pub state: pley_transcoder_api::JobState,
    pub video: &'static str,
    pub audio: &'static str,
    pub video_in: Option<String>,
    pub video_out: Option<String>,
    pub audio_in: Option<String>,
    pub audio_out: Option<String>,
    pub encoder: Option<String>,
    pub decoder: Option<String>,
    pub tonemap: Option<String>,
    pub subtitle_burn: bool,
    pub speed: Option<f64>,
    pub fps: Option<f64>,
    pub transcoded_ms: i64,
    pub segments_ready: u32,
    pub segments_total: u32,
    pub last_error: Option<String>,
}

fn codec_label(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" => "HEVC".into(),
        "av1" => "AV1".into(),
        "mpeg2video" => "MPEG-2".into(),
        "mpeg4" => "MPEG-4".into(),
        "truehd" => "TrueHD".into(),
        other => other.to_uppercase(),
    }
}

pub fn channel_label(channels: i64) -> String {
    match channels {
        1 => "1.0".into(),
        2 => "2.0".into(),
        3 => "2.1".into(),
        6 => "5.1".into(),
        7 => "6.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

pub fn transcode_info(job: &JobStatus) -> TranscodeInfo {
    let video = job.video.as_ref();
    let audio = job.audio.as_ref();
    let video_in = video.map(|v| {
        let mut s = format!("{} {}×{}", codec_label(&v.codec_in), v.width_in.unwrap_or(0), v.height_in.unwrap_or(0));
        if v.dynamic_range_in.is_hdr() {
            s.push(' ');
            s.push_str(&match v.dv_profile {
                Some(p) => format!("Dolby Vision P{p}"),
                None => v.dynamic_range_in.label().to_owned(),
            });
        }
        s
    });
    let video_out = video
        .map(|v| format!("{} {}×{}", codec_label(&v.codec_out), v.width_out.unwrap_or(0), v.height_out.unwrap_or(0)));
    let audio_in = audio.map(|a| {
        format!("{} {}", codec_label(&a.codec_in), a.channels_in.map(channel_label).unwrap_or_default())
            .trim()
            .to_owned()
    });
    let audio_out = audio.map(|a| {
        format!("{} {}", codec_label(&a.codec_out), a.channels_out.map(channel_label).unwrap_or_default())
            .trim()
            .to_owned()
    });
    let process = job.process.as_ref();
    let transcoded_ms = process.map(|p| p.out_time_ms).unwrap_or_else(|| {
        (job.segments_ready as f64 * job.segment_seconds * 1000.0).min(job.duration_ms as f64) as i64
    });
    TranscodeInfo {
        job_id: job.id.clone(),
        state: job.state,
        video: match video {
            None => "none",
            Some(v) if v.mode == VideoMode::Copy => "copy",
            Some(_) => "transcode",
        },
        audio: match audio {
            None => "none",
            Some(a) if a.mode == AudioMode::Copy => "copy",
            Some(_) => "transcode",
        },
        video_in,
        video_out,
        audio_in,
        audio_out,
        encoder: video.and_then(|v| v.encoder.clone()),
        decoder: video.and_then(|v| v.decoder.clone()),
        tonemap: video.and_then(|v| v.tonemap).map(|t| t.as_str().to_owned()),
        subtitle_burn: video.is_some_and(|v| v.subtitle_burn.is_some()),
        speed: process.and_then(|p| p.speed),
        fps: process.and_then(|p| p.fps),
        transcoded_ms,
        segments_ready: job.segments_ready,
        segments_total: job.segment_count,
        last_error: job.last_error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pley_transcoder_api::DynamicRange;

    fn file(container: &str, bitrate: i64) -> FileRow {
        FileRow {
            id: 7,
            item_id: 1,
            library_id: 1,
            path: "/m/a".into(),
            size: 1,
            mtime: 0,
            container: Some(container.into()),
            duration_ms: Some(60_000),
            bitrate: Some(bitrate),
            width: Some(1920),
            height: Some(1080),
            video_codec: None,
            audio_codec: None,
            media_info: None,
            external_subs: "[]".into(),
        }
    }

    fn info(video: &str, audio: &[&str], range: DynamicRange) -> MediaInfo {
        let mut streams = vec![MediaStream {
            index: 0,
            kind: StreamKind::Video,
            codec: video.into(),
            width: Some(1920),
            height: Some(1080),
            bit_depth: Some(8),
            dynamic_range: Some(range),
            ..Default::default()
        }];
        for (i, a) in audio.iter().enumerate() {
            streams.push(MediaStream {
                index: i as i64 + 1,
                kind: StreamKind::Audio,
                codec: (*a).into(),
                default: i == 0,
                channels: Some(6),
                ..Default::default()
            });
        }
        streams.push(MediaStream {
            index: 9,
            kind: StreamKind::Subtitle,
            codec: "hdmv_pgs_subtitle".into(),
            text_based: Some(false),
            ..Default::default()
        });
        MediaInfo { container: Some("mkv".into()), duration_ms: Some(60_000), streams, ..Default::default() }
    }

    fn browser() -> Caps {
        Caps {
            containers: vec!["mp4".into(), "webm".into()],
            video_codecs: vec!["h264".into(), "vp9".into()],
            audio_codecs: vec!["aac".into(), "mp3".into(), "opus".into()],
            hdr: false,
        }
    }

    fn request() -> PlaybackRequest {
        PlaybackRequest {
            item_id: 1,
            file_id: None,
            audio_index: None,
            subtitle_index: None,
            max_bitrate: None,
            max_height: None,
            start_ms: None,
            caps: browser(),
        }
    }

    #[test]
    fn mp4_h264_aac_direct_plays() {
        let i = info("h264", &["aac"], DynamicRange::Sdr);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &request());
        assert_eq!(d.method, Method::Direct);
        assert!(d.reasons.is_empty());
    }

    #[test]
    fn mkv_h264_remuxes_and_transcodes_audio() {
        let i = info("h264", &["ac3"], DynamicRange::Sdr);
        let d = decide(&file("mkv", 5_000_000), Some(&i), &i.streams, &request());
        assert_eq!(d.method, Method::Remux);
        assert_eq!(d.video.unwrap().mode, VideoMode::Copy);
        assert_eq!(d.audio.unwrap().mode, AudioMode::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("mkv")));
    }

    #[test]
    fn hdr_hevc_transcodes() {
        let i = info("hevc", &["eac3"], DynamicRange::DolbyVision);
        let d = decide(&file("mp4", 20_000_000), Some(&i), &i.streams, &request());
        assert_eq!(d.method, Method::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("Dolby Vision")));
    }

    #[test]
    fn quality_limit_and_burn_in_force_transcode() {
        let i = info("h264", &["aac"], DynamicRange::Sdr);
        let mut req = request();
        req.max_bitrate = Some(2_000);
        let d = decide(&file("mp4", 8_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.video.as_ref().unwrap().max_bitrate_kbps, Some(1_808));

        let mut req = request();
        req.subtitle_index = Some(9);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.subtitle.unwrap().stream_index, Some(9));
    }

    #[test]
    fn alternate_audio_track_needs_hls() {
        let i = info("h264", &["aac", "aac"], DynamicRange::Sdr);
        let mut req = request();
        req.audio_index = Some(2);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Remux);
        assert_eq!(d.audio.unwrap().stream_index, Some(2));
    }

    #[test]
    fn subtitle_tracks_have_urls_for_text() {
        let streams = vec![
            MediaStream {
                index: 3,
                kind: StreamKind::Subtitle,
                codec: "subrip".into(),
                text_based: Some(true),
                ..Default::default()
            },
            MediaStream {
                index: 4,
                kind: StreamKind::Subtitle,
                codec: "hdmv_pgs_subtitle".into(),
                text_based: Some(false),
                ..Default::default()
            },
        ];
        let tracks = subtitle_tracks(7, &streams);
        assert_eq!(tracks[0].url.as_deref(), Some("/api/files/7/subtitles/3.vtt"));
        assert_eq!(tracks[1].url, None);
    }
}
