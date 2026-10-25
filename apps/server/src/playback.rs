//! Playback decisions and sessions.
//!
//! For every play request the server compares the file with what the client
//! can decode and picks **direct play** (the raw file over HTTP range requests),
//! **remux** (HLS with the video stream copied) or **transcode** (HLS produced by
//! the transcoder). Sessions are tracked for "now playing" and resume.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use pley_transcoder_api::{
    AudioCodec, AudioMode, AudioRequest, DynamicRange, JobRequest, JobStatus, MediaInfo, MediaStream, StreamKind,
    SubtitleBurnRequest, VideoCodec, VideoMode, VideoRequest, dynamic_range_label,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::catalog::FileRow;
use crate::events::{EventBus, ServerEvent};
use crate::transcoder::Transcoder;
use crate::util::now;

/// What a player plays as it is, sent with every play request.
///
/// Version 1 is the first five fields: codec lists and two booleans, what
/// the web app and the Android app's web view send. Version 2
/// (`caps_version: 2`, the native Android app's `ClientCaps`) adds every
/// decoder with its limits, the screen's dynamic ranges, the Dolby Vision
/// profiles decoded, and the audio decoded, passed through to a receiver
/// and the output's channels. Players without `caps_version` play exactly
/// as before version 2 existed. Version 2 players still send the version 1
/// fields: `containers` always counts, the codec lists only where the
/// version 2 fields that replace them are missing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Caps {
    #[serde(default)]
    pub containers: Vec<String>,
    #[serde(default)]
    pub video_codecs: Vec<String>,
    #[serde(default)]
    pub audio_codecs: Vec<String>,
    /// Client shows HDR10 / HLG as it is: on an HDR screen, or tone-mapping
    /// it itself (desktop Chromium does on SDR screens).
    #[serde(default)]
    pub hdr: bool,
    /// Client decodes Dolby Vision (including profile 5, which has no HDR10
    /// or SDR base layer).
    #[serde(default)]
    pub dolby_vision: bool,

    // ---- version 2 ----
    /// 2 for the fields below; absent: version 1.
    #[serde(default)]
    pub caps_version: Option<u32>,
    /// Every video decoder with its limits. A codec can have several (4K at
    /// 30 fps and 1080p at 60, say): a stream plays when one takes it.
    #[serde(default)]
    pub video: Option<Vec<VideoDecoder>>,
    /// The dynamic ranges the screen shows as they are (a decoder that feeds
    /// it included): `"hdr10"`, `"hdr10plus"`, `"hlg"`, `"dolby_vision"`.
    #[serde(default)]
    pub hdr_types: Option<Vec<String>>,
    /// The Dolby Vision profiles decoded, each with its highest level; sent
    /// only when the screen shows Dolby Vision.
    #[serde(default)]
    pub dolby_vision_profiles: Option<Vec<DolbyVisionDecoder>>,
    /// Audio codecs decoded on the device, by FFmpeg's names.
    #[serde(default)]
    pub audio_decode: Option<Vec<String>>,
    /// Audio codecs sent as they are to a receiver or a TV.
    #[serde(default)]
    pub audio_passthrough: Option<Vec<String>>,
    /// The most channels the audio output takes: 2, 6, 8.
    #[serde(default)]
    pub audio_channels: Option<u32>,
    /// Subtitle codecs the player shows itself, pictures (PGS…) included.
    #[serde(default)]
    pub subtitle_codecs: Option<Vec<String>>,
    /// The player switches a file's audio tracks itself.
    #[serde(default)]
    pub audio_track_switching: Option<bool>,
    #[serde(default)]
    pub hls: Option<HlsCaps>,
    #[serde(default)]
    pub display: Option<DisplayCaps>,
    #[serde(default)]
    pub device: Option<DeviceCaps>,
}

/// One video decoder of a version 2 player. Limits left out aren't known
/// and aren't checked.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VideoDecoder {
    pub codec: String,
    pub hardware: bool,
    /// The profiles it decodes, as ffprobe names them: "Main", "Main 10"…
    pub profiles: Vec<String>,
    pub max_width: Option<i64>,
    pub max_height: Option<i64>,
    pub max_fps: Option<f64>,
    pub max_bit_depth: Option<i64>,
    /// Numbered as ffprobe numbers a stream's `level`.
    pub max_level: Option<i64>,
}

/// A Dolby Vision profile a version 2 player decodes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DolbyVisionDecoder {
    pub profile: i64,
    pub max_level: Option<i64>,
}

/// What a version 2 player takes of HLS.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HlsCaps {
    /// Reads multivariant playlists.
    pub master_playlist: bool,
    /// Segment formats: "ts", "fmp4".
    pub segments: Vec<String>,
}

/// A version 2 player's screen mode.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DisplayCaps {
    pub width: i64,
    pub height: i64,
    pub refresh_rates: Vec<f64>,
}

/// A version 2 player's device.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeviceCaps {
    /// "phone", "tablet", "tv" or "unknown".
    pub form: String,
    /// "low", "mid" or "high".
    pub tier: Option<String>,
    pub fire_tv: bool,
    pub model: Option<String>,
}

fn listed(list: &[String], c: &str) -> bool {
    list.iter().any(|x| x.eq_ignore_ascii_case(c))
}

impl Caps {
    fn container(&self, c: &str) -> bool {
        listed(&self.containers, c)
    }
    fn video(&self, c: &str) -> bool {
        listed(&self.video_codecs, c)
    }
    fn audio(&self, c: &str) -> bool {
        listed(&self.audio_codecs, c)
    }

    /// A version 2 report (`caps_version` 2 or more).
    pub fn is_v2(&self) -> bool {
        self.caps_version.is_some_and(|v| v >= 2)
    }

    /// The player takes this audio codec as it is: decoded, or (version 2)
    /// passed through to a receiver.
    fn plays_audio(&self, c: &str) -> bool {
        if !self.is_v2() || (self.audio_decode.is_none() && self.audio_passthrough.is_none()) {
            return self.audio(c);
        }
        [&self.audio_decode, &self.audio_passthrough].into_iter().flatten().any(|list| listed(list, c))
    }

    /// The screen shows this dynamic range (version 2 names).
    fn shows(&self, range: &str) -> bool {
        self.hdr_types.as_deref().is_some_and(|types| listed(types, range))
    }

    /// The version 2 player shows these subtitles itself.
    fn shows_subtitles(&self, codec: &str) -> bool {
        self.is_v2() && self.subtitle_codecs.as_deref().is_some_and(|list| listed(list, codec))
    }
}

/// How the client wants the media delivered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackMode {
    /// The cheapest method the client supports: direct play, then remux, then transcode.
    #[default]
    Auto,
    /// The original file, even when the client did not claim support for it.
    Direct,
    /// HLS, copying the video stream when possible.
    Remux,
    /// HLS with the video always re-encoded.
    Transcode,
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
    #[serde(default)]
    pub mode: PlaybackMode,
    /// Identifies the player (a browser tab, a device). A client plays one
    /// thing at a time, so its previous sessions end when it starts a new one.
    #[serde(default)]
    pub client_id: Option<String>,
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
    /// The audio is converted, also when the video is copied (a direct stream).
    pub audio_converted: bool,
    pub subtitles: Vec<SubtitleTrack>,
    /// Why the file isn't played as it is, one short sentence each.
    pub reasons: Vec<String>,
    /// The same, in one line.
    pub reason: Option<String>,
    /// The quality a transcode of the original aims for, by the players' id
    /// ("30m"…, see [`QUALITIES`]): `None` when a quality was asked for or the
    /// video isn't converted.
    pub auto_quality: Option<String>,
}

/// The qualities players offer (apps/web/src/player/quality.ts), highest
/// bitrate first: id, kbps, lines.
pub const QUALITIES: [(&str, u32, u32); 7] = [
    ("30m", 30_000, 2160),
    ("20m", 20_000, 1080),
    ("15m", 15_000, 2160),
    ("12m", 12_000, 1080),
    ("8m", 8_000, 1080),
    ("4m", 4_000, 720),
    ("2m", 2_000, 480),
];

/// What a transcode of the original aims for, instead of the file's own
/// size and bitrate (heavy to make and to send, for no better picture): the
/// closest quality players offer. It keeps the file's resolution and never
/// goes above it (4K stays 4K, a 1920×800 film stays 1080p), with the
/// highest bitrate up to 1.5 times the file's (H.264 needs more than HEVC
/// for the same picture), or that resolution's lowest for a file with fewer
/// bits than any.
pub fn closest_quality(width: Option<i64>, height: Option<i64>, bitrate: Option<i64>) -> (&'static str, u32, u32) {
    closest_quality_within(width, height, bitrate, None)
}

/// [`closest_quality`] for a player whose decoders stop at `most_lines`
/// (a 1080p TV stick gets 1080p for a 4K file).
pub fn closest_quality_within(
    width: Option<i64>,
    height: Option<i64>,
    bitrate: Option<i64>,
    most_lines: Option<u32>,
) -> (&'static str, u32, u32) {
    let (w, h) = (width.unwrap_or(0), height.unwrap_or(0));
    let lines = if w >= 3800 || h >= 2100 {
        2160
    } else if w >= 1800 || h >= 1000 || (w <= 0 && h <= 0) {
        1080
    } else if w >= 1200 || h >= 700 {
        720
    } else {
        480
    };
    let lines = match most_lines {
        Some(most) => [2160, 1080, 720, 480].into_iter().find(|l| *l <= lines && *l <= most).unwrap_or(480),
        None => lines,
    };
    let cap = bitrate.filter(|b| *b > 0).map(|b| b as f64 * 1.5 / 1000.0);
    let mut tier = QUALITIES.iter().filter(|q| q.2 == lines);
    let lowest = tier.clone().next_back();
    tier.find(|q| cap.is_none_or(|cap| f64::from(q.1) <= cap)).or(lowest).copied().unwrap_or(QUALITIES[4])
}

/// `GET /api/playback/{session}`: how a session plays, with its transcode's
/// live numbers.
#[derive(Debug, Clone, Serialize)]
pub struct SessionStatus {
    pub session_id: String,
    pub method: Method,
    pub transcode: Option<TranscodeInfo>,
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
    /// The quality picked for the original ([`closest_quality`]), by id.
    pub quality: Option<&'static str>,
}

/// Why the client cannot show this video's dynamic range as is (the video is
/// then tone-mapped to SDR), or `None` when it can.
fn range_problem(v: &MediaStream, caps: &Caps) -> Option<String> {
    let range = v.dynamic_range();
    if !range.is_hdr() {
        return None;
    }
    let label = v.dynamic_range_label();
    if range == DynamicRange::DolbyVision {
        if caps.dolby_vision {
            return None;
        }
        if v.is_dolby_vision_only() {
            // No HDR10/SDR base layer: colours are wrong without DV processing.
            return Some(format!("{label} needs a Dolby Vision screen: it's converted to SDR."));
        }
        // Profile 7/8: an HDR10 or HLG base layer plays as plain HDR.
        if caps.hdr {
            return None;
        }
    } else if caps.hdr {
        return None;
    }
    Some(format!("{label} is converted to SDR for this screen."))
}

/// What a version 2 player does with a video stream.
#[derive(Debug, Default, PartialEq)]
struct VideoVerdict {
    /// The file can play as it is, as far as the video goes.
    direct: bool,
    /// The stream can be copied into HLS and play right (Dolby Vision
    /// profile 7 as its HDR10 base layer, say).
    copy: bool,
    reasons: Vec<String>,
}

impl VideoVerdict {
    fn plays() -> Self {
        VideoVerdict { direct: true, copy: true, reasons: Vec::new() }
    }
    fn no(reason: String) -> Self {
        VideoVerdict { direct: false, copy: false, reasons: vec![reason] }
    }
}

/// Dolby Vision base-layer compatibility ids (ffprobe's
/// `dv_bl_signal_compatibility_id`).
const DV_BL_HDR10: i64 = 1;
const DV_BL_SDR: i64 = 2;
const DV_BL_HLG: i64 = 4;

/// The version 2 rules for a video stream (docs/API.md, "Version 2"):
/// - SDR video needs a decoder entry that takes it: codec, profile (Main 10
///   for 10-bit), size, frame rate and level;
/// - HDR10 needs `hdr10` on the screen, HDR10+ `hdr10plus` or `hdr10`, HLG
///   `hlg`; otherwise it is tone-mapped to SDR;
/// - Dolby Vision plays as it is when the screen shows it and the profile
///   is listed. Otherwise profiles 8.1 and 8.4 (and 9, 10) play their
///   HDR10, HLG or SDR base layer by those rules; profile 7, whose
///   enhancement layer Media3 can't decode, is repackaged so its HDR10 base
///   layer plays; profile 5, with no base layer, is tone-mapped, never
///   repackaged (an MPEG-TS remux loses the Dolby Vision signalling, which
///   leaves its colours wrong).
fn video_v2(v: &MediaStream, caps: &Caps) -> VideoVerdict {
    let label = v.dynamic_range_label();
    let to_sdr = || format!("{label} is converted to SDR for this screen.");
    match v.dynamic_range() {
        DynamicRange::Sdr => fits(v, caps),
        DynamicRange::Hdr10 if caps.shows("hdr10") => fits(v, caps),
        DynamicRange::Hdr10plus if caps.shows("hdr10plus") || caps.shows("hdr10") => fits(v, caps),
        DynamicRange::Hlg if caps.shows("hlg") => fits(v, caps),
        DynamicRange::Hdr10 | DynamicRange::Hdr10plus | DynamicRange::Hlg => VideoVerdict::no(to_sdr()),
        DynamicRange::DolbyVision => {
            let profile = v.dv_profile.unwrap_or(0);
            let decoder = caps
                .dolby_vision_profiles
                .iter()
                .flatten()
                .find(|d| d.profile == profile)
                .filter(|_| caps.shows("dolby_vision"));
            let level_ok = match (v.dv_level, decoder.and_then(|d| d.max_level)) {
                (Some(level), Some(max)) => level <= max,
                _ => true,
            };
            // What plays when Dolby Vision itself doesn't: the base layer.
            let compat = v.dv_bl_compat.or((profile == 8).then_some(DV_BL_HDR10));
            let base = match (profile, compat) {
                (5, _) => VideoVerdict::no(format!("{label} needs a Dolby Vision screen: it's converted to SDR.")),
                (7, _) if caps.shows("hdr10") => {
                    let mut base = fits(v, caps);
                    if base.copy {
                        base.direct = false;
                        base.reasons.push(format!("{label} plays as its HDR10 base layer, repackaged."));
                    }
                    base
                }
                (7, _) => VideoVerdict::no(to_sdr()),
                (_, Some(DV_BL_HDR10)) if caps.shows("hdr10") => fits(v, caps),
                (_, Some(DV_BL_HLG)) if caps.shows("hlg") => fits(v, caps),
                (_, Some(DV_BL_SDR)) => fits(v, caps),
                (_, Some(DV_BL_HDR10 | DV_BL_HLG)) => VideoVerdict::no(to_sdr()),
                _ => VideoVerdict::no(format!("{label} needs a Dolby Vision screen: it's converted to SDR.")),
            };
            match decoder {
                Some(_) if level_ok => VideoVerdict { direct: true, copy: base.copy, reasons: Vec::new() },
                Some(d) if !base.direct => {
                    let mut base = base;
                    let max = d.max_level.unwrap_or_default();
                    base.reasons.insert(
                        0,
                        format!("Dolby Vision level {} is above this player's {max}.", v.dv_level.unwrap_or_default()),
                    );
                    base
                }
                _ => base,
            }
        }
    }
}

/// Whether one of a version 2 player's decoders takes the stream: any one
/// entry of its codec, each limit it gives checked.
fn fits(v: &MediaStream, caps: &Caps) -> VideoVerdict {
    let Some(decoders) = caps.video.as_deref() else {
        // A version 2 report without decoders: its codec list.
        return if caps.video(&v.codec) {
            VideoVerdict::plays()
        } else {
            VideoVerdict::no(format!("This player can't play {} video.", codec_label(&v.codec)))
        };
    };
    let entries: Vec<&VideoDecoder> = decoders.iter().filter(|d| d.codec.eq_ignore_ascii_case(&v.codec)).collect();
    if entries.is_empty() {
        return VideoVerdict::no(format!("This player can't play {} video.", codec_label(&v.codec)));
    }
    let depth = bit_depth(v);
    if entries.iter().any(|d| takes(d, v, depth)) {
        return VideoVerdict::plays();
    }
    let mut what = match (v.width, v.height) {
        (Some(w), Some(h)) => format!("{w}×{h}"),
        _ => String::new(),
    };
    if let Some(fps) = v.frame_rate {
        what.push_str(&format!(" at {} fps", trim_fps(fps)));
    }
    what.push_str(&format!(", {depth}-bit"));
    if let Some(profile) = &v.profile {
        what.push_str(&format!(" ({profile})"));
    }
    VideoVerdict::no(format!("This player can't play {} {}.", codec_label(&v.codec), what.trim_start()))
}

/// A frame rate as people write it: 23.976, 25, 59.94.
fn trim_fps(fps: f64) -> String {
    let s = format!("{fps:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Profiles both ffprobe and Android name the same way: a stream's profile
/// missing from a decoder's list refuses it only when it is one of these.
const KNOWN_PROFILES: &[&str] = &[
    "main",
    "main 10",
    "main still picture",
    "rext",
    "baseline",
    "constrained baseline",
    "high",
    "high 10",
    "high 4:2:2",
    "high 4:4:4 predictive",
    "profile 0",
    "profile 1",
    "profile 2",
    "profile 3",
    "professional",
];

fn takes(d: &VideoDecoder, v: &MediaStream, depth: i64) -> bool {
    if let (Some(w), Some(h), Some(max_w), Some(max_h)) = (v.width, v.height, d.max_width, d.max_height) {
        // A decoder that takes 1920×1080 takes 1080×1920 too.
        if !(w <= max_w && h <= max_h) && !(h <= max_w && w <= max_h) {
            return false;
        }
    }
    if let (Some(fps), Some(max)) = (v.frame_rate, d.max_fps) {
        if fps > max + 0.01 {
            return false;
        }
    }
    if d.max_bit_depth.is_some_and(|max| depth > max) {
        return false;
    }
    if let (Some(level), Some(max)) = (v.level, d.max_level) {
        if level > 0 && level > max {
            return false;
        }
    }
    if !d.profiles.is_empty() {
        let has = |p: &str| d.profiles.iter().any(|x| x.eq_ignore_ascii_case(p));
        if let Some(profile) = v.profile.as_deref() {
            if !has(profile) && KNOWN_PROFILES.contains(&profile.to_ascii_lowercase().as_str()) {
                return false;
            }
        }
        // 10-bit HEVC needs Main 10 even when ffprobe gave no profile.
        if v.codec == "hevc" && depth >= 10 && !has("Main 10") && !has("Rext") {
            return false;
        }
    }
    true
}

/// The stream's bit depth, else from its pixel format ("yuv420p10le") or
/// profile ("Main 10").
fn bit_depth(v: &MediaStream) -> i64 {
    if let Some(depth) = v.bit_depth {
        return depth;
    }
    let from_pix_fmt = v.pix_fmt.as_deref().and_then(|f| {
        let f = f.trim_end_matches("le").trim_end_matches("be");
        let digits = &f[f.trim_end_matches(|c: char| c.is_ascii_digit()).len()..];
        digits.parse::<i64>().ok().filter(|d| matches!(d, 10 | 12))
    });
    from_pix_fmt.unwrap_or(if v.profile.as_deref().is_some_and(|p| p.contains("10")) { 10 } else { 8 })
}

/// The codec and channels audio is converted to: E-AC-3 5.1 for a
/// multichannel output that takes E-AC-3 (decoded or passed through), else
/// AC-3 5.1, else AAC with as many channels as the output takes (up to
/// 5.1). Stereo sources, stereo outputs and players that don't say how many
/// channels they have (the web) get stereo AAC.
fn converted_audio(caps: &Caps, source: &MediaStream) -> (AudioCodec, u32) {
    let output = caps.audio_channels.unwrap_or(2).max(1);
    let surround = output >= 6 && source.channels.unwrap_or(2) > 2;
    if surround && caps.plays_audio("eac3") {
        (AudioCodec::Eac3, 6)
    } else if surround && caps.plays_audio("ac3") {
        (AudioCodec::Ac3, 6)
    } else {
        // The transcoder never makes more channels than the source has.
        (AudioCodec::Aac, output.clamp(2, 6))
    }
}

/// The most lines a version 2 player's H.264 decoders take (what
/// transcodes are made in), when it says.
fn h264_lines(caps: &Caps) -> Option<u32> {
    caps.video
        .iter()
        .flatten()
        .filter(|d| d.codec.eq_ignore_ascii_case("h264"))
        .map(|d| d.max_height.zip(d.max_width).map(|(h, w)| h.min(w)))
        .collect::<Option<Vec<i64>>>()
        .and_then(|lines| lines.into_iter().max())
        .and_then(|l| u32::try_from(l).ok())
}

/// Video codecs HLS can carry in MPEG-TS segments, which players unpack
/// (hls.js turns them into fragmented MP4 for the browser).
fn hls_copyable(codec: &str) -> bool {
    matches!(codec, "h264" | "hevc")
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
        let direct = match req.mode {
            PlaybackMode::Direct => true,
            PlaybackMode::Auto => caps.container(container) || web_container(container),
            PlaybackMode::Remux | PlaybackMode::Transcode => false,
        };
        return Decision {
            method: if direct { Method::Direct } else { Method::Transcode },
            video: (!direct).then(VideoRequest::default),
            audio: (!direct).then(AudioRequest::default),
            subtitle: None,
            audio_index: None,
            burn_index: None,
            reasons: if direct { vec![] } else { vec!["This file hasn't been analyzed yet.".into()] },
            quality: None,
        };
    };

    let video = info.video();
    let default_audio = info.default_audio();
    let audio = req.audio_index.and_then(|i| info.stream(i).filter(|s| s.kind == StreamKind::Audio)).or(default_audio);

    let v2 = caps.is_v2();
    let burn_stream =
        req.subtitle_index.and_then(|i| streams.iter().find(|s| s.index == i && s.kind == StreamKind::Subtitle));
    let pictures = burn_stream.filter(|s| !s.text_based.unwrap_or(true) && video.is_some());
    // Picture subtitles a version 2 player shows itself need drawing only
    // when the file doesn't play as it is (HLS segments carry no subtitles).
    let player_draws = pictures.is_some_and(|s| !s.external && caps.shows_subtitles(&s.codec));
    let mut burn = pictures.filter(|_| !player_draws);
    let burn_reason =
        |s: &MediaStream| format!("{} subtitles are pictures, drawn onto the video.", codec_label(&s.codec));
    if let Some(s) = burn {
        reasons.push(burn_reason(s));
    }

    let container_ok = caps.container(container);
    if !container_ok {
        reasons.push(format!("This player can't open {} files.", container.to_uppercase()));
    }
    // Version 2: whether the video plays as it is, and whether it plays
    // copied into HLS (a profile 7 base layer, say), which version 1 takes
    // to be the same.
    let mut video_copy_ok = true;
    let video_ok = match video {
        None => true,
        Some(v) if v2 => {
            let verdict = video_v2(v, caps);
            reasons.extend(verdict.reasons);
            video_copy_ok = verdict.copy;
            verdict.direct
        }
        Some(v) => {
            let mut ok = true;
            if !caps.video(&v.codec) {
                reasons.push(format!("This player can't play {} video.", codec_label(&v.codec)));
                ok = false;
            }
            if let Some(problem) = range_problem(v, caps) {
                reasons.push(problem);
                ok = false;
            }
            if v.codec == "h264" && v.bit_depth.unwrap_or(8) > 8 {
                reasons.push("Browsers can't play 10-bit H.264.".into());
                ok = false;
            }
            video_copy_ok = ok;
            ok
        }
    };
    let audio_ok = match audio {
        None => true,
        Some(a) if caps.plays_audio(&a.codec) => true,
        Some(a) => {
            reasons.push(format!("This player can't play {} audio.", codec_label(&a.codec)));
            false
        }
    };
    let audio_is_default = match (audio, default_audio) {
        (Some(a), Some(d)) => a.index == d.index,
        _ => true,
    } || (v2 && caps.audio_track_switching == Some(true));
    if !audio_is_default {
        reasons.push("This player plays only a file's main audio track.".into());
    }
    let bitrate_ok = req.max_bitrate.is_none_or(|max| file.bitrate.unwrap_or(0) <= max as i64 * 1000);
    let size_ok = req.max_height.is_none_or(|max| video.and_then(|v| v.height).unwrap_or(0) <= max as i64);
    if !bitrate_ok || !size_ok {
        reasons.push("You picked a lower quality.".into());
    }

    let audio_index = audio.map(|a| a.index);
    let mut direct_ok =
        container_ok && video_ok && audio_ok && audio_is_default && bitrate_ok && size_ok && burn.is_none();
    if player_draws && !direct_ok && req.mode != PlaybackMode::Direct {
        // Not as it is after all: the subtitles are drawn onto the video.
        burn = pictures;
        if let Some(s) = burn {
            reasons.insert(0, burn_reason(s));
        }
        direct_ok = false;
    }
    let direct = match req.mode {
        PlaybackMode::Auto => direct_ok,
        // The user knows better (e.g. a browser that plays more than it
        // admits); image subtitles and alternate audio are lost.
        PlaybackMode::Direct => true,
        PlaybackMode::Remux | PlaybackMode::Transcode => false,
    };
    if direct {
        return Decision {
            method: Method::Direct,
            video: None,
            audio: None,
            subtitle: None,
            audio_index,
            burn_index: None,
            reasons: Vec::new(),
            quality: None,
        };
    }

    match req.mode {
        PlaybackMode::Remux if direct_ok => reasons.push("The player asked for the file repackaged.".into()),
        PlaybackMode::Transcode => reasons.push("The player asked for the file converted.".into()),
        _ => {}
    }
    let mut quality = None;
    let video_req = video.map(|v| {
        // Whatever the client plays as it is goes through untouched, like
        // Plex's Direct Stream: only the audio or the container changes.
        let copy = req.mode != PlaybackMode::Transcode
            && video_copy_ok
            && hls_copyable(&v.codec)
            && v.interlaced != Some(true)
            && bitrate_ok
            && size_ok
            && burn.is_none();
        // The original, converted: at the closest quality players offer.
        // A version 2 player's H.264 decoders bound what it is converted to.
        let most_lines = if v2 && !copy { h264_lines(caps) } else { None };
        let (max_height, max_bitrate) = if copy || req.max_bitrate.is_some() || req.max_height.is_some() {
            let max_height = match (req.max_height, most_lines) {
                (Some(asked), Some(most)) => Some(asked.min(most)),
                (None, Some(most)) if v.height.is_some_and(|h| h > i64::from(most)) => Some(most),
                (asked, _) => asked,
            };
            (max_height, req.max_bitrate)
        } else {
            let (id, kbps, lines) = closest_quality_within(v.width, v.height, file.bitrate, most_lines);
            quality = Some(id);
            (Some(lines), Some(kbps))
        };
        VideoRequest {
            stream_index: Some(v.index),
            mode: if copy { VideoMode::Copy } else { VideoMode::Transcode },
            codec: VideoCodec::H264,
            max_width: None,
            max_height,
            max_bitrate_kbps: max_bitrate.map(|b| b.saturating_sub(192).max(500)),
        }
    });
    let audio_req = audio.map(|a| {
        // Version 1 players copy what browsers play from MPEG-TS; version 2
        // players everything MPEG-TS carries that they play.
        let copy = if v2 {
            matches!(a.codec.as_str(), "aac" | "mp3" | "ac3" | "eac3") && caps.plays_audio(&a.codec)
        } else {
            matches!(a.codec.as_str(), "aac" | "mp3") && caps.audio(&a.codec)
        };
        let (codec, channels) = converted_audio(caps, a);
        AudioRequest {
            stream_index: Some(a.index),
            mode: if copy { AudioMode::Copy } else { AudioMode::Transcode },
            codec,
            channels: Some(channels),
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
        quality,
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

/// What a transcode session asked the transcoder for, kept to re-create the
/// job with the exact same segment layout if the transcoder loses it.
#[derive(Debug, Clone)]
pub struct TranscodeSpec {
    /// The request with the *effective* video mode (a remux request the
    /// transcoder answered with a transcode stays a transcode).
    pub request: JobRequest,
    pub segment_count: u32,
    pub duration_ms: i64,
}

pub struct Session {
    pub id: String,
    pub user_id: i64,
    pub username: String,
    pub client_id: Option<String>,
    pub item_id: i64,
    pub file_id: i64,
    pub method: Method,
    pub duration_ms: i64,
    pub transcode: Option<TranscodeSpec>,
    pub job_id: Mutex<Option<String>>,
    pub started_at: i64,
    pub play: Mutex<PlayState>,
    /// The app grant whose token started it (`None`: a session token). The
    /// Plex API's token-less session files refuse these.
    pub app_grant: Option<i64>,
    /// Serialises job re-creation when the transcoder lost the job.
    pub recreate: tokio::sync::Mutex<()>,
    last_transcoder_touch: AtomicI64,
    /// Not heard from for [`ACTIVE_WINDOW_SECS`] (no report, heartbeat or
    /// request for media): out of "Now playing", its transcode stopped. The
    /// next sign of life wakes it, and a request for media makes a new
    /// transcode (`api::playback::replacement_job`).
    quiet: AtomicBool,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: i64,
        username: String,
        client_id: Option<String>,
        item_id: i64,
        file_id: i64,
        method: Method,
        duration_ms: i64,
        transcode: Option<TranscodeSpec>,
        job_id: Option<String>,
        start_ms: i64,
    ) -> Self {
        let now = now();
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            user_id,
            username,
            client_id,
            item_id,
            file_id,
            method,
            duration_ms,
            transcode,
            job_id: Mutex::new(job_id),
            started_at: now,
            play: Mutex::new(PlayState { position_ms: start_ms, state: "playing".into(), updated_at: now }),
            app_grant: None,
            recreate: tokio::sync::Mutex::new(()),
            last_transcoder_touch: AtomicI64::new(now),
            quiet: AtomicBool::new(false),
        }
    }

    pub fn job_id(&self) -> Option<String> {
        self.job_id.lock().clone()
    }

    pub fn position_ms(&self) -> i64 {
        self.play.lock().position_ms
    }

    /// A request for its media; whether that woke it (it shows again in
    /// "Now playing").
    pub fn touch(&self) -> bool {
        self.play.lock().updated_at = now();
        self.quiet.swap(false, Ordering::SeqCst)
    }

    /// Records a progress report; whether the play state (playing, paused,
    /// stopped) changed or the session woke, which the dashboard and Plex
    /// tools hear about at once.
    pub fn update(&self, position_ms: i64, state: &str) -> bool {
        let mut play = self.play.lock();
        let changed = play.state != state;
        play.position_ms = position_ms;
        if changed {
            play.state = state.to_owned();
        }
        play.updated_at = now();
        drop(play);
        let woke = self.quiet.swap(false, Ordering::SeqCst);
        changed || woke
    }

    /// Playing or paused, with a player that was heard from lately: what
    /// "Now playing" lists.
    pub fn is_active(&self, now: i64) -> bool {
        let play = self.play.lock();
        play.state != "stopped" && now - play.updated_at <= ACTIVE_WINDOW_SECS
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
    /// Transcode slots of play requests still creating their job (see
    /// [`Sessions::reserve_transcode`]). Only raised while `map` is locked.
    reserved: AtomicUsize,
    /// Sessions started so far (see [`Sessions::started_since`]).
    starts: AtomicU64,
    /// A session started or ended.
    changed: Notify,
}

/// A transcode slot, held while the job is created. It counts against the
/// limit until the session takes it over ([`Sessions::insert_transcode`]) or
/// it is dropped: the request failed, or the client went away.
pub struct TranscodeSlot<'a> {
    sessions: &'a Sessions,
}

impl Drop for TranscodeSlot<'_> {
    fn drop(&mut self) {
        self.sessions.reserved.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Quiet sessions are forgotten after this (a player that comes back later
/// starts a new one).
const SESSION_IDLE_SECS: i64 = 15 * 60;
const STOPPED_LINGER_SECS: i64 = 60;
/// Players report every 10 seconds while playing and every 30 while paused:
/// a session not heard from for this long has lost its player.
const ACTIVE_WINDOW_SECS: i64 = 90;
const REAP_EVERY: Duration = Duration::from_secs(15);
/// Background work that reads whole files (seek previews for a library,
/// intro detection) starts only once nothing has played for this long, so it
/// never takes the disk, the network or the GPU from someone watching (or
/// about to start the next episode).
pub const QUIET_BEFORE_BACKGROUND_SECS: i64 = 60;
/// A playback is starting for this long after its session began: work that
/// can wait (seek previews) keeps off the disk and the network meanwhile, so
/// the player's first reads get them (see [`Sessions::starting_for`]).
pub const STARTING_SECS: i64 = 30;
/// Direct plays a user has at most: the oldest go first. They cost only
/// memory (transcodes have their own limit), but a client starting plays in
/// a loop mustn't be able to grow it without end.
const MAX_DIRECT_PER_USER: usize = 20;

impl Sessions {
    pub fn insert(&self, session: Session) -> Arc<Session> {
        let session = Arc::new(session);
        let mut map = self.map.lock();
        let mut direct: Vec<(i64, String)> = map
            .values()
            .filter(|s| s.user_id == session.user_id && s.transcode.is_none())
            .map(|s| (s.started_at, s.id.clone()))
            .collect();
        if direct.len() >= MAX_DIRECT_PER_USER {
            direct.sort_unstable();
            for (_, id) in &direct[..=direct.len() - MAX_DIRECT_PER_USER] {
                map.remove(id);
            }
        }
        map.insert(session.id.clone(), session.clone());
        drop(map);
        self.started();
        session
    }

    /// Take one of `max` transcode slots, or `None` when live transcodes and
    /// the play requests still creating their job use them all. Creating a
    /// job takes seconds: checking and taking happen under one lock, so play
    /// requests arriving together can't all pass the check before any of
    /// them has a session.
    pub fn reserve_transcode(&self, max: usize) -> Option<TranscodeSlot<'_>> {
        let map = self.map.lock();
        if live_transcodes(&map) + self.reserved.load(Ordering::SeqCst) >= max {
            return None;
        }
        self.reserved.fetch_add(1, Ordering::SeqCst);
        Some(TranscodeSlot { sessions: self })
    }

    /// Add a transcode session, which takes over the slot reserved for it.
    pub fn insert_transcode(&self, session: Session, slot: TranscodeSlot<'_>) -> Arc<Session> {
        let session = Arc::new(session);
        let mut map = self.map.lock();
        map.insert(session.id.clone(), session.clone());
        // Under the same lock: the session counts as the slot stops counting.
        drop(slot);
        drop(map);
        self.started();
        session
    }

    fn started(&self) {
        self.starts.fetch_add(1, Ordering::SeqCst);
        self.changed.notify_waiters();
    }

    /// How many sessions have started so far, for [`Sessions::started_since`].
    pub fn starts(&self) -> u64 {
        self.starts.load(Ordering::SeqCst)
    }

    /// Resolves once a session starts after [`Sessions::starts`] said `seen`.
    pub async fn started_since(&self, seen: u64) {
        loop {
            // Registered before the look, so no start slips in between.
            let changed = self.changed.notified();
            if self.starts() != seen {
                return;
            }
            changed.await;
        }
    }

    /// Resolves at the next session that starts or ends.
    pub async fn changed(&self) {
        self.changed.notified().await;
    }

    /// How long the newest session that hasn't stopped is still starting
    /// (it began less than [`STARTING_SECS`] ago), or `None` when none is.
    pub fn starting_for(&self) -> Option<Duration> {
        let now = now();
        let newest =
            self.map.lock().values().filter(|s| s.play.lock().state != "stopped").map(|s| s.started_at).max()?;
        let left = STARTING_SECS - (now - newest);
        (left > 0).then(|| Duration::from_secs(left as u64))
    }

    pub fn get(&self, id: &str) -> Option<Arc<Session>> {
        self.map.lock().get(id).cloned()
    }

    pub fn remove(&self, id: &str) -> Option<Arc<Session>> {
        let removed = self.map.lock().remove(id);
        if removed.is_some() {
            self.changed.notify_waiters();
        }
        removed
    }

    /// Remove the sessions of `user_id` playing on `client_id` (the client
    /// moved on to something else, or switched quality / audio track).
    pub fn take_client_sessions(&self, user_id: i64, client_id: &str) -> Vec<Arc<Session>> {
        let mut map = self.map.lock();
        let ids: Vec<String> = map
            .values()
            .filter(|s| s.user_id == user_id && s.client_id.as_deref() == Some(client_id))
            .map(|s| s.id.clone())
            .collect();
        let taken: Vec<_> = ids.iter().filter_map(|id| map.remove(id)).collect();
        drop(map);
        if !taken.is_empty() {
            self.changed.notify_waiters();
        }
        taken
    }

    pub fn list(&self) -> Vec<Arc<Session>> {
        let mut list: Vec<_> = self.map.lock().values().cloned().collect();
        list.sort_by_key(|s| s.started_at);
        list
    }

    /// The sessions a player is still there for (see [`Session::is_active`]),
    /// oldest first: "Now playing".
    pub fn active(&self) -> Vec<Arc<Session>> {
        let now = now();
        self.list().into_iter().filter(|s| s.is_active(now)).collect()
    }

    /// Seconds since anything played: 0 while a player is there, counted
    /// from the last sign of the sessions still kept otherwise, and "long
    /// ago" without any.
    pub fn quiet_for(&self) -> i64 {
        let now = now();
        let list = self.list();
        if list.iter().any(|s| s.is_active(now)) {
            return 0;
        }
        list.iter().map(|s| now - s.play.lock().updated_at).min().unwrap_or(i64::MAX)
    }

    /// One look at every session at `now`: those to forget (stopped a
    /// while ago, or quiet for long) and those that just went quiet. Both
    /// leave "Now playing"; their transcodes are the caller's to stop.
    pub fn sweep(&self, now: i64) -> Sweep {
        let mut sweep = Sweep::default();
        for session in self.list() {
            let (stopped, idle) = {
                let play = session.play.lock();
                (play.state == "stopped", now - play.updated_at)
            };
            if idle > SESSION_IDLE_SECS || (stopped && idle > STOPPED_LINGER_SECS) {
                self.remove(&session.id);
                sweep.forgotten.push(session);
            } else if idle > ACTIVE_WINDOW_SECS && !session.quiet.swap(true, Ordering::SeqCst) {
                sweep.quieted.push(session);
            }
        }
        sweep
    }

    /// Transcode sessions with recent activity (segment requests or progress
    /// reports). Abandoned sessions (closed tabs) stop counting quickly even
    /// though they are only reaped later.
    pub fn active_transcodes(&self) -> usize {
        live_transcodes(&self.map.lock())
    }

    /// Every [`REAP_EVERY`]: sessions whose player went quiet leave "Now
    /// playing" and their transcodes stop (a closed tab, a phone gone
    /// to sleep, a crashed app); sessions quiet for long are forgotten.
    pub fn spawn_reaper(self: &Arc<Self>, transcoder: Arc<Transcoder>, events: EventBus) {
        let sessions = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(REAP_EVERY);
            loop {
                tick.tick().await;
                let Some(sessions) = sessions.upgrade() else { break };
                let sweep = sessions.sweep(now());
                // A quieted session's job is gone already when it is forgotten.
                let stop =
                    sweep.quieted.iter().chain(sweep.forgotten.iter().filter(|s| !s.quiet.load(Ordering::SeqCst)));
                for job in stop.filter_map(|s| s.job_id()).collect::<Vec<_>>() {
                    let _ = transcoder.delete_job(&job).await;
                }
                if !sweep.quieted.is_empty() || !sweep.forgotten.is_empty() {
                    events.send(ServerEvent::SessionsChanged);
                }
            }
        });
    }
}

/// What [`Sessions::sweep`] found.
#[derive(Default)]
pub struct Sweep {
    pub forgotten: Vec<Arc<Session>>,
    pub quieted: Vec<Arc<Session>>,
}

/// See [`Sessions::active_transcodes`].
fn live_transcodes(map: &HashMap<String, Arc<Session>>) -> usize {
    let now = now();
    map.values().filter(|s| s.transcode.is_some() && s.is_active(now)).count()
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
    /// The software encoder's preset now (the automatic one changes).
    pub preset: Option<String>,
    pub decoder: Option<String>,
    /// `gpu` (decode → filters → encode on one GPU), `hybrid`, `cpu` or `copy`.
    pub pipeline: Option<String>,
    pub tonemap: Option<String>,
    pub subtitle_burn: bool,
    pub speed: Option<f64>,
    pub fps: Option<f64>,
    pub transcoded_ms: i64,
    pub segments_ready: u32,
    pub segments_total: u32,
    /// What the transcode aims for, video and audio together (bits per second).
    pub bitrate_out: Option<i64>,
    pub last_error: Option<String>,
}

/// A codec as people know it: `"hevc"` → `"HEVC"`, `"hdmv_pgs_subtitle"` → `"PGS"`.
pub fn codec_label(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" => "HEVC".into(),
        "av1" => "AV1".into(),
        "vp9" => "VP9".into(),
        "vc1" => "VC-1".into(),
        "mpeg2video" => "MPEG-2".into(),
        "mpeg4" => "MPEG-4".into(),
        "truehd" => "TrueHD".into(),
        "eac3" => "E-AC3".into(),
        "opus" => "Opus".into(),
        "hdmv_pgs_subtitle" | "pgssub" => "PGS".into(),
        "dvd_subtitle" | "dvdsub" => "VobSub".into(),
        "dvb_subtitle" => "DVB".into(),
        other => other.to_uppercase(),
    }
}

/// A note from the transcoder as a sentence: `"no tone mapper available"` →
/// `"No tone mapper available."`.
pub fn sentence(note: &str) -> String {
    let note = note.trim();
    let mut chars = note.chars();
    let mut out: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    if !out.is_empty() && !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
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
            s.push_str(&dynamic_range_label(v.dynamic_range_in, v.dv_profile, v.dv_bl_compat));
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
        preset: process.and_then(|p| p.preset.clone()),
        decoder: video.and_then(|v| v.decoder.clone()),
        pipeline: video.and_then(|v| v.pipeline.clone()),
        tonemap: video.and_then(|v| v.tonemap).map(|t| t.as_str().to_owned()),
        subtitle_burn: video.is_some_and(|v| v.subtitle_burn.is_some()),
        speed: process.and_then(|p| p.speed),
        fps: process.and_then(|p| p.fps),
        transcoded_ms,
        segments_ready: job.segments_ready,
        segments_total: job.segment_count,
        bitrate_out: video
            .filter(|v| v.mode != VideoMode::Copy)
            .and_then(|v| v.bitrate_kbps)
            .map(|kbps| (i64::from(kbps) + audio.and_then(|a| a.bitrate_kbps).map_or(0, i64::from)) * 1000),
        last_error: job.last_error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pley_transcoder_api::DynamicRange;

    #[test]
    fn a_user_keeps_a_bounded_number_of_direct_plays() {
        let sessions = Sessions::default();
        let play = |user: i64| Session::new(user, "u".into(), None, 1, 1, Method::Direct, 0, None, None, 0);
        let other = sessions.insert(play(2));
        let mut last = None;
        for _ in 0..MAX_DIRECT_PER_USER + 5 {
            last = Some(sessions.insert(play(1)));
        }
        let mine = sessions.list().into_iter().filter(|s| s.user_id == 1).count();
        assert_eq!(mine, MAX_DIRECT_PER_USER);
        assert!(sessions.get(&last.unwrap().id).is_some(), "the newest stays");
        assert!(sessions.get(&other.id).is_some(), "other users' plays are theirs");
    }

    #[tokio::test]
    async fn new_sessions_are_starting_for_a_while() {
        let sessions = Sessions::default();
        assert_eq!(sessions.starting_for(), None, "nothing plays");
        let play = |age: i64| {
            let mut session = Session::new(1, "u".into(), None, 1, 1, Method::Direct, 0, None, None, 0);
            session.started_at -= age;
            session
        };
        let seen = sessions.starts();
        let old = sessions.insert(play(STARTING_SECS + 5));
        assert_eq!(sessions.starting_for(), None, "playing for a while");
        assert_ne!(sessions.starts(), seen);
        // Noticed even when it started before the wait began.
        tokio::time::timeout(Duration::from_secs(1), sessions.started_since(seen)).await.expect("a start");

        let seen = sessions.starts();
        let waiting = sessions.started_since(seen);
        tokio::pin!(waiting);
        assert!(futures_now(waiting.as_mut()).is_none(), "nothing new yet");
        let new = sessions.insert(play(10));
        assert!(futures_now(waiting.as_mut()).is_some(), "a session started");
        let left = sessions.starting_for().expect("starting");
        assert!(
            left <= Duration::from_secs((STARTING_SECS - 10) as u64) && left >= Duration::from_secs(15),
            "{left:?}"
        );

        // A stopped one is no longer starting; nor one that ended.
        new.update(0, "stopped");
        assert_eq!(sessions.starting_for(), None);
        new.update(0, "playing");
        assert!(sessions.starting_for().is_some());
        let ended = sessions.changed();
        tokio::pin!(ended);
        assert!(futures_now(ended.as_mut()).is_none());
        sessions.remove(&new.id);
        assert!(futures_now(ended.as_mut()).is_some(), "the end is heard");
        assert_eq!(sessions.starting_for(), None);
        assert!(sessions.get(&old.id).is_some());
    }

    /// Polls a future once.
    fn futures_now<F: std::future::Future>(future: std::pin::Pin<&mut F>) -> Option<F::Output> {
        let waker = std::task::Waker::noop();
        match future.poll(&mut std::task::Context::from_waker(waker)) {
            std::task::Poll::Ready(out) => Some(out),
            std::task::Poll::Pending => None,
        }
    }

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
            fingerprint: None,
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
            dolby_vision: false,
            ..Default::default()
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
            mode: PlaybackMode::Auto,
            client_id: None,
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
        assert!(d.reasons.iter().any(|r| r.contains("MKV")), "{:?}", d.reasons);
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
    fn the_original_converted_aims_for_the_closest_quality() {
        let pick = |w, h, mbps: i64| closest_quality(Some(w), Some(h), Some(mbps * 1_000_000)).0;
        assert_eq!(pick(3840, 2160, 57), "30m", "a 4K remux");
        assert_eq!(pick(3840, 2160, 18), "15m");
        assert_eq!(pick(3840, 2160, 6), "15m", "4K stays 4K, even with few bits");
        assert_eq!(pick(3840, 1600, 40), "30m", "scope 4K");
        assert_eq!(pick(1920, 1080, 25), "20m");
        assert_eq!(pick(1920, 1080, 10), "12m", "HEVC 10 Mbps: H.264 needs more");
        assert_eq!(pick(1920, 1080, 4), "8m");
        assert_eq!(pick(1920, 800, 9), "12m", "scope 1080p stays 1080p");
        assert_eq!(pick(1280, 720, 3), "4m");
        assert_eq!(pick(720, 480, 2), "2m");
        assert_eq!(closest_quality(None, None, None).0, "20m", "nothing known: 1080p's best");

        // HEVC can't play here: converted at that quality, which the menu shows.
        let i = info("hevc", &["aac"], DynamicRange::Sdr);
        let d = decide(&file("mkv", 25_000_000), Some(&i), &i.streams, &request());
        let v = d.video.as_ref().unwrap();
        assert_eq!((d.method, d.quality), (Method::Transcode, Some("20m")));
        assert_eq!((v.max_height, v.max_bitrate_kbps), (Some(1080), Some(19_808)));

        // A quality asked for wins, and copied video has none.
        let mut req = request();
        req.max_bitrate = Some(4_000);
        req.max_height = Some(720);
        let d = decide(&file("mkv", 25_000_000), Some(&i), &i.streams, &req);
        assert_eq!((d.quality, d.video.unwrap().max_height), (None, Some(720)));
        let h264 = info("h264", &["ac3"], DynamicRange::Sdr);
        let d = decide(&file("mkv", 5_000_000), Some(&h264), &h264.streams, &request());
        assert_eq!((d.method, d.quality), (Method::Remux, None));
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

    fn dolby_vision(profile: i64, compat: i64) -> MediaInfo {
        let mut i = info("hevc", &["aac"], DynamicRange::DolbyVision);
        i.streams[0].dv_profile = Some(profile);
        i.streams[0].dv_bl_compat = Some(compat);
        i.streams[0].bit_depth = Some(10);
        i
    }

    #[test]
    fn dolby_vision_needs_matching_client() {
        let mut req = request();
        req.caps.containers.push("mkv".into());
        req.caps.video_codecs.push("hevc".into());
        req.caps.hdr = true;

        // Profile 8.1 has an HDR10 base layer: an HDR client plays it directly.
        let p81 = dolby_vision(8, 1);
        assert_eq!(decide(&file("mkv", 20_000_000), Some(&p81), &p81.streams, &req).method, Method::Direct);

        // Profile 5 needs Dolby Vision decoding, otherwise it is tone-mapped.
        let p5 = dolby_vision(5, 0);
        let d = decide(&file("mkv", 20_000_000), Some(&p5), &p5.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("Dolby Vision P5")), "{:?}", d.reasons);
        req.caps.dolby_vision = true;
        assert_eq!(decide(&file("mkv", 20_000_000), Some(&p5), &p5.streams, &req).method, Method::Direct);

        // Without HDR support, 8.1 is tone-mapped too.
        req.caps.dolby_vision = false;
        req.caps.hdr = false;
        let d = decide(&file("mkv", 20_000_000), Some(&p81), &p81.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("Dolby Vision P8.1")), "{:?}", d.reasons);
    }

    /// Chrome on Windows: HEVC through the graphics card, Matroska, no Dolby
    /// Vision, no AC-3 family, an SDR screen it tone-maps HDR for.
    fn chrome() -> Caps {
        let mut caps = browser();
        caps.containers.push("mkv".into());
        caps.video_codecs.extend(["hevc".into(), "av1".into()]);
        caps.hdr = true;
        caps
    }

    #[test]
    fn video_the_player_decodes_is_copied_when_only_the_audio_needs_work() {
        let mut req = request();
        req.caps = chrome();

        // Blade Runner 2049's remux: Dolby Vision 8.1 with TrueHD Atmos, as
        // Plex plays it in a browser: the video copied, the audio converted.
        let mut p81 = dolby_vision(8, 1);
        p81.streams[1].codec = "truehd".into();
        let d = decide(&file("mkv", 60_000_000), Some(&p81), &p81.streams, &req);
        assert_eq!(d.method, Method::Remux);
        assert_eq!(d.video.unwrap().mode, VideoMode::Copy);
        assert_eq!(d.audio.unwrap().mode, AudioMode::Transcode);
        assert_eq!(d.reasons, ["This player can't play TrueHD audio."]);

        // HDR10 and SDR HEVC alike.
        for range in [DynamicRange::Hdr10, DynamicRange::Sdr] {
            let i = info("hevc", &["eac3"], range);
            let d = decide(&file("mkv", 20_000_000), Some(&i), &i.streams, &req);
            assert_eq!((d.method, d.video.unwrap().mode), (Method::Remux, VideoMode::Copy), "{range:?}");
        }

        // VP9 and AV1 can't travel in MPEG-TS segments: they play directly
        // when they can and are converted otherwise.
        for codec in ["vp9", "av1"] {
            let i = info(codec, &["aac"], DynamicRange::Sdr);
            assert_eq!(decide(&file("mkv", 5_000_000), Some(&i), &i.streams, &req).method, Method::Direct);
            let i = info(codec, &["ac3"], DynamicRange::Sdr);
            let d = decide(&file("mkv", 5_000_000), Some(&i), &i.streams, &req);
            assert_eq!((d.method, d.video.unwrap().mode), (Method::Transcode, VideoMode::Transcode), "{codec}");
        }

        // A screen without HDR, a lower quality or burned-in subtitles still
        // convert the video.
        let hdr10 = info("hevc", &["eac3"], DynamicRange::Hdr10);
        req.caps.hdr = false;
        assert_eq!(decide(&file("mkv", 20_000_000), Some(&hdr10), &hdr10.streams, &req).method, Method::Transcode);
        req.caps.hdr = true;
        req.max_bitrate = Some(8_000);
        assert_eq!(decide(&file("mkv", 20_000_000), Some(&hdr10), &hdr10.streams, &req).method, Method::Transcode);
    }

    #[test]
    fn playback_modes_override_the_decision() {
        let i = info("h264", &["aac"], DynamicRange::Sdr);
        let mut req = request();
        req.mode = PlaybackMode::Transcode;
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.video.unwrap().mode, VideoMode::Transcode);

        req.mode = PlaybackMode::Remux;
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Remux);

        let hevc = info("hevc", &["eac3"], DynamicRange::Sdr);
        req.mode = PlaybackMode::Direct;
        let d = decide(&file("mkv", 5_000_000), Some(&hevc), &hevc.streams, &req);
        assert_eq!(d.method, Method::Direct);
    }

    #[test]
    fn unprobed_files_play_directly_only_in_web_containers() {
        let req = request();
        let d = decide(&file("mp4", 0), None, &[], &req);
        assert_eq!((d.method, d.reasons.len()), (Method::Direct, 0));
        let d = decide(&file("mkv", 0), None, &[], &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.reasons, ["This file hasn't been analyzed yet."]);
        assert!(d.video.is_some() && d.audio.is_some());
        // The player claims the container: trusted.
        let mut req = request();
        req.caps.containers.push("mkv".into());
        assert_eq!(decide(&file("mkv", 0), None, &[], &req).method, Method::Direct);
        req.mode = PlaybackMode::Remux;
        assert_eq!(decide(&file("mp4", 0), None, &[], &req).method, Method::Transcode, "nothing is known to copy");
    }

    #[test]
    fn ten_bit_and_interlaced_h264_are_re_encoded() {
        let mut i = info("h264", &["aac"], DynamicRange::Sdr);
        i.streams[0].bit_depth = Some(10);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &request());
        assert_eq!(d.method, Method::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("10-bit")), "{:?}", d.reasons);
        assert_eq!(d.video.unwrap().mode, VideoMode::Transcode);

        let mut i = info("h264", &["aac"], DynamicRange::Sdr);
        i.streams[0].interlaced = Some(true);
        let d = decide(&file("mkv", 5_000_000), Some(&i), &i.streams, &request());
        assert_eq!(d.video.unwrap().mode, VideoMode::Transcode, "browsers can't deinterlace");
    }

    #[test]
    fn resolution_and_bitrate_limits() {
        let i = info("h264", &["aac"], DynamicRange::Sdr);
        let mut req = request();
        req.max_height = Some(1080);
        assert_eq!(decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req).method, Method::Direct);
        req.max_height = Some(720);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.video.as_ref().unwrap().max_height, Some(720));
        // Very low limits keep a watchable floor; the file's bitrate exactly at the limit plays.
        let mut req = request();
        req.max_bitrate = Some(300);
        let d = decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.video.unwrap().max_bitrate_kbps, Some(500));
        req.max_bitrate = Some(5_000);
        assert_eq!(decide(&file("mp4", 5_000_000), Some(&i), &i.streams, &req).method, Method::Direct);
    }

    #[test]
    fn hdr10_plays_directly_on_hdr_displays() {
        let i = info("hevc", &["aac"], DynamicRange::Hdr10);
        let mut req = request();
        req.caps.video_codecs.push("hevc".into());
        let d = decide(&file("mp4", 20_000_000), Some(&i), &i.streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert!(d.reasons.iter().any(|r| r.contains("converted to SDR")), "{:?}", d.reasons);
        req.caps.hdr = true;
        assert_eq!(decide(&file("mp4", 20_000_000), Some(&i), &i.streams, &req).method, Method::Direct);
    }

    fn track(codec: &str) -> MediaInfo {
        MediaInfo {
            duration_ms: Some(200_000),
            streams: vec![MediaStream {
                index: 0,
                kind: StreamKind::Audio,
                codec: codec.into(),
                default: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn audio_only_files_remux_or_transcode_the_audio() {
        let mut req = request();
        req.caps.containers.push("flac".into());
        let flac = track("flac");
        let d = decide(&file("flac", 900_000), Some(&flac), &flac.streams, &req);
        assert_eq!(d.method, Method::Transcode, "flac is not in the player's codecs");
        assert!(d.video.is_none());
        assert_eq!(d.audio.unwrap().mode, AudioMode::Transcode);

        let mp3 = track("mp3");
        let d = decide(&file("mka", 320_000), Some(&mp3), &mp3.streams, &request());
        assert_eq!(d.method, Method::Remux, "the audio is copied into HLS");
        assert_eq!(d.audio.unwrap().mode, AudioMode::Copy);
    }

    #[test]
    fn external_image_subtitles_are_burned_from_their_file() {
        let mut i = info("h264", &["aac"], DynamicRange::Sdr);
        i.streams.push(MediaStream {
            index: 5,
            kind: StreamKind::Subtitle,
            codec: "subrip".into(),
            text_based: Some(true),
            ..Default::default()
        });
        let mut f = file("mp4", 5_000_000);
        f.media_info = Some(serde_json::to_string(&i).unwrap());
        f.external_subs =
            r#"[{"index":1000,"path":"/m/a.en.sup","language":"eng","codec":"hdmv_pgs_subtitle"}]"#.into();
        let streams = f.streams();
        let ext = streams.iter().find(|s| s.index == 1000).expect("the sidecar is a stream");
        assert!(ext.external && ext.text_based == Some(false));
        let mut req = request();
        req.subtitle_index = Some(1000);
        let d = decide(&f, Some(&i), &streams, &req);
        assert_eq!(d.method, Method::Transcode);
        assert_eq!(d.burn_index, Some(1000));
        let burn = d.subtitle.unwrap();
        assert_eq!((burn.stream_index, burn.external_path.as_deref()), (None, Some("/m/a.en.sup")));
        // Text subtitles are shown by the player: nothing to burn.
        let mut req = request();
        req.subtitle_index = Some(5);
        assert_eq!(decide(&f, Some(&i), &streams, &req).method, Method::Direct);
    }

    #[test]
    fn transcode_status_labels() {
        use pley_transcoder_api::{AudioPlan, JobState, ProcessStatus, VideoPlan};
        let job = JobStatus {
            id: "j".into(),
            state: JobState::Running,
            duration_ms: 60_000,
            segment_seconds: 4.0,
            segment_count: 15,
            segments_ready: 3,
            video: Some(VideoPlan {
                mode: VideoMode::Transcode,
                codec_in: "hevc".into(),
                codec_out: "h264".into(),
                width_in: Some(3840),
                height_in: Some(2160),
                width_out: Some(1920),
                height_out: Some(1080),
                dynamic_range_in: DynamicRange::DolbyVision,
                dv_profile: Some(8),
                dv_bl_compat: Some(1),
                ..Default::default()
            }),
            audio: Some(AudioPlan {
                mode: AudioMode::Transcode,
                codec_in: "truehd".into(),
                codec_out: "aac".into(),
                channels_in: Some(8),
                channels_out: Some(2),
                ..Default::default()
            }),
            ..Default::default()
        };
        let info = transcode_info(&job);
        assert_eq!(info.video_in.as_deref(), Some("HEVC 3840×2160 Dolby Vision P8.1"));
        assert_eq!(info.video_out.as_deref(), Some("H.264 1920×1080"));
        assert_eq!((info.audio_in.as_deref(), info.audio_out.as_deref()), (Some("TrueHD 7.1"), Some("AAC 2.0")));
        assert_eq!((info.video, info.audio), ("transcode", "transcode"));
        assert_eq!(info.transcoded_ms, 12_000, "ready segments while no process reports a time");
        let running =
            JobStatus { process: Some(ProcessStatus { out_time_ms: 30_500, ..Default::default() }), ..job.clone() };
        assert_eq!(transcode_info(&running).transcoded_ms, 30_500);
        assert_eq!(transcode_info(&JobStatus { video: None, audio: None, ..job }).video, "none");
        assert_eq!(channel_label(6), "5.1");
        assert_eq!(channel_label(4), "4ch");
    }

    fn session(user_id: i64, client: Option<&str>, transcode: bool) -> Session {
        let spec =
            transcode.then(|| TranscodeSpec { request: JobRequest::default(), segment_count: 1, duration_ms: 1 });
        Session::new(user_id, "u".into(), client.map(str::to_owned), 1, 1, Method::Transcode, 1, spec, None, 0)
    }

    #[test]
    fn a_client_plays_one_thing_at_a_time() {
        let sessions = Sessions::default();
        let tab = sessions.insert(session(1, Some("tab"), true)).id.clone();
        sessions.insert(session(1, Some("tv"), true));
        sessions.insert(session(2, Some("tab"), true));
        sessions.insert(session(1, None, false));
        let taken = sessions.take_client_sessions(1, "tab");
        assert_eq!(taken.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), [tab.as_str()]);
        assert!(sessions.get(&tab).is_none());
        assert_eq!(sessions.list().len(), 3, "other clients and users keep playing");
        assert!(sessions.take_client_sessions(1, "tab").is_empty());
    }

    #[test]
    fn only_live_transcodes_count_against_the_limit() {
        let sessions = Sessions::default();
        sessions.insert(session(1, None, false));
        let live = sessions.insert(session(1, None, true));
        let stopped = sessions.insert(session(1, None, true));
        stopped.update(0, "stopped");
        let abandoned = sessions.insert(session(1, None, true));
        abandoned.play.lock().updated_at = now() - ACTIVE_WINDOW_SECS - 1;
        assert_eq!(sessions.active_transcodes(), 1);
        live.update(1_000, "paused");
        assert_eq!(sessions.active_transcodes(), 1, "paused players keep their slot");
    }

    #[test]
    fn sessions_whose_player_went_quiet_leave_now_playing_and_wake_again() {
        let sessions = Sessions::default();
        let playing = sessions.insert(session(1, None, true));
        let paused = sessions.insert(session(1, None, false));
        paused.update(5_000, "paused");
        let stopped = sessions.insert(session(1, None, false));
        stopped.update(9_000, "stopped");
        let gone = sessions.insert(session(1, None, true));
        gone.play.lock().updated_at = now() - ACTIVE_WINDOW_SECS - 1;
        // Sorted: sessions started in the same second come in any order.
        let ids = |list: Vec<Arc<Session>>| {
            let mut ids = list.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
            ids.sort();
            ids
        };
        assert_eq!(
            ids(sessions.active()),
            ids(vec![playing.clone(), paused.clone()]),
            "stopped and quiet ones aren't playing"
        );

        let sweep = sessions.sweep(now());
        assert_eq!(ids(sweep.quieted), std::slice::from_ref(&gone.id));
        assert!(sweep.forgotten.is_empty(), "stopped a moment ago: kept a little");
        assert!(sessions.sweep(now()).quieted.is_empty(), "quieted once");
        assert!(sessions.get(&gone.id).is_some(), "kept, to wake");

        // A sign of life wakes it.
        assert!(gone.touch(), "a request for media");
        assert!(!gone.touch());
        assert!(ids(sessions.active()).contains(&gone.id));
        gone.play.lock().updated_at = now() - ACTIVE_WINDOW_SECS - 1;
        assert_eq!(sessions.sweep(now()).quieted.len(), 1);
        assert!(gone.update(10_000, "playing"), "a progress report wakes it too");

        // Later: stopped ones and long-quiet ones are forgotten.
        let later = now() + SESSION_IDLE_SECS + 1;
        let forgotten = ids(sessions.sweep(later).forgotten);
        assert_eq!(forgotten.len(), 4);
        assert!(sessions.list().is_empty());
    }

    #[test]
    fn background_work_waits_for_quiet() {
        let sessions = Sessions::default();
        assert_eq!(sessions.quiet_for(), i64::MAX, "nothing played since the start");
        let playing = sessions.insert(session(1, None, false));
        assert_eq!(sessions.quiet_for(), 0);
        playing.update(5_000, "paused");
        assert_eq!(sessions.quiet_for(), 0, "a paused player is still there");
        playing.update(5_000, "stopped");
        playing.play.lock().updated_at = now() - 30;
        assert_eq!(sessions.quiet_for(), 30, "stopped half a minute ago");
        assert!(sessions.quiet_for() < QUIET_BEFORE_BACKGROUND_SECS);
    }

    #[test]
    fn transcode_slots_are_taken_before_the_job_exists() {
        let sessions = Sessions::default();
        let first = sessions.reserve_transcode(2).expect("a free slot");
        let second = sessions.reserve_transcode(2).expect("another one");
        assert!(sessions.reserve_transcode(2).is_none(), "both are taken while their jobs are created");
        drop(second);
        let third = sessions.reserve_transcode(2).expect("a failed request gives its slot back");
        sessions.insert_transcode(session(1, None, true), first);
        assert_eq!(sessions.active_transcodes(), 1);
        assert!(sessions.reserve_transcode(2).is_none(), "the session keeps the slot it took over");
        drop(third);
        assert!(sessions.reserve_transcode(2).is_some());
        assert!(sessions.reserve_transcode(1).is_none(), "a lower limit counts the live session");
    }

    #[test]
    fn bit_depths_and_frame_rates_for_version_2_decisions() {
        let stream = |bit_depth, pix_fmt: Option<&str>, profile: Option<&str>| MediaStream {
            bit_depth,
            pix_fmt: pix_fmt.map(str::to_owned),
            profile: profile.map(str::to_owned),
            ..Default::default()
        };
        assert_eq!(bit_depth(&stream(Some(12), Some("yuv420p10le"), None)), 12, "the stream says");
        assert_eq!(bit_depth(&stream(None, Some("yuv420p10le"), None)), 10);
        assert_eq!(bit_depth(&stream(None, Some("p010le"), None)), 10);
        assert_eq!(bit_depth(&stream(None, Some("yuv444p12be"), None)), 12);
        assert_eq!(bit_depth(&stream(None, Some("yuv420p"), Some("Main 10"))), 10);
        assert_eq!(bit_depth(&stream(None, Some("yuv420p"), Some("High"))), 8);
        assert_eq!(bit_depth(&stream(None, None, None)), 8);
        assert_eq!(trim_fps(24_000.0 / 1001.0), "23.976");
        assert_eq!(trim_fps(25.0), "25");
        assert_eq!(trim_fps(59.94), "59.94");
        assert_eq!(closest_quality_within(Some(3840), Some(2160), Some(57_000_000), Some(1080)), ("20m", 20_000, 1080));
        assert_eq!(closest_quality_within(Some(1280), Some(720), None, Some(1080)).2, 720, "never above the file");
        assert_eq!(closest_quality_within(Some(3840), Some(2160), None, Some(600)).2, 480);
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
