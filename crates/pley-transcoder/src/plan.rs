// SPDX-License-Identifier: LicenseRef-Restricted
//! Turns a [`JobRequest`] plus [`MediaInfo`] into a concrete plan (segment
//! layout, encoder, filters, tone mapping) and renders ffmpeg command lines for
//! any window of segments.
//!
//! Segment alignment: every process is started with `-ss <boundary>` and
//! `-output_ts_offset <boundary>`, so segment timestamps stay on the absolute
//! timeline no matter where ffmpeg was (re)started. Split points are passed via
//! the segment muxer's `-segment_times`; when transcoding, keyframes are forced
//! on the same grid, when remuxing the grid *is* the source keyframes.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pley_transcoder_api::{
    AudioMode, AudioPlan, DynamicRange, EncoderInfo, HwAccel, JobRequest, MediaInfo, MediaStream, StreamKind,
    Tonemapper, VideoCodec, VideoMode, VideoPlan, is_text_subtitle_codec,
};

use crate::ffmpeg::{Ffmpeg, VAAPI_DEVICE};
use crate::util::{input_arg, secs};

pub const DEFAULT_SEGMENT_SECONDS: f64 = 4.0;
/// Maximum segments per ffmpeg process (keeps command lines short on Windows).
pub const PROCESS_WINDOW: u32 = 1200;
/// Throttling: read at 2× realtime after a 60 s burst.
const READRATE: &str = "2";
const READRATE_BURST: &str = "60";

/// CPU tone mapping chain (zimg). Expects HDR input, outputs 8-bit BT.709.
pub const ZSCALE_TONEMAP: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";
const OPENCL_TONEMAP: &str = "format=p010,hwupload,tonemap_opencl=tonemap=hable:desat=0:t=bt709:m=bt709:p=bt709:format=nv12,hwdownload,format=nv12";

#[derive(Debug, Clone)]
pub struct VideoOut {
    pub stream_index: i64,
    pub copy: bool,
    pub encoder: Option<EncoderInfo>,
    pub codec: VideoCodec,
    pub src_width: i64,
    pub src_height: i64,
    pub width: i64,
    pub height: i64,
    pub bitrate_kbps: u32,
    pub tonemap: Option<Tonemapper>,
    pub hw_decode: Option<String>,
    pub deinterlace: bool,
    pub preset: String,
}

#[derive(Debug, Clone)]
pub struct AudioOut {
    pub stream_index: i64,
    pub copy: bool,
    pub channels: u32,
    pub bitrate_kbps: u32,
    pub resample: bool,
}

#[derive(Debug, Clone)]
pub enum Burn {
    /// Bitmap subtitle stream of the input (PGS, VobSub, DVB).
    Image { stream_index: i64, label: String },
    /// Bitmap subtitle file (`.sup`), added as a second input.
    ImageFile { path: PathBuf, label: String },
    /// Text subtitles rendered by libass from `file_name` inside the job dir.
    Text { source: TextSource, file_name: String, label: String },
}

#[derive(Debug, Clone)]
pub enum TextSource {
    Embedded(i64),
    File(PathBuf),
}

impl Burn {
    pub fn label(&self) -> &str {
        match self {
            Burn::Image { label, .. } | Burn::ImageFile { label, .. } | Burn::Text { label, .. } => label,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub duration_ms: i64,
    /// Segment start times in ms; `boundaries[segment_count]` is the duration.
    pub boundaries: Vec<i64>,
    /// Absolute split times passed to the segment muxer (slightly before each
    /// boundary to absorb timestamp jitter). `split_at[k]` belongs to boundary k.
    pub split_at: Vec<i64>,
    pub segment_seconds: f64,
    pub video: Option<VideoOut>,
    pub audio: Option<AudioOut>,
    pub burn: Option<Burn>,
    pub throttle: bool,
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn segment_count(&self) -> u32 {
        (self.boundaries.len() - 1) as u32
    }

    pub fn segment_for(&self, position_ms: i64) -> u32 {
        let idx = self.boundaries.partition_point(|&b| b <= position_ms);
        (idx.saturating_sub(1) as u32).min(self.segment_count().saturating_sub(1))
    }

    pub fn build(req: &JobRequest, media: &MediaInfo, ff: &Ffmpeg, keyframes: Option<&[i64]>) -> Result<Plan, String> {
        let duration_ms = media.duration_ms.filter(|d| *d > 0).ok_or("media duration is unknown")?;
        let segment_seconds = req.segment_seconds.unwrap_or(DEFAULT_SEGMENT_SECONDS).clamp(1.0, 10.0);
        let target_ms = (segment_seconds * 1000.0).round() as i64;
        let mut warnings = Vec::new();

        // ---- subtitle burn-in ----
        let burn = match &req.subtitle {
            None => None,
            Some(sub) => Some(resolve_burn(sub, media, ff)?),
        };

        // ---- video ----
        let video = match (&req.video, media_video(media, req)) {
            (Some(vreq), Some(src)) => {
                let src_width = src.width.unwrap_or(1920);
                let src_height = src.height.unwrap_or(1080);
                let want_copy = vreq.mode == VideoMode::Copy && burn.is_none();
                let copy = want_copy && keyframes.is_some_and(|k| !k.is_empty());
                if want_copy && !copy {
                    warnings.push("keyframe index not ready yet: transcoding instead of remuxing".into());
                }
                if copy {
                    Some(VideoOut {
                        stream_index: src.index,
                        copy: true,
                        encoder: None,
                        codec: vreq.codec,
                        src_width,
                        src_height,
                        width: src_width,
                        height: src_height,
                        bitrate_kbps: 0,
                        tonemap: None,
                        hw_decode: None,
                        deinterlace: false,
                        preset: String::new(),
                    })
                } else {
                    let (width, height) = fit(src_width, src_height, vreq.max_width, vreq.max_height);
                    let mut bitrate = default_bitrate_kbps(height);
                    if let Some(max) = vreq.max_bitrate_kbps {
                        bitrate = bitrate.min(max.max(300));
                    }
                    let (encoder, warning) = ff.pick_encoder(vreq.codec, req.options.hwaccel);
                    warnings.extend(warning);
                    let encoder = encoder.cloned().ok_or("no working video encoder found (is ffmpeg installed?)")?;
                    let range = src.dynamic_range();
                    let tonemap = if range.is_hdr() {
                        let p5 = range == DynamicRange::DolbyVision
                            && (src.dv_profile == Some(5) || src.dv_bl_compat == Some(0));
                        let (choice, w) = ff.pick_tonemapper(req.options.tonemap, p5);
                        warnings.extend(w);
                        choice
                    } else {
                        None
                    };
                    let hw_decode = if req.options.hardware_decoding { pick_hw_decoder(ff, &encoder) } else { None };
                    Some(VideoOut {
                        stream_index: src.index,
                        copy: false,
                        codec: vreq.codec,
                        src_width,
                        src_height,
                        width,
                        height,
                        bitrate_kbps: bitrate,
                        tonemap,
                        hw_decode,
                        deinterlace: src.interlaced == Some(true),
                        preset: req
                            .options
                            .preset
                            .clone()
                            .filter(|p| is_x264_preset(p))
                            .unwrap_or_else(|| "veryfast".into()),
                        encoder: Some(encoder),
                    })
                }
            }
            (Some(_), None) => {
                warnings.push("file has no video stream".into());
                None
            }
            _ => None,
        };
        if burn.is_some() && video.is_none() {
            return Err("subtitle burn-in requires a video stream".into());
        }

        // ---- audio ----
        let audio = match (&req.audio, media_audio(media, req)) {
            (Some(areq), Some(src)) => {
                let copy_ok = matches!(src.codec.as_str(), "aac" | "mp3" | "ac3" | "eac3");
                let copy = areq.mode == AudioMode::Copy && copy_ok;
                if areq.mode == AudioMode::Copy && !copy_ok {
                    warnings.push(format!("{} audio cannot be copied into HLS, transcoding to AAC", src.codec));
                }
                let src_channels = src.channels.unwrap_or(2).max(1) as u32;
                let channels = areq.channels.unwrap_or(2).clamp(1, 8).min(src_channels);
                let bitrate =
                    areq.bitrate_kbps.unwrap_or(if channels <= 2 { 192 } else { 64 * channels }).clamp(64, 640);
                let resample = !matches!(src.sample_rate, Some(44_100) | Some(48_000));
                Some(AudioOut { stream_index: src.index, copy, channels, bitrate_kbps: bitrate, resample })
            }
            (Some(_), None) if video.is_none() => return Err("file has no audio or video stream".into()),
            _ => None,
        };
        if video.is_none() && audio.is_none() {
            return Err("nothing to transcode: no video or audio requested".into());
        }

        // ---- segment layout ----
        let (boundaries, split_at) = match (&video, keyframes) {
            (Some(v), Some(kf)) if v.copy => keyframe_layout(kf, duration_ms, target_ms),
            _ => uniform_layout(duration_ms, target_ms),
        };

        Ok(Plan {
            duration_ms,
            boundaries,
            split_at,
            segment_seconds,
            video,
            audio,
            burn,
            throttle: req.options.throttle && ff.caps.readrate,
            warnings,
        })
    }

    pub fn video_plan(&self, media: &MediaInfo, encoder_override: Option<&EncoderInfo>) -> Option<VideoPlan> {
        let v = self.video.as_ref()?;
        let src = media.stream(v.stream_index);
        Some(VideoPlan {
            mode: if v.copy { VideoMode::Copy } else { VideoMode::Transcode },
            codec_in: src.map(|s| s.codec.clone()).unwrap_or_default(),
            codec_out: if v.copy {
                src.map(|s| s.codec.clone()).unwrap_or_default()
            } else {
                match v.codec {
                    VideoCodec::H264 => "h264".into(),
                    VideoCodec::Hevc => "hevc".into(),
                }
            },
            width_in: Some(v.src_width),
            height_in: Some(v.src_height),
            width_out: Some(v.width),
            height_out: Some(v.height),
            dynamic_range_in: src.map(|s| s.dynamic_range()).unwrap_or_default(),
            dv_profile: src.and_then(|s| s.dv_profile),
            tonemap: v.tonemap,
            encoder: if v.copy { None } else { encoder_override.or(v.encoder.as_ref()).map(|e| e.name.clone()) },
            decoder: Some(if v.copy { "copy".into() } else { v.hw_decode.clone().unwrap_or_else(|| "cpu".into()) }),
            bitrate_kbps: (!v.copy).then_some(v.bitrate_kbps),
            subtitle_burn: self.burn.as_ref().map(|b| b.label().to_owned()),
        })
    }

    pub fn audio_plan(&self, media: &MediaInfo) -> Option<AudioPlan> {
        let a = self.audio.as_ref()?;
        let src = media.stream(a.stream_index);
        Some(AudioPlan {
            mode: if a.copy { AudioMode::Copy } else { AudioMode::Transcode },
            codec_in: src.map(|s| s.codec.clone()).unwrap_or_default(),
            codec_out: if a.copy { src.map(|s| s.codec.clone()).unwrap_or_default() } else { "aac".into() },
            channels_in: src.and_then(|s| s.channels),
            channels_out: Some(if a.copy { src.and_then(|s| s.channels).unwrap_or(2) } else { a.channels as i64 }),
            bitrate_kbps: (!a.copy).then_some(a.bitrate_kbps),
        })
    }

    /// ffmpeg arguments producing segments `[start, end)`.
    pub fn ffmpeg_args(&self, ctx: &ArgsContext<'_>, start: u32, end: u32) -> Vec<OsString> {
        let start_ms = self.boundaries[start as usize];
        let end_ms = self.boundaries[end as usize];
        let copy = self.video.as_ref().is_some_and(|v| v.copy);
        // Remux seeks land on the keyframe at or before the seek point; nudge
        // past the boundary so rounding never selects the previous keyframe.
        let seek_ms = if copy && start_ms > 0 { start_ms + 1 } else { start_ms };
        let encoder = ctx.encoder.or_else(|| self.video.as_ref().and_then(|v| v.encoder.as_ref()));

        let mut a = Args::default();
        a.push_all(["-loglevel", "error", "-nostats", "-progress", "pipe:2", "-y"]);

        // Global hardware devices.
        if let Some(v) = self.video.as_ref().filter(|v| !v.copy) {
            match v.tonemap {
                Some(Tonemapper::Libplacebo) => a.push_all(["-init_hw_device", "vulkan=vk", "-filter_hw_device", "vk"]),
                Some(Tonemapper::Opencl) => a.push_all(["-init_hw_device", "opencl=ocl", "-filter_hw_device", "ocl"]),
                _ => {}
            }
            if encoder.is_some_and(|e| e.accel == HwAccel::Vaapi) {
                a.push_all(["-vaapi_device", VAAPI_DEVICE]);
            }
            if let Some(dec) = &v.hw_decode {
                a.push_all(["-hwaccel", dec.as_str()]);
            }
        }

        // Main input.
        if self.throttle {
            a.push_all(["-readrate", READRATE, "-readrate_initial_burst", READRATE_BURST]);
        }
        if seek_ms > 0 {
            a.push_all(["-ss", &secs(seek_ms)]);
        }
        a.push("-i");
        a.push_os(input_arg(ctx.input));

        // Optional bitmap subtitle file as a second input.
        if let Some(Burn::ImageFile { path, .. }) = &self.burn {
            if seek_ms > 0 {
                a.push_all(["-ss", &secs(seek_ms)]);
            }
            a.push("-i");
            a.push_os(input_arg(path));
        }

        if seek_ms > 0 {
            a.push_all(["-output_ts_offset", &secs(seek_ms)]);
        }

        // Video.
        match &self.video {
            Some(v) if v.copy => {
                a.push_all(["-map", &format!("0:{}", v.stream_index), "-c:v", "copy"]);
            }
            Some(v) => {
                let graph = self.filter_graph(v, encoder, start_ms);
                a.push_all(["-filter_complex", &graph, "-map", "[vout]"]);
                if let Some(enc) = encoder {
                    a.extend(encoder_args(enc, v));
                }
                let keyint = format!("expr:gte(t,n_forced*{})", trim_float(self.segment_seconds));
                a.push_all(["-force_key_frames", &keyint]);
            }
            None => a.push("-vn"),
        }

        // Audio.
        match &self.audio {
            Some(au) => {
                a.push_all(["-map", &format!("0:{}", au.stream_index)]);
                if au.copy {
                    a.push_all(["-c:a", "copy"]);
                } else {
                    a.push_all([
                        "-c:a",
                        "aac",
                        "-ac",
                        &au.channels.to_string(),
                        "-b:a",
                        &format!("{}k", au.bitrate_kbps),
                    ]);
                    if au.resample {
                        a.push_all(["-ar", "48000"]);
                    }
                }
            }
            None => a.push("-an"),
        }

        a.push_all(["-map_metadata", "-1", "-map_chapters", "-1", "-max_muxing_queue_size", "8192"]);
        // Never shift timestamps (B-frame delay would otherwise move only the
        // process that starts at 0), so every process shares one timeline.
        a.push_all(["-avoid_negative_ts", "disabled"]);
        if end < self.segment_count() {
            a.push_all(["-t", &secs(end_ms - seek_ms)]);
        }

        // Segmenting.
        let times: Vec<String> = ((start + 1)..end)
            .map(|k| {
                let at = self.split_at[k as usize];
                secs(if ctx.relative_split_times { at - seek_ms } else { at })
            })
            .collect();
        a.push_all([
            "-f",
            "segment",
            "-segment_format",
            "mpegts",
            "-segment_format_options",
            "avoid_negative_ts=disabled",
        ]);
        if !times.is_empty() {
            a.push_all(["-segment_times", &times.join(",")]);
        } else {
            // Single segment window: never split.
            a.push_all(["-segment_time", "999999"]);
        }
        a.push_all([
            "-segment_start_number",
            &start.to_string(),
            "-segment_list",
            "pipe:1",
            "-segment_list_type",
            "csv",
            "%d.ts",
        ]);
        a.0
    }

    fn filter_graph(&self, v: &VideoOut, encoder: Option<&EncoderInfo>, start_ms: i64) -> String {
        let scale_needed = v.width != v.src_width || v.height != v.src_height;
        let mut pre = Vec::<String>::new(); // at source resolution
        let mut post = Vec::<String>::new(); // after (optional) overlay

        if v.deinterlace {
            pre.push("yadif=mode=send_frame:deint=interlaced".into());
        }
        let overlay_needed = matches!(self.burn, Some(Burn::Image { .. } | Burn::ImageFile { .. }));
        match v.tonemap {
            Some(Tonemapper::Libplacebo) => {
                // libplacebo scales too, unless a bitmap overlay needs source size first.
                let (w, h) = if overlay_needed { (v.src_width, v.src_height) } else { (v.width, v.height) };
                pre.push(format!(
                    "libplacebo=w={w}:h={h}:format=yuv420p:colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:tonemapping=auto"
                ));
                if overlay_needed && scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
            Some(Tonemapper::Opencl) => {
                pre.push(OPENCL_TONEMAP.into());
                if scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
            Some(Tonemapper::Zscale) => {
                pre.push(ZSCALE_TONEMAP.into());
                if scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
            None => {
                if scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
        }
        if let Some(Burn::Text { file_name, .. }) = &self.burn {
            // libass works on absolute subtitle times; frames are on the local
            // (offset) timeline, so shift around the filter.
            let offset = secs(start_ms);
            post.push(format!(
                "setpts=PTS+{offset}/TB,subtitles=filename={file_name}:original_size={}x{},setpts=PTS-{offset}/TB",
                v.src_width, v.src_height
            ));
        }
        if encoder.is_some_and(|e| e.accel == HwAccel::Vaapi) {
            post.push("format=nv12,hwupload".into());
        } else {
            post.push("format=yuv420p".into());
        }

        let join = |parts: &[String]| if parts.is_empty() { "null".to_owned() } else { parts.join(",") };
        let input = format!("[0:{}]", v.stream_index);
        match &self.burn {
            Some(Burn::Image { stream_index, .. }) => format!(
                "{input}{}[base];[base][0:{stream_index}]overlay=x=(W-w)/2:y=H-h:eof_action=pass:repeatlast=0[ov];[ov]{}[vout]",
                join(&pre),
                join(&post)
            ),
            Some(Burn::ImageFile { .. }) => format!(
                "{input}{}[base];[base][1:0]overlay=x=(W-w)/2:y=H-h:eof_action=pass:repeatlast=0[ov];[ov]{}[vout]",
                join(&pre),
                join(&post)
            ),
            _ => {
                let mut all = pre;
                all.extend(post);
                format!("{input}{}[vout]", join(&all))
            }
        }
    }
}

pub struct ArgsContext<'a> {
    pub input: &'a Path,
    /// Overrides the planned encoder (software fallback when hardware sessions run out).
    pub encoder: Option<&'a EncoderInfo>,
    pub relative_split_times: bool,
}

#[derive(Default)]
struct Args(Vec<OsString>);

impl Args {
    fn push(&mut self, s: &str) {
        self.0.push(s.into());
    }
    fn push_os(&mut self, s: OsString) {
        self.0.push(s);
    }
    fn push_all<'a>(&mut self, items: impl IntoIterator<Item = &'a str>) {
        self.0.extend(items.into_iter().map(OsString::from));
    }
    fn extend(&mut self, items: Vec<String>) {
        self.0.extend(items.into_iter().map(OsString::from));
    }
}

fn encoder_args(enc: &EncoderInfo, v: &VideoOut) -> Vec<String> {
    let br = v.bitrate_kbps;
    let rate = |s: &mut Vec<String>| {
        s.extend([
            "-b:v".into(),
            format!("{br}k"),
            "-maxrate".into(),
            format!("{br}k"),
            "-bufsize".into(),
            format!("{}k", br * 2),
        ]);
    };
    let h264 = v.codec == VideoCodec::H264;
    let mut s: Vec<String> = vec!["-c:v".into(), enc.name.clone()];
    match enc.accel {
        HwAccel::Nvenc => {
            s.extend(
                ["-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", if h264 { "23" } else { "26" }].map(String::from),
            );
            rate(&mut s);
            s.extend(["-forced-idr", "1", "-no-scenecut", "1", "-spatial-aq", "1", "-g", "9999"].map(String::from));
            s.extend(["-profile:v".into(), if h264 { "high" } else { "main" }.into()]);
        }
        HwAccel::Qsv => {
            s.extend(["-preset", "veryfast", "-look_ahead", "0", "-forced_idr", "1", "-g", "9999"].map(String::from));
            rate(&mut s);
            s.extend(["-profile:v".into(), if h264 { "high" } else { "main" }.into()]);
        }
        HwAccel::Amf => {
            s.extend(
                ["-usage", "transcoding", "-quality", "balanced", "-rc", "vbr_peak", "-forced_idr", "1", "-g", "9999"]
                    .map(String::from),
            );
            rate(&mut s);
            s.extend(["-profile:v".into(), if h264 { "high" } else { "main" }.into()]);
        }
        HwAccel::Vaapi => {
            rate(&mut s);
            s.extend(["-g", "9999"].map(String::from));
        }
        HwAccel::Videotoolbox => {
            rate(&mut s);
            s.extend(["-allow_sw", "1", "-realtime", "1", "-g", "9999"].map(String::from));
        }
        HwAccel::None | HwAccel::Auto => {
            if h264 {
                s.extend(["-preset".into(), v.preset.clone(), "-crf".into(), "21".into()]);
                s.extend(["-maxrate".into(), format!("{br}k"), "-bufsize".into(), format!("{}k", br * 2)]);
                s.extend(
                    ["-profile:v", "high", "-sc_threshold", "0", "-forced-idr", "1", "-g", "9999"].map(String::from),
                );
            } else {
                s.extend(["-preset".into(), v.preset.clone(), "-crf".into(), "24".into()]);
                s.extend(["-maxrate".into(), format!("{br}k"), "-bufsize".into(), format!("{}k", br * 2)]);
                s.extend(["-x265-params", "log-level=error:scenecut=0:open-gop=0:keyint=9999"].map(String::from));
            }
        }
    }
    s
}

/// Decoder hwaccel paired with an encoder, only when FFmpeg reports it.
fn pick_hw_decoder(ff: &Ffmpeg, encoder: &EncoderInfo) -> Option<String> {
    let has = |name: &str| ff.caps.hwaccels.iter().any(|h| h == name);
    let nvidia = ff.caps.encoders.iter().any(|e| e.accel == HwAccel::Nvenc);
    let choice = match encoder.accel {
        HwAccel::Nvenc => "cuda",
        HwAccel::Vaapi => "vaapi",
        HwAccel::Videotoolbox => "videotoolbox",
        HwAccel::Qsv | HwAccel::Amf if cfg!(windows) => "d3d11va",
        HwAccel::Qsv => "vaapi",
        _ if nvidia => "cuda",
        _ if cfg!(target_os = "macos") => "videotoolbox",
        _ => return None,
    };
    has(choice).then(|| choice.to_owned())
}

fn resolve_burn(
    sub: &pley_transcoder_api::SubtitleBurnRequest,
    media: &MediaInfo,
    ff: &Ffmpeg,
) -> Result<Burn, String> {
    if let Some(path) = sub.external_path.as_deref().filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return match ext.as_str() {
            "sup" => {
                if !ff.caps.subtitle_burn_image {
                    return Err("ffmpeg lacks the overlay filter".into());
                }
                Ok(Burn::ImageFile { path, label: name })
            }
            "srt" | "ass" | "ssa" | "vtt" | "sub" | "smi" => {
                if !ff.caps.subtitle_burn_text {
                    return Err("ffmpeg was built without libass (subtitles filter)".into());
                }
                Ok(Burn::Text { source: TextSource::File(path), file_name: "burn.ass".into(), label: name })
            }
            other => Err(format!("unsupported subtitle file type '.{other}'")),
        };
    }
    let index = sub.stream_index.ok_or("subtitle burn-in needs a stream index or a file")?;
    let stream = media
        .stream(index)
        .filter(|s| s.kind == StreamKind::Subtitle)
        .ok_or_else(|| format!("stream {index} is not a subtitle stream"))?;
    let label = format!("{} #{index}", stream.codec);
    if stream.text_based.unwrap_or_else(|| is_text_subtitle_codec(&stream.codec)) {
        if !ff.caps.subtitle_burn_text {
            return Err("ffmpeg was built without libass (subtitles filter)".into());
        }
        Ok(Burn::Text { source: TextSource::Embedded(index), file_name: "burn.ass".into(), label })
    } else {
        if !ff.caps.subtitle_burn_image {
            return Err("ffmpeg lacks the overlay filter".into());
        }
        Ok(Burn::Image { stream_index: index, label })
    }
}

fn media_video<'a>(media: &'a MediaInfo, req: &JobRequest) -> Option<&'a MediaStream> {
    let wanted = req.video.as_ref().and_then(|v| v.stream_index);
    match wanted {
        Some(i) => media.stream(i).filter(|s| s.kind == StreamKind::Video),
        None => media.video(),
    }
}

fn media_audio<'a>(media: &'a MediaInfo, req: &JobRequest) -> Option<&'a MediaStream> {
    let wanted = req.audio.as_ref().and_then(|a| a.stream_index);
    match wanted {
        Some(i) => media.stream(i).filter(|s| s.kind == StreamKind::Audio).or_else(|| media.default_audio()),
        None => media.default_audio(),
    }
}

/// Fit `w×h` inside the limits keeping aspect ratio, never upscaling, even sizes.
pub fn fit(w: i64, h: i64, max_w: Option<u32>, max_h: Option<u32>) -> (i64, i64) {
    let (w, h) = (w.max(2), h.max(2));
    let mut scale = 1.0f64;
    if let Some(mw) = max_w.filter(|m| *m > 0) {
        scale = scale.min(mw as f64 / w as f64);
    }
    if let Some(mh) = max_h.filter(|m| *m > 0) {
        scale = scale.min(mh as f64 / h as f64);
    }
    let scale = scale.min(1.0);
    let even = |v: f64| ((v / 2.0).round() as i64 * 2).max(2);
    (even(w as f64 * scale), even(h as f64 * scale))
}

pub fn default_bitrate_kbps(height: i64) -> u32 {
    match height {
        h if h >= 2000 => 20_000,
        h if h >= 1400 => 12_000,
        h if h >= 1000 => 8_000,
        h if h >= 700 => 4_000,
        h if h >= 500 => 2_000,
        _ => 1_200,
    }
}

fn is_x264_preset(p: &str) -> bool {
    matches!(p, "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium" | "slow" | "slower" | "veryslow")
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Fixed-length segments. A trailing sliver shorter than a quarter segment is
/// merged into the previous one.
pub fn uniform_layout(duration_ms: i64, target_ms: i64) -> (Vec<i64>, Vec<i64>) {
    let mut boundaries: Vec<i64> = (0..).map(|k| k * target_ms).take_while(|&t| t < duration_ms).collect();
    if boundaries.len() > 1 && duration_ms - boundaries[boundaries.len() - 1] < target_ms / 4 {
        boundaries.pop();
    }
    boundaries.push(duration_ms);
    let jitter = (target_ms / 4).min(500);
    let split_at = boundaries.iter().map(|&b| (b - jitter).max(0)).collect();
    (boundaries, split_at)
}

/// Segments that start on source keyframes, each at least `target_ms` long.
/// Split points sit halfway between a boundary keyframe and its predecessor.
pub fn keyframe_layout(keyframes: &[i64], duration_ms: i64, target_ms: i64) -> (Vec<i64>, Vec<i64>) {
    let mut boundaries = vec![0i64];
    let mut split_at = vec![0i64];
    let mut previous_kf = 0i64;
    for &kf in keyframes {
        if kf <= 0 || kf >= duration_ms {
            previous_kf = previous_kf.max(kf.min(duration_ms));
            continue;
        }
        let last = *boundaries.last().unwrap_or(&0);
        if kf - last >= target_ms && duration_ms - kf >= target_ms / 4 {
            boundaries.push(kf);
            split_at.push(kf - ((kf - previous_kf) / 2).min(500));
        }
        previous_kf = kf;
    }
    boundaries.push(duration_ms);
    split_at.push(duration_ms);
    (boundaries, split_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pley_transcoder_api::{AudioRequest, Capabilities, SubtitleBurnRequest, TranscodeOptions, VideoRequest};

    fn ffmpeg_with(encoders: Vec<EncoderInfo>, tonemappers: Vec<Tonemapper>) -> Ffmpeg {
        Ffmpeg {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            caps: Capabilities {
                ffmpeg_available: true,
                encoders,
                tonemappers,
                hwaccels: vec!["cuda".into(), "d3d11va".into()],
                subtitle_burn_image: true,
                subtitle_burn_text: true,
                readrate: true,
                ..Default::default()
            },
            segment_times_relative: true,
        }
    }

    fn x264() -> EncoderInfo {
        EncoderInfo { name: "libx264".into(), codec: VideoCodec::H264, hardware: false, accel: HwAccel::None }
    }

    fn nvenc() -> EncoderInfo {
        EncoderInfo { name: "h264_nvenc".into(), codec: VideoCodec::H264, hardware: true, accel: HwAccel::Nvenc }
    }

    fn media(range: DynamicRange, dv_profile: Option<i64>) -> MediaInfo {
        MediaInfo {
            container: Some("mkv".into()),
            duration_ms: Some(60_000),
            streams: vec![
                MediaStream {
                    index: 0,
                    kind: StreamKind::Video,
                    codec: "hevc".into(),
                    width: Some(3840),
                    height: Some(2160),
                    dynamic_range: Some(range),
                    dv_profile,
                    dv_bl_compat: dv_profile.map(|p| if p == 5 { 0 } else { 1 }),
                    ..Default::default()
                },
                MediaStream {
                    index: 1,
                    kind: StreamKind::Audio,
                    codec: "eac3".into(),
                    channels: Some(6),
                    sample_rate: Some(48_000),
                    default: true,
                    ..Default::default()
                },
                MediaStream {
                    index: 2,
                    kind: StreamKind::Subtitle,
                    codec: "hdmv_pgs_subtitle".into(),
                    text_based: Some(false),
                    ..Default::default()
                },
                MediaStream {
                    index: 3,
                    kind: StreamKind::Subtitle,
                    codec: "subrip".into(),
                    text_based: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    fn request() -> JobRequest {
        JobRequest {
            input: "/media/movie.mkv".into(),
            video: Some(VideoRequest { max_height: Some(1080), ..Default::default() }),
            audio: Some(AudioRequest::default()),
            options: TranscodeOptions::default(),
            ..Default::default()
        }
    }

    fn args_string(plan: &Plan, start: u32, end: u32) -> String {
        let ctx = ArgsContext { input: Path::new("/media/movie.mkv"), encoder: None, relative_split_times: true };
        plan.ffmpeg_args(&ctx, start, end)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn uniform_layout_merges_tail() {
        let (b, s) = uniform_layout(10_500, 4_000);
        assert_eq!(b, vec![0, 4_000, 8_000, 10_500]);
        assert_eq!(s, vec![0, 3_500, 7_500, 10_000]);
        let (b, _) = uniform_layout(8_400, 4_000);
        assert_eq!(b, vec![0, 4_000, 8_400]);
    }

    #[test]
    fn keyframe_layout_groups_short_gops() {
        let kf = vec![0, 2_002, 4_004, 6_006, 8_008, 10_010, 12_012];
        let (b, s) = keyframe_layout(&kf, 13_000, 4_000);
        assert_eq!(b, vec![0, 4_004, 8_008, 13_000]);
        assert_eq!(s[1], 4_004 - 500);
        let kf = vec![0, 9_000, 18_000];
        let (b, _) = keyframe_layout(&kf, 20_000, 4_000);
        assert_eq!(b, vec![0, 9_000, 18_000, 20_000]);
    }

    #[test]
    fn segment_lookup() {
        let (boundaries, split_at) = uniform_layout(10_000, 4_000);
        let plan = Plan {
            duration_ms: 10_000,
            boundaries,
            split_at,
            segment_seconds: 4.0,
            video: None,
            audio: None,
            burn: None,
            throttle: false,
            warnings: vec![],
        };
        assert_eq!(plan.segment_for(0), 0);
        assert_eq!(plan.segment_for(3_999), 0);
        assert_eq!(plan.segment_for(4_000), 1);
        assert_eq!(plan.segment_for(99_999), 2);
    }

    #[test]
    fn fits_within_limits() {
        assert_eq!(fit(3840, 2160, None, Some(1080)), (1920, 1080));
        assert_eq!(fit(1920, 800, Some(1280), Some(720)), (1280, 534));
        assert_eq!(fit(640, 480, None, Some(1080)), (640, 480));
        assert_eq!(fit(1440, 1080, Some(1920), Some(1080)), (1440, 1080));
    }

    #[test]
    fn hdr10_transcode_uses_nvenc_and_tonemapping() {
        let ff = ffmpeg_with(vec![nvenc(), x264()], vec![Tonemapper::Libplacebo, Tonemapper::Zscale]);
        let plan = Plan::build(&request(), &media(DynamicRange::Hdr10, None), &ff, None).unwrap();
        let v = plan.video.as_ref().unwrap();
        assert_eq!(v.encoder.as_ref().unwrap().name, "h264_nvenc");
        assert_eq!(v.tonemap, Some(Tonemapper::Libplacebo));
        assert_eq!(v.hw_decode.as_deref(), Some("cuda"));
        assert_eq!((v.width, v.height), (1920, 1080));
        let a = plan.audio.as_ref().unwrap();
        assert!(!a.copy);
        assert_eq!(a.channels, 2);

        let args = args_string(&plan, 3, 8);
        assert!(args.contains("-init_hw_device vulkan=vk -filter_hw_device vk"));
        assert!(args.contains("-hwaccel cuda"));
        assert!(args.contains("-ss 12.000 -i file:/media/movie.mkv"));
        assert!(args.contains("-output_ts_offset 12.000"));
        assert!(args.contains("libplacebo=w=1920:h=1080"));
        assert!(args.contains("-c:v h264_nvenc"));
        assert!(args.contains("-force_key_frames expr:gte(t,n_forced*4)"));
        // Splits for boundaries 16,20,24,28 s relative to the 12 s start, 0.5 s early.
        assert!(args.contains("-segment_times 3.500,7.500,11.500,15.500"), "{args}");
        assert!(args.contains("-segment_start_number 3"));
        assert!(args.contains("-t 20.000"));
        assert!(args.contains("-readrate 2 -readrate_initial_burst 60"));
    }

    #[test]
    fn dolby_vision_profile5_requires_libplacebo() {
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(5)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Zscale));
        assert!(plan.warnings.iter().any(|w| w.contains("Dolby Vision")));

        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Opencl, Tonemapper::Libplacebo]);
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(5)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Libplacebo));
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(8)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Opencl));
    }

    #[test]
    fn copy_needs_keyframes() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.video = Some(VideoRequest { mode: VideoMode::Copy, ..Default::default() });
        req.audio = Some(AudioRequest { mode: AudioMode::Copy, ..Default::default() });
        let m = media(DynamicRange::Sdr, None);

        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        assert!(!plan.video.as_ref().unwrap().copy);
        assert!(plan.warnings.iter().any(|w| w.contains("keyframe")));

        let kf: Vec<i64> = (0..30).map(|i| i * 2_000).collect();
        let plan = Plan::build(&req, &m, &ff, Some(&kf)).unwrap();
        assert!(plan.video.as_ref().unwrap().copy);
        assert!(plan.audio.as_ref().unwrap().copy, "eac3 can be copied into MPEG-TS");
        let args = args_string(&plan, 2, 4);
        assert!(args.contains("-ss 8.001"), "{args}");
        assert!(args.contains("-c:v copy"));
        assert!(!args.contains("-force_key_frames"));
    }

    #[test]
    fn image_subtitles_are_overlaid() {
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let mut req = request();
        req.subtitle = Some(SubtitleBurnRequest { stream_index: Some(2), external_path: None });
        let plan = Plan::build(&req, &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("[base][0:2]overlay"), "{args}");
        assert!(args.contains("scale=1920:1080"));
        assert!(!args.contains("-ss"));
    }

    #[test]
    fn text_subtitles_use_libass_with_offset() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.subtitle = Some(SubtitleBurnRequest { stream_index: Some(3), external_path: None });
        let plan = Plan::build(&req, &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        assert!(matches!(plan.burn, Some(Burn::Text { .. })));
        let args = args_string(&plan, 5, 6);
        assert!(args.contains("setpts=PTS+20.000/TB,subtitles=filename=burn.ass"), "{args}");
        assert!(args.contains("-segment_time 999999"));
    }

    #[test]
    fn audio_only_job() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let req =
            JobRequest { input: "/music/a.flac".into(), audio: Some(AudioRequest::default()), ..Default::default() };
        let m = MediaInfo {
            duration_ms: Some(200_000),
            streams: vec![MediaStream {
                index: 0,
                kind: StreamKind::Audio,
                codec: "flac".into(),
                channels: Some(2),
                sample_rate: Some(96_000),
                ..Default::default()
            }],
            ..Default::default()
        };
        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        let args = args_string(&plan, 0, 3);
        assert!(args.contains("-vn"));
        assert!(args.contains("-c:a aac -ac 2 -b:a 192k -ar 48000"));
    }
}
