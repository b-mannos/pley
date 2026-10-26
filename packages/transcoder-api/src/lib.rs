//! Wire protocol between the pley media server and a pley transcoder node.
//!
//! The transcoder is a separate program that owns every
//! interaction with FFmpeg. The server talks to it over plain HTTP + JSON using
//! the types in this crate, so that both
//! sides can use it. See `docs/TRANSCODER.md` for the endpoint list.
//!
//! Conventions: all timestamps are unix **milliseconds** (`*_ms`), durations are
//! milliseconds, stream indexes are FFmpeg input stream indexes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Protocol version spoken by this crate. Bumped on breaking changes.
pub const API_VERSION: u32 = 1;

/// Environment variable carrying the shared secret when the server spawns a
/// managed transcoder.
pub const TOKEN_ENV: &str = "PLEY_TRANSCODER_TOKEN";

/// Line printed on stdout by `pley-transcoder --announce` once it is listening.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announce {
    pub event: String,
    pub addr: String,
}

// ---------------------------------------------------------------------------
// Media information
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MediaInfo {
    /// Normalised container name: `mkv`, `mp4`, `webm`, `avi`, `ts`, `mp3`, `flac`, …
    pub container: Option<String>,
    /// Raw FFmpeg demuxer name(s), e.g. `matroska,webm`.
    pub format_name: Option<String>,
    pub duration_ms: Option<i64>,
    /// Container start time (ms, may be negative). FFmpeg's `-ss` and the
    /// output timeline are relative to it, packet timestamps are not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<i64>,
    /// Overall bitrate in bits per second.
    pub bitrate: Option<i64>,
    pub streams: Vec<MediaStream>,
    /// Container level tags with lowercase keys (`title`, `artist`, `album`, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<Chapter>,
    /// An embedded cover picture exists (MP3/M4A/FLAC artwork, MP4 cover).
    #[serde(default)]
    pub has_cover_art: bool,
}

impl MediaInfo {
    pub fn video(&self) -> Option<&MediaStream> {
        self.streams.iter().find(|s| s.kind == StreamKind::Video)
    }

    pub fn default_audio(&self) -> Option<&MediaStream> {
        let mut audio = self.streams.iter().filter(|s| s.kind == StreamKind::Audio);
        let first = audio.clone().next();
        audio.find(|s| s.default).or(first)
    }

    pub fn stream(&self, index: i64) -> Option<&MediaStream> {
        self.streams.iter().find(|s| s.index == index)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Chapter {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    #[default]
    Video,
    Audio,
    Subtitle,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRange {
    #[default]
    Sdr,
    Hdr10,
    Hdr10plus,
    Hlg,
    DolbyVision,
}

impl DynamicRange {
    pub fn is_hdr(self) -> bool {
        !matches!(self, DynamicRange::Sdr)
    }

    pub fn label(self) -> &'static str {
        match self {
            DynamicRange::Sdr => "SDR",
            DynamicRange::Hdr10 => "HDR10",
            DynamicRange::Hdr10plus => "HDR10+",
            DynamicRange::Hlg => "HLG",
            DynamicRange::DolbyVision => "Dolby Vision",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MediaStream {
    /// FFmpeg stream index inside the file. External subtitle files get 1000+.
    pub index: i64,
    pub kind: StreamKind,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,

    // ---- video ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pix_fmt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interlaced: Option<bool>,
    /// Frames are stored out of order (B-frames). FFmpeg then seeks a little
    /// before the point asked for in containers that don't seek by
    /// presentation time (Matroska, MPEG-TS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_b_frames: Option<bool>,
    /// The frames are stored out of display order (B-frames), but the file
    /// gives each one the time it is decoded at as the time it is shown (an
    /// MP4 muxed from a raw stream, without composition offsets). Players
    /// that trust those times show frames out of order or drop them, and
    /// copying the stream keeps the wrong times: only decoding it puts them
    /// right.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decode_order_times: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_range: Option<DynamicRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dv_profile: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dv_level: Option<i64>,
    /// Dolby Vision base-layer signal compatibility id (0 none, 1 HDR10, 2 SDR, 4 HLG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dv_bl_compat: Option<i64>,
    /// HDR10+ dynamic metadata (SMPTE ST 2094-40) in the video's first
    /// frames. Looked for in Dolby Vision streams only: `true` is a Dolby
    /// Vision + HDR10+ stream, which some Dolby Vision decoders fail on.
    /// `None`: not looked for (other streams, or probed by an older
    /// transcoder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr10plus: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_transfer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_primaries: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_space: Option<String>,

    // ---- audio ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i64>,

    // ---- subtitle ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_based: Option<bool>,
    /// Set by the server for sidecar subtitle files; never by the transcoder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
}

/// "Dolby Vision P8.1", "HDR10", … for display.
pub fn dynamic_range_label(range: DynamicRange, dv_profile: Option<i64>, dv_bl_compat: Option<i64>) -> String {
    match (range, dv_profile) {
        (DynamicRange::DolbyVision, Some(p)) => match dv_bl_compat {
            Some(c @ (1 | 2 | 4)) if p >= 8 => format!("Dolby Vision P{p}.{c}"),
            _ => format!("Dolby Vision P{p}"),
        },
        _ => range.label().to_owned(),
    }
}

impl MediaStream {
    pub fn dynamic_range(&self) -> DynamicRange {
        self.dynamic_range.unwrap_or_default()
    }

    pub fn dynamic_range_label(&self) -> String {
        dynamic_range_label(self.dynamic_range(), self.dv_profile, self.dv_bl_compat)
    }

    /// Dolby Vision without a backward-compatible base layer (profile 5, or
    /// 7/8 with compatibility id 0): only DV-capable decoders show it correctly.
    pub fn is_dolby_vision_only(&self) -> bool {
        self.dynamic_range() == DynamicRange::DolbyVision
            && (self.dv_profile == Some(5) || self.dv_bl_compat == Some(0))
    }

    pub fn is_text_subtitle(&self) -> bool {
        self.kind == StreamKind::Subtitle && self.text_based.unwrap_or(false)
    }
}

/// Subtitle codecs that are text (can be converted to WebVTT / rendered with libass).
pub fn is_text_subtitle_codec(codec: &str) -> bool {
    matches!(
        codec,
        "subrip"
            | "srt"
            | "ass"
            | "ssa"
            | "webvtt"
            | "mov_text"
            | "text"
            | "microdvd"
            | "subviewer"
            | "subviewer1"
            | "sami"
            | "jacosub"
            | "realtext"
            | "stl"
            | "mpl2"
            | "pjs"
            | "vplayer"
    )
}

// ---------------------------------------------------------------------------
// Capabilities, limits, status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum HwAccel {
    #[default]
    Auto,
    None,
    Nvenc,
    Qsv,
    Amf,
    Vaapi,
    Videotoolbox,
    /// Vulkan Video (`h264_vulkan`), used for fully-GPU tone mapping pipelines.
    Vulkan,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Tonemapper {
    /// GPU (Vulkan) via libplacebo. Handles Dolby Vision profile 5 reshaping.
    Libplacebo,
    /// GPU via OpenCL (`tonemap_opencl`).
    Opencl,
    /// CPU via zimg (`zscale` + `tonemap`).
    Zscale,
}

impl Tonemapper {
    pub fn as_str(self) -> &'static str {
        match self {
            Tonemapper::Libplacebo => "libplacebo",
            Tonemapper::Opencl => "opencl",
            Tonemapper::Zscale => "zscale",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TonemapMode {
    #[default]
    Auto,
    Libplacebo,
    Opencl,
    Zscale,
    None,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    #[default]
    H264,
    Hevc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EncoderInfo {
    /// FFmpeg encoder name, e.g. `h264_nvenc`.
    pub name: String,
    pub codec: VideoCodec,
    pub hardware: bool,
    pub accel: HwAccel,
    /// An HEVC encoder that passed a 10-bit (Main 10) test encode too: what
    /// transcodes that keep HDR need.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main10: bool,
}

/// A hardware pipeline that keeps frames in GPU memory from decode to encode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuPipeline {
    pub accel: HwAccel,
    /// Input codec keys: `h264`, `hevc`, `hevc10`, `av1`, `vp9`, `mpeg2video`.
    pub codecs: Vec<String>,
}

/// Codec key used for hardware decode capability checks (`hevc10` for 10-bit HEVC).
pub fn decode_key(codec: &str, bit_depth: Option<i64>) -> String {
    match (codec, bit_depth.unwrap_or(8)) {
        ("hevc", d) if d > 8 => "hevc10".into(),
        ("h264", d) if d > 8 => "h264-10".into(),
        ("vp9", d) if d > 8 => "vp9-10".into(),
        (c, _) => c.to_owned(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Capabilities {
    pub ffmpeg_available: bool,
    pub ffprobe_available: bool,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub ffmpeg_version: Option<String>,
    /// Video encoders that passed a test encode, best first.
    pub encoders: Vec<EncoderInfo>,
    /// Decoding hwaccels reported by `ffmpeg -hwaccels`.
    pub hwaccels: Vec<String>,
    /// Working tone mappers, best first.
    pub tonemappers: Vec<Tonemapper>,
    /// Dolby Vision profile 5 can be rendered correctly (requires libplacebo).
    pub dolby_vision: bool,
    /// Codecs the GPU decodes straight into Vulkan memory, enabling a
    /// zero-copy decode → libplacebo tone mapping pipeline.
    #[serde(default)]
    pub vulkan_decode: Vec<String>,
    /// `h264_vulkan` encodes libplacebo output without leaving the GPU.
    #[serde(default)]
    pub vulkan_encode: bool,
    /// Zero-copy decode → scale → encode pipelines that passed a self-test,
    /// with the input codecs each one handles.
    #[serde(default)]
    pub gpu_pipelines: Vec<GpuPipeline>,
    pub subtitle_burn_image: bool,
    pub subtitle_burn_text: bool,
    /// `-readrate`/`-readrate_initial_burst` are supported (transcode throttling).
    pub readrate: bool,
    /// FFmpeg's `chromaprint` muxer is available: audio fingerprints for
    /// intro and credits detection (`POST /v1/fingerprint`).
    #[serde(default)]
    pub chromaprint: bool,
    /// Jobs have a multivariant playlist (`master.m3u8`) naming their
    /// codecs, and remuxes do what [`VideoRequest::dolby_vision`] and
    /// [`VideoRequest::strip_hdr10plus`] ask. Older transcoders lack both.
    #[serde(default)]
    pub hls_master_playlist: bool,
    /// Transcodes of HDR video can keep it, as HEVC Main 10 in HDR10 or HLG
    /// ([`VideoRequest::dynamic_range`]): a 10-bit HEVC encoder works. Older
    /// transcoders lack it and always tone-map to SDR.
    #[serde(default)]
    pub hdr_transcode: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Limits {
    pub max_jobs: u32,
    pub max_hw_jobs: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TranscoderInfo {
    pub name: String,
    pub version: String,
    pub api_version: u32,
    /// SPDX license of the transcoder program.
    pub license: String,
    /// Where the source code can be obtained.
    pub source: Option<String>,
    pub started_at_ms: i64,
    pub capabilities: Capabilities,
    pub limits: Limits,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    pub jobs_created: u64,
    pub jobs_failed: u64,
    pub processes_started: u64,
    pub process_failures: u64,
    pub segments_produced: u64,
    pub segments_served: u64,
    pub bytes_served: u64,
    pub probes: u64,
    pub probe_failures: u64,
    pub probe_ms_total: u64,
    pub thumbnails: u64,
    pub subtitle_extractions: u64,
    pub keyframe_scans: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TranscoderStatus {
    pub version: String,
    pub uptime_secs: u64,
    pub ffmpeg_version: Option<String>,
    pub max_jobs: u32,
    pub running_processes: u32,
    pub hw_processes: u32,
    pub max_hw_jobs: u32,
    pub work_dir_bytes: u64,
    pub counters: Counters,
    pub jobs: Vec<JobStatus>,
}

// ---------------------------------------------------------------------------
// Jobs (HLS sessions)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VideoMode {
    /// Stream copy (remux). Falls back to `Transcode` when the stream cannot be
    /// segmented on keyframes yet; the effective mode is reported in the plan.
    Copy,
    #[default]
    Transcode,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AudioMode {
    Copy,
    #[default]
    Transcode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VideoRequest {
    /// Video stream to use (default: first video stream).
    #[serde(default)]
    pub stream_index: Option<i64>,
    #[serde(default)]
    pub mode: VideoMode,
    #[serde(default)]
    pub codec: VideoCodec,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default)]
    pub max_height: Option<u32>,
    /// Target video bitrate cap in kbit/s (default chosen from the output resolution).
    #[serde(default)]
    pub max_bitrate_kbps: Option<u32>,
    /// What a remux does with Dolby Vision (ignored when transcoding, and
    /// for other streams).
    #[serde(default)]
    pub dolby_vision: DolbyVisionMode,
    /// A remux leaves out the HDR10+ metadata (SMPTE ST 2094-40 SEI
    /// messages) of an HEVC stream, for Dolby Vision decoders that fail on
    /// streams carrying both (androidx/media#1895).
    #[serde(default)]
    pub strip_hdr10plus: bool,
    /// The dynamic range a transcode of HDR video is made in (ignored for
    /// SDR video and remuxes). HDR10 and HLG need `codec` HEVC: the
    /// transcode is then HEVC Main 10 with the HDR10 metadata of the source
    /// kept (Dolby Vision's left out); video in the other HDR format is
    /// converted with libplacebo (HLG or Dolby Vision profile 5 to HDR10, say).
    /// Where that can't be done (no 10-bit HEVC encoder, no libplacebo for a
    /// conversion, subtitles drawn onto the video) it is tone-mapped to SDR,
    /// with a warning.
    #[serde(default)]
    pub dynamic_range: OutputRange,
}

/// The dynamic range of a transcode of HDR video.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum OutputRange {
    /// Tone-mapped to SDR: what every player shows.
    #[default]
    Sdr,
    /// PQ, BT.2020 (HDR10 static metadata kept when the source has it).
    Hdr10,
    /// Hybrid log-gamma, BT.2020.
    Hlg,
}

impl OutputRange {
    /// The range as [`DynamicRange`] names it.
    pub fn dynamic_range(self) -> DynamicRange {
        match self {
            OutputRange::Sdr => DynamicRange::Sdr,
            OutputRange::Hdr10 => DynamicRange::Hdr10,
            OutputRange::Hlg => DynamicRange::Hlg,
        }
    }
}

/// What a remux does with a Dolby Vision stream. Whatever it is, the
/// stream's Dolby Vision metadata (RPU) stays in the MPEG-TS segments,
/// which can't say what they carry: the master playlist's `CODECS` does.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DolbyVisionMode {
    /// Copied as it is and not named: the base layer's codec only
    /// (`hvc1…`), what a player without Dolby Vision decodes.
    #[default]
    Copy,
    /// Copied as it is and named (`dvh1.05.06`; `hvc1…` with
    /// `SUPPLEMENTAL-CODECS="dvh1.08.06/db1p"` for profile 8): for players
    /// that decode the profile.
    Signal,
    /// Profile 7 made profile 8.1 and named so: the enhancement layer left
    /// out and the RPU rewritten (`dovi_tool`'s mode 2). Other profiles are
    /// signalled as they are.
    To81,
}

/// The codec converted audio is encoded to.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    /// AAC-LC: every player decodes it.
    #[default]
    Aac,
    /// Dolby Digital, up to 5.1 at 640 kbit/s.
    Ac3,
    /// Dolby Digital Plus (E-AC-3), for players that decode it or pass it
    /// through to a receiver.
    Eac3,
}

impl AudioCodec {
    /// FFmpeg's name for the codec (and its encoder).
    pub fn as_str(self) -> &'static str {
        match self {
            AudioCodec::Aac => "aac",
            AudioCodec::Ac3 => "ac3",
            AudioCodec::Eac3 => "eac3",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioRequest {
    /// Audio stream to use (default: the default audio stream).
    #[serde(default)]
    pub stream_index: Option<i64>,
    #[serde(default)]
    pub mode: AudioMode,
    /// Codec when transcoding (default AAC).
    #[serde(default)]
    pub codec: AudioCodec,
    /// Output channels when transcoding (default 2), never more than the
    /// source has.
    #[serde(default)]
    pub channels: Option<u32>,
    /// Default: 192 kbit/s for stereo AAC, 64 per channel for more; 640 for
    /// AC-3 and E-AC-3 above stereo.
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubtitleBurnRequest {
    /// Embedded subtitle stream to burn in.
    #[serde(default)]
    pub stream_index: Option<i64>,
    /// Sidecar subtitle file (`.srt`, `.ass`, `.sup`, …) to burn in instead.
    #[serde(default)]
    pub external_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscodeOptions {
    #[serde(default)]
    pub hwaccel: HwAccel,
    #[serde(default = "default_true")]
    pub hardware_decoding: bool,
    #[serde(default)]
    pub tonemap: TonemapMode,
    /// x264 preset for software encodes (`veryfast` by default), or `auto`:
    /// chosen by the transcoder from its load and how far ahead of the player
    /// it is, and changed as they change.
    #[serde(default)]
    pub preset: Option<String>,
    /// Limit ffmpeg to ~2× realtime once a buffer has been built.
    #[serde(default = "default_true")]
    pub throttle: bool,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            hwaccel: HwAccel::Auto,
            hardware_decoding: true,
            tonemap: TonemapMode::Auto,
            preset: None,
            throttle: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct JobRequest {
    /// Absolute path of the input file, as seen by the transcoder.
    pub input: String,
    /// Media information the caller already has (skips a probe). Ignored when it
    /// does not contain a duration.
    #[serde(default)]
    pub media: Option<MediaInfo>,
    /// Human readable label for status pages, e.g. `"The Matrix (1999)"`.
    #[serde(default)]
    pub label: Option<String>,
    /// Who the job is for (username), for status pages.
    #[serde(default)]
    pub owner: Option<String>,
    /// `None` = drop video (audio-only output).
    #[serde(default)]
    pub video: Option<VideoRequest>,
    /// `None` = drop audio.
    #[serde(default)]
    pub audio: Option<AudioRequest>,
    #[serde(default)]
    pub subtitle: Option<SubtitleBurnRequest>,
    /// Target segment length (default 4 s).
    #[serde(default)]
    pub segment_seconds: Option<f64>,
    /// Start transcoding at this position right away (playback start / resume).
    #[serde(default)]
    pub start_ms: Option<i64>,
    #[serde(default)]
    pub options: TranscodeOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VideoPlan {
    /// Effective mode (a requested copy may fall back to transcode).
    pub mode: VideoMode,
    pub codec_in: String,
    pub codec_out: String,
    pub width_in: Option<i64>,
    pub height_in: Option<i64>,
    pub width_out: Option<i64>,
    pub height_out: Option<i64>,
    pub dynamic_range_in: DynamicRange,
    /// What a transcode comes out as: SDR (tone-mapped, or SDR already),
    /// HDR10 or HLG. `None` for remuxes (and from older transcoders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_range_out: Option<DynamicRange>,
    pub dv_profile: Option<i64>,
    /// Dolby Vision base-layer compatibility id (1 HDR10, 2 SDR, 4 HLG; 0 none).
    #[serde(default)]
    pub dv_bl_compat: Option<i64>,
    pub tonemap: Option<Tonemapper>,
    pub encoder: Option<String>,
    /// `cpu`, `cuda`, `qsv`, `vaapi`, `videotoolbox`, `d3d11va`, `vulkan`, …
    pub decoder: Option<String>,
    /// `gpu` (frames never leave the GPU), `hybrid` (GPU decode/filter with a
    /// copy to system memory), `cpu`, or `copy`.
    #[serde(default)]
    pub pipeline: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub subtitle_burn: Option<String>,
    /// What the remux does with Dolby Vision; `None` when the output carries
    /// none (a transcode, or another stream).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dolby_vision: Option<DolbyVisionMode>,
    /// The remux leaves out the stream's HDR10+ metadata.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hdr10plus_stripped: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioPlan {
    pub mode: AudioMode,
    pub codec_in: String,
    pub codec_out: String,
    pub channels_in: Option<i64>,
    pub channels_out: Option<i64>,
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct JobCreated {
    pub id: String,
    pub duration_ms: i64,
    pub segment_seconds: f64,
    pub segment_count: u32,
    pub video: Option<VideoPlan>,
    pub audio: Option<AudioPlan>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    #[default]
    Starting,
    /// An ffmpeg process is producing segments.
    Running,
    /// No process running (paused client, or everything requested is ready).
    Idle,
    /// The whole file has been produced.
    Finished,
    /// The last process failed; see `last_error`.
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessStatus {
    pub pid: Option<u32>,
    pub start_segment: u32,
    pub started_at_ms: i64,
    pub frame: Option<u64>,
    pub fps: Option<f64>,
    pub speed: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    /// Absolute media position (ms) that ffmpeg has produced up to.
    pub out_time_ms: i64,
    pub total_size: Option<u64>,
    pub dup_frames: Option<u64>,
    pub drop_frames: Option<u64>,
    pub hardware: bool,
    /// The software encoder's preset (`veryfast`…), when the video is
    /// encoded in software.
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct JobStatus {
    pub id: String,
    pub label: Option<String>,
    pub owner: Option<String>,
    pub input: String,
    pub state: JobState,
    pub created_at_ms: i64,
    pub last_access_ms: i64,
    pub duration_ms: i64,
    pub segment_seconds: f64,
    pub segment_count: u32,
    pub segments_ready: u32,
    pub video: Option<VideoPlan>,
    pub audio: Option<AudioPlan>,
    pub process: Option<ProcessStatus>,
    pub restarts: u32,
    pub served_segments: u64,
    pub served_bytes: u64,
    pub last_error: Option<String>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// One-shot extraction requests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProbeRequest {
    pub path: String,
}

/// `POST /v1/thumbnail` → `image/jpeg`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThumbnailRequest {
    pub path: String,
    /// Position of the frame (default: 20 % into the file).
    #[serde(default)]
    pub time_ms: Option<i64>,
    /// Output width in pixels (default 640).
    #[serde(default)]
    pub width: Option<u32>,
    /// Duration hint used to compute the default position.
    #[serde(default)]
    pub duration_ms: Option<i64>,
    /// Tone-map HDR sources so frame grabs are not washed out.
    #[serde(default)]
    pub hdr: bool,
}

/// `POST /v1/cover` → embedded cover art bytes (`image/jpeg` or `image/png`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CoverRequest {
    pub path: String,
}

/// `POST /v1/subtitles` → `text/vtt`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SubtitleRequest {
    pub path: String,
    /// Embedded stream index; `None` means `path` is itself a subtitle file.
    #[serde(default)]
    pub stream_index: Option<i64>,
    /// For an embedded track: answer within a few seconds, with the cues
    /// found so far when the track isn't all out by then, rather than wait
    /// for the whole file; `X-Pley-Complete: 0` says more are coming.
    #[serde(default)]
    pub partial: bool,
}

/// Header of a `partial` subtitles answer: `1` when the track is complete.
pub const SUBTITLES_COMPLETE_HEADER: &str = "x-pley-complete";

/// `POST /v1/keyframes` → `202` (index is built in the background so later
/// copy-mode jobs can be segmented exactly on keyframes).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct KeyframesRequest {
    pub path: String,
}

/// `POST /v1/prefetch` → [`Prefetched`]: read the parts of a file a playback
/// starting at `start_ms` reads first (its head, its index, the keyframe
/// before the start and a few seconds on), so that they come from a cache
/// when it does: worth seconds on a network share.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PrefetchRequest {
    pub path: String,
    /// Where the playback would start (ms); 0 or none: the beginning.
    #[serde(default)]
    pub start_ms: Option<i64>,
}

/// What a prefetch read.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Prefetched {
    /// The byte ranges read, in the order they were read.
    pub ranges: Vec<PrefetchedRange>,
    /// Bytes read in all.
    pub bytes: u64,
    pub elapsed_ms: u64,
    /// Why nothing was read: a playback is starting, or the file is being
    /// read already.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Stopped before the end: a playback started, or time ran out.
    #[serde(default)]
    pub stopped: bool,
}

/// A byte range a prefetch read: `part` is `head`, `index` (Matroska Cues or
/// MP4 movie box) or `start` (where the playback starts).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PrefetchedRange {
    pub part: String,
    pub start: u64,
    /// Exclusive.
    pub end: u64,
}

/// `POST /v1/fingerprint` → [`Fingerprint`]: a Chromaprint fingerprint of the
/// audio in part of a file. The server compares the fingerprints of episodes
/// to find the intro and credits they share.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FingerprintRequest {
    pub path: String,
    /// Audio stream to analyse (default: the first audio stream).
    #[serde(default)]
    pub stream_index: Option<i64>,
    /// Where to start, ms from the beginning of the file.
    #[serde(default)]
    pub start_ms: i64,
    /// How much audio to analyse; the end of the file may stop it earlier.
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Fingerprint {
    /// Position of the first value, ms from the beginning of the file.
    pub start_ms: i64,
    /// Seconds between consecutive values (1365 samples at 11025 Hz).
    pub item_seconds: f64,
    /// Raw Chromaprint values of the default algorithm, one per `item_seconds`.
    pub data: Vec<u32>,
}

/// `POST /v1/frames` → [`Frames`]: how dark the key frames of part of a video
/// are and how much contrast they have. The server looks for credits in
/// them: text on black.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FramesRequest {
    pub path: String,
    /// Where to start, ms from the beginning of the file.
    #[serde(default)]
    pub start_ms: i64,
    /// How much video to look at; the end of the file may stop it earlier.
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Frames {
    /// Key frames in order of time.
    pub frames: Vec<FrameStats>,
}

/// One key frame, its luma in 8-bit studio range (black is 16).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct FrameStats {
    /// Ms from the beginning of the file.
    pub ms: i64,
    /// Share of pixels at most 32 (near black), in percent.
    pub black: f32,
    /// Luma of the darkest and brightest 10 % of pixels, and the extremes.
    pub ylow: f32,
    pub yhigh: f32,
    pub ymax: f32,
    /// Average saturation.
    pub sat: f32,
}

/// `POST /v1/trickplay`: seek-bar preview thumbnails of a video, tiled into
/// JPEG sprite sheets fetched with `GET /v1/trickplay/{id}/{n}`. Cached until
/// every sheet has been fetched (callers keep their own copy), or for a day:
/// asking again for the same file version meanwhile returns at once.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrickplayRequest {
    pub path: String,
    /// Time between thumbnails (default: 2–10 s depending on the length).
    #[serde(default)]
    pub interval_ms: Option<i64>,
    /// Thumbnail width in pixels (default 320).
    #[serde(default)]
    pub width: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrickplayInfo {
    /// Identifies the sheets on the transcoder.
    pub id: String,
    pub interval_ms: i64,
    /// Size of one thumbnail.
    pub width: u32,
    pub height: u32,
    /// Thumbnails per sheet: `columns × rows`, left to right, top to bottom.
    pub columns: u32,
    pub rows: u32,
    /// Number of thumbnails; thumbnail `k` shows the video at `k × interval_ms`.
    pub count: u32,
    /// Number of sheets, `0.jpg` to `{sheets - 1}.jpg`.
    pub sheets: u32,
}

/// Error body returned by the transcoder for non-2xx responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_request_defaults_are_lenient() {
        let req: JobRequest =
            serde_json::from_str(r#"{"input":"/m/a.mkv","video":{},"audio":{"mode":"copy"}}"#).unwrap();
        assert_eq!(req.video.as_ref().unwrap().mode, VideoMode::Transcode);
        let audio = req.audio.unwrap();
        assert_eq!((audio.mode, audio.codec), (AudioMode::Copy, AudioCodec::Aac), "AAC unless asked");
        let audio: AudioRequest = serde_json::from_str(r#"{"codec":"eac3","channels":6}"#).unwrap();
        assert_eq!((audio.codec, audio.codec.as_str()), (AudioCodec::Eac3, "eac3"));
        assert!(req.options.hardware_decoding);
        assert!(req.options.throttle);
        assert_eq!(req.options.hwaccel, HwAccel::Auto);
        // Dolby Vision copied as it is unless asked.
        let video = req.video.unwrap();
        assert_eq!((video.dolby_vision, video.strip_hdr10plus), (DolbyVisionMode::Copy, false));
        let video: VideoRequest =
            serde_json::from_str(r#"{"mode":"copy","dolby_vision":"to81","strip_hdr10plus":true}"#).unwrap();
        assert_eq!((video.dolby_vision, video.strip_hdr10plus), (DolbyVisionMode::To81, true));
        assert_eq!(serde_json::to_value(DolbyVisionMode::Signal).unwrap(), "signal");
        // Transcodes are SDR unless asked.
        assert_eq!(video.dynamic_range, OutputRange::Sdr);
        let video: VideoRequest = serde_json::from_str(r#"{"codec":"hevc","dynamic_range":"hdr10"}"#).unwrap();
        assert_eq!((video.codec, video.dynamic_range), (VideoCodec::Hevc, OutputRange::Hdr10));
        assert_eq!(OutputRange::Hlg.dynamic_range(), DynamicRange::Hlg);
        // Encoders and capabilities from older transcoders.
        let enc: EncoderInfo =
            serde_json::from_str(r#"{"name":"hevc_nvenc","codec":"hevc","hardware":true,"accel":"nvenc"}"#).unwrap();
        assert!(!enc.main10);
        let caps: Capabilities = serde_json::from_str(
            r#"{"ffmpeg_available":true,"ffprobe_available":true,"ffmpeg_path":"","ffprobe_path":"",
                "ffmpeg_version":null,"encoders":[],"hwaccels":[],"tonemappers":[],"dolby_vision":false,
                "subtitle_burn_image":false,"subtitle_burn_text":false,"readrate":false}"#,
        )
        .unwrap();
        assert!(!caps.hdr_transcode);
    }

    #[test]
    fn media_stream_round_trip_skips_empty_fields() {
        let s = MediaStream {
            index: 1,
            kind: StreamKind::Audio,
            codec: "aac".into(),
            channels: Some(2),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("width"));
        assert!(!json.contains("external"));
        let back: MediaStream = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn dolby_vision_labels() {
        assert_eq!(dynamic_range_label(DynamicRange::DolbyVision, Some(8), Some(1)), "Dolby Vision P8.1");
        assert_eq!(dynamic_range_label(DynamicRange::DolbyVision, Some(8), Some(4)), "Dolby Vision P8.4");
        assert_eq!(dynamic_range_label(DynamicRange::DolbyVision, Some(5), Some(0)), "Dolby Vision P5");
        assert_eq!(dynamic_range_label(DynamicRange::Hdr10, None, None), "HDR10");
        let p5 =
            MediaStream { dynamic_range: Some(DynamicRange::DolbyVision), dv_profile: Some(5), ..Default::default() };
        assert!(p5.is_dolby_vision_only());
        let p81 = MediaStream {
            dynamic_range: Some(DynamicRange::DolbyVision),
            dv_profile: Some(8),
            dv_bl_compat: Some(1),
            ..Default::default()
        };
        assert!(!p81.is_dolby_vision_only());
    }

    #[test]
    fn default_audio_prefers_flagged_stream() {
        let info = MediaInfo {
            streams: vec![
                MediaStream { index: 0, kind: StreamKind::Video, codec: "h264".into(), ..Default::default() },
                MediaStream { index: 1, kind: StreamKind::Audio, codec: "aac".into(), ..Default::default() },
                MediaStream {
                    index: 2,
                    kind: StreamKind::Audio,
                    codec: "ac3".into(),
                    default: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(info.default_audio().unwrap().index, 2);
    }
}
