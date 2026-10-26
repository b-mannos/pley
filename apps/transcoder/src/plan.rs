// SPDX-License-Identifier: LicenseRef-Restricted
//! Turns a [`JobRequest`] plus [`MediaInfo`] into a concrete plan (segment
//! layout, encoder, filters, tone mapping) and renders ffmpeg command lines for
//! any window of segments.
//!
//! Segment alignment: every process is started with `-ss <boundary>` and
//! `-output_ts_offset <boundary>` (a remux a little past its boundary
//! keyframe, see [`remux_seek_points`]), so segment timestamps stay on the absolute
//! timeline no matter where ffmpeg was (re)started. Split points are passed via
//! the segment muxer's `-segment_times`; when transcoding, keyframes are forced
//! on the same grid, when remuxing the grid *is* the source keyframes.

use crate::adaptive::AUTO;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pley_transcoder_api::{
    AudioCodec, AudioMode, AudioPlan, DolbyVisionMode, DynamicRange, EncoderInfo, FileContainer, HwAccel, JobRequest,
    MediaInfo, MediaStream, StreamKind, Tonemapper, VideoCodec, VideoMode, VideoPlan, VideoRequest, decode_key,
    is_text_subtitle_codec,
};

use crate::ffmpeg::{Ffmpeg, VAAPI_DEVICE};
use crate::tsedit::Edits;
use crate::util::{MEDIA_INPUT_OPTIONS, input_arg, secs};

pub const DEFAULT_SEGMENT_SECONDS: f64 = 4.0;
/// Most segments a job may have. Its layout, segment states and playlist grow
/// with the count, so a file claiming an absurd length (damaged or crafted)
/// must not turn into gigabytes of memory.
pub const MAX_SEGMENTS: i64 = 200_000;
/// Longest media the transcoder believes: 200 000 segments of 4 s, about 9
/// days. Anything longer comes from a damaged or crafted file.
pub const MAX_DURATION_MS: i64 = MAX_SEGMENTS * 4_000;
/// Maximum segments per ffmpeg process (keeps command lines short on Windows).
pub const PROCESS_WINDOW: u32 = 1200;
/// Throttling: read at 2× realtime after a 60 s burst.
pub const READRATE: f64 = 2.0;
pub const READRATE_BURST_MS: i64 = 60_000;

/// CPU tone mapping chain (zimg). Expects HDR input, outputs 8-bit BT.709.
pub const ZSCALE_TONEMAP: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";
const OPENCL_TONEMAP: &str = "format=p010,hwupload,tonemap_opencl=tonemap=hable:desat=0:t=bt709:m=bt709:p=bt709:format=nv12,hwdownload,format=nv12";

/// A `setparams` filter giving HDR frames the colour properties they lack
/// (PQ, BT.2020), for the zimg and OpenCL tone mappers, which refuse to
/// convert from unknown ones ("no path between colorspaces"). Dolby Vision
/// profile 5 frames come out of the decoder untagged (their IPTPQc2 colours
/// are only defined by the RPU, which libplacebo applies), and so do HDR
/// streams that tag only some properties. For profile 5 the colours are then
/// off, but there is a picture.
pub fn untagged_hdr_fix(stream: &MediaStream) -> Option<String> {
    let unknown =
        |v: &Option<String>| v.as_deref().is_none_or(|v| matches!(v, "" | "unknown" | "unspecified" | "reserved"));
    let params: Vec<&str> = [
        (&stream.color_transfer, "color_trc=smpte2084"),
        (&stream.color_primaries, "color_primaries=bt2020"),
        (&stream.color_space, "colorspace=bt2020nc"),
    ]
    .into_iter()
    .filter(|(value, _)| unknown(value))
    .map(|(_, param)| param)
    .collect();
    (!params.is_empty()).then(|| format!("setparams={}", params.join(":")))
}

/// Where frames live between decode and encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pipeline {
    /// CPU filters. Decoding may use `-hwaccel` with frames copied to system memory.
    Software,
    /// Vulkan decode → libplacebo tone mapping → `h264_vulkan` (`encode_on_gpu`)
    /// or a single download into the selected encoder.
    Vulkan { encode_on_gpu: bool },
    /// Decode, scale and encode on one GPU API without any copies.
    Gpu(HwAccel),
}

impl Pipeline {
    /// The pipeline to use when the planned (hardware) encoder is replaced by a
    /// software one because hardware sessions ran out.
    fn for_software_encoder(self) -> Pipeline {
        match self {
            Pipeline::Vulkan { .. } => Pipeline::Vulkan { encode_on_gpu: false },
            _ => Pipeline::Software,
        }
    }
}

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
    /// Tags the tone mapper's input when the source leaves colours unknown
    /// ([`untagged_hdr_fix`]).
    pub tonemap_input: Option<String>,
    pub hw_decode: Option<String>,
    pub deinterlace: bool,
    pub preset: String,
    pub pipeline: Pipeline,
    /// What a remux of a Dolby Vision HEVC stream does with it (`None`:
    /// transcoded, or no Dolby Vision).
    pub dolby_vision: Option<DolbyVisionMode>,
    /// A remux of HEVC leaves out its HDR10+ metadata.
    pub strip_hdr10plus: bool,
    /// A transcode that keeps HDR: HEVC Main 10 in HDR10 or HLG (see
    /// [`hdr_output`]); `None` when it comes out SDR.
    pub hdr: Option<HdrOut>,
}

/// What a transcode that keeps HDR comes out as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdrOut {
    /// [`DynamicRange::Hdr10`] or [`DynamicRange::Hlg`].
    pub range: DynamicRange,
    /// libplacebo converts the video to it (HLG or Dolby Vision profile 5
    /// to HDR10, say); otherwise the decoded frames are in it already,
    /// their HDR10 metadata going to the encoder with them.
    pub convert: bool,
}

impl HdrOut {
    /// FFmpeg's name for the output's transfer function.
    fn transfer(self) -> &'static str {
        if self.range == DynamicRange::Hlg { "arib-std-b67" } else { "smpte2084" }
    }
}

/// The 10-bit pixel format an encoder takes: P010 for hardware encoders,
/// planar for x265.
pub fn ten_bit_format(accel: HwAccel) -> &'static str {
    match accel {
        HwAccel::None | HwAccel::Auto => "yuv420p10le",
        _ => "p010le",
    }
}

/// Whether a stream's base picture is HLG rather than PQ: HLG itself, and
/// Dolby Vision profile 8.4.
fn hlg_source(src: &MediaStream) -> bool {
    match src.dynamic_range() {
        DynamicRange::Hlg => true,
        DynamicRange::DolbyVision => src.dv_bl_compat == Some(4),
        _ => false,
    }
}

/// What a transcode of `src` keeps of its HDR, as `vreq` asks (see
/// [`pley_transcoder_api::VideoRequest::dynamic_range`]), with the encoder
/// for it; `None` with a warning when it can't be kept, and quietly for SDR
/// video and SDR requests.
///
/// The HDR10 metadata (mastering display, content light levels) goes along
/// with the decoded frames, through scaling and libplacebo, to the encoders
/// (NVENC and x265 write it). Dolby Vision metadata doesn't: NVENC never
/// writes it and x265 is told not to, so Dolby Vision comes out as its
/// HDR10 base layer, or converted by libplacebo when it has none (profile 5).
fn hdr_output<'a>(
    vreq: &VideoRequest,
    src: &MediaStream,
    burn: bool,
    ff: &'a Ffmpeg,
    accel: HwAccel,
    warnings: &mut Vec<String>,
) -> Option<(HdrOut, &'a EncoderInfo)> {
    let range = src.dynamic_range();
    let wanted = vreq.dynamic_range.dynamic_range();
    // Dolby Vision on an SDR base layer (profile 8.2) is SDR underneath.
    let sdr_base = range == DynamicRange::DolbyVision && src.dv_bl_compat == Some(2);
    if !range.is_hdr() || !wanted.is_hdr() || sdr_base {
        return None;
    }
    let mut no = |why: &str| -> Option<(HdrOut, &'a EncoderInfo)> {
        warnings.push(format!("{why}: {} is tone-mapped to SDR", src.dynamic_range_label()));
        None
    };
    if vreq.codec != VideoCodec::Hevc {
        return no("HDR is kept in HEVC only");
    }
    if burn {
        return no("subtitles drawn onto the video are SDR");
    }
    let convert = src.is_dolby_vision_only() || hlg_source(src) != (wanted == DynamicRange::Hlg);
    if convert && !ff.caps.tonemappers.contains(&Tonemapper::Libplacebo) {
        return no("converting to another HDR format needs libplacebo");
    }
    let Some(encoder) = ff.pick_main10_encoder(accel).0 else {
        return no("no 10-bit HEVC encoder works");
    };
    Some((HdrOut { range: wanted, convert }, encoder))
}

/// The encoder used by fully-GPU Vulkan pipelines.
pub fn vulkan_encoder() -> EncoderInfo {
    EncoderInfo {
        name: "h264_vulkan".into(),
        codec: VideoCodec::H264,
        hardware: true,
        accel: HwAccel::Vulkan,
        main10: false,
    }
}

#[derive(Debug, Clone)]
pub struct AudioOut {
    pub stream_index: i64,
    pub copy: bool,
    /// The codec when converting.
    pub codec: AudioCodec,
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
    /// Where a process starting at boundary k seeks (`-ss`): the boundary
    /// itself when transcoding, a little past the boundary keyframe when
    /// remuxing (see [`remux_seek_points`]).
    pub seek_at: Vec<i64>,
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
        if duration_ms > MAX_DURATION_MS {
            return Err(format!("a media duration of {} days is not plausible", duration_ms / 86_400_000));
        }
        let requested =
            req.segment_seconds.filter(|s| s.is_finite()).unwrap_or(DEFAULT_SEGMENT_SECONDS).clamp(1.0, 10.0);
        // Very long files get longer segments rather than more of them.
        let target_ms = ((requested * 1000.0).round() as i64).max((duration_ms + MAX_SEGMENTS - 1) / MAX_SEGMENTS);
        let segment_seconds = target_ms as f64 / 1000.0;
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
                    let dv_hevc = src.dynamic_range() == DynamicRange::DolbyVision && src.codec == "hevc";
                    let dolby_vision = dv_hevc.then_some(match vreq.dolby_vision {
                        DolbyVisionMode::To81 if src.dv_profile != Some(7) => DolbyVisionMode::Signal,
                        mode => mode,
                    });
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
                        tonemap_input: None,
                        hw_decode: None,
                        deinterlace: false,
                        preset: String::new(),
                        pipeline: Pipeline::Software,
                        dolby_vision,
                        strip_hdr10plus: vreq.strip_hdr10plus && src.codec == "hevc",
                        hdr: None,
                    })
                } else {
                    let (width, height) = fit(src_width, src_height, vreq.max_width, vreq.max_height);
                    let mut bitrate = default_bitrate_kbps(height);
                    if let Some(max) = vreq.max_bitrate_kbps {
                        bitrate = bitrate.min(max.max(300));
                    }
                    let accel = req.options.hwaccel;
                    let kept = hdr_output(vreq, src, burn.is_some(), ff, accel, &mut warnings);
                    let hdr = kept.map(|(hdr, _)| hdr);
                    let encoder = match kept {
                        Some((_, encoder)) => encoder.clone(),
                        None => {
                            let (encoder, warning) = ff.pick_encoder(vreq.codec, accel);
                            warnings.extend(warning);
                            encoder.cloned().ok_or("no working video encoder found (is ffmpeg installed?)")?
                        }
                    };
                    let range = src.dynamic_range();
                    let tonemap = match hdr {
                        // libplacebo converts between HDR formats; nothing
                        // is needed for frames in the output's already.
                        Some(hdr) => hdr.convert.then_some(Tonemapper::Libplacebo),
                        None if range.is_hdr() => {
                            let (choice, w) = ff.pick_tonemapper(req.options.tonemap, src.is_dolby_vision_only());
                            warnings.extend(w);
                            choice
                        }
                        None => None,
                    };
                    let tonemap_input = match tonemap {
                        Some(Tonemapper::Zscale | Tonemapper::Opencl) => untagged_hdr_fix(src),
                        _ => None,
                    };
                    let deinterlace = src.interlaced == Some(true);
                    let key = decode_key(&src.codec, src.bit_depth);
                    let pipeline = if !req.options.hardware_decoding || deinterlace {
                        Pipeline::Software
                    } else if tonemap == Some(Tonemapper::Libplacebo) && ff.caps.vulkan_decode.contains(&key) {
                        // Encoding on Vulkan too only when asked for: browsers
                        // don't play what h264_vulkan makes of 4K (and of
                        // anything, with B-frames). The frames go to the
                        // usual encoder after one download instead.
                        let encode_on_gpu = ff.caps.vulkan_encode
                            && burn.is_none()
                            && vreq.codec == VideoCodec::H264
                            && req.options.hwaccel == HwAccel::Vulkan;
                        Pipeline::Vulkan { encode_on_gpu }
                    } else if tonemap.is_none() && burn.is_none() {
                        // HDR in 10 bits stays on the GPU with CUDA only
                        // (the one tried); the others scale on the CPU.
                        ff.caps
                            .gpu_pipelines
                            .iter()
                            .filter(|p| hdr.is_none() || p.accel == HwAccel::Nvenc)
                            .find(|p| p.accel == encoder.accel && p.codecs.contains(&key))
                            .map_or(Pipeline::Software, |p| Pipeline::Gpu(p.accel))
                    } else {
                        Pipeline::Software
                    };
                    let hw_decode = match pipeline {
                        Pipeline::Vulkan { .. } => Some("vulkan".to_owned()),
                        Pipeline::Gpu(accel) => Some(gpu_decoder_name(accel).to_owned()),
                        Pipeline::Software if req.options.hardware_decoding => pick_hw_decoder(ff, &encoder),
                        Pipeline::Software => None,
                    };
                    let encoder = match pipeline {
                        Pipeline::Vulkan { encode_on_gpu: true } => vulkan_encoder(),
                        _ => encoder,
                    };
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
                        tonemap_input,
                        hw_decode,
                        deinterlace,
                        pipeline,
                        preset: req
                            .options
                            .preset
                            .clone()
                            .filter(|p| is_x264_preset(p) || p == AUTO)
                            .unwrap_or_else(|| "veryfast".into()),
                        encoder: Some(encoder),
                        dolby_vision: None,
                        strip_hdr10plus: false,
                        hdr,
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
                let codec = areq.codec;
                if areq.mode == AudioMode::Copy && !copy_ok {
                    warnings.push(format!(
                        "{} audio cannot be copied into HLS, transcoding to {}",
                        src.codec,
                        codec_name(codec)
                    ));
                }
                let src_channels = src.channels.unwrap_or(2).max(1) as u32;
                // AC-3 and E-AC-3 (as Dolby makes it, and FFmpeg's encoders)
                // stop at 5.1; AAC goes to 7.1.
                let most = if codec == AudioCodec::Aac { 8 } else { 6 };
                let channels = areq.channels.unwrap_or(2).clamp(1, most).min(src_channels);
                let default_bitrate = match codec {
                    _ if channels <= 2 => 192,
                    AudioCodec::Aac => 64 * channels,
                    AudioCodec::Ac3 | AudioCodec::Eac3 => 640,
                };
                let bitrate = areq.bitrate_kbps.unwrap_or(default_bitrate).clamp(64, 640);
                // FFmpeg's AC-3 encoders take 32, 44.1 and 48 kHz only.
                let resample = !matches!(src.sample_rate, Some(44_100) | Some(48_000));
                Some(AudioOut { stream_index: src.index, copy, codec, channels, bitrate_kbps: bitrate, resample })
            }
            (Some(_), None) if video.is_none() => return Err("file has no audio or video stream".into()),
            _ => None,
        };
        if video.is_none() && audio.is_none() {
            return Err("nothing to transcode: no video or audio requested".into());
        }

        // ---- segment layout ----
        let (boundaries, split_at, seek_at) = match (&video, keyframes) {
            (Some(v), Some(kf)) if v.copy => {
                // Keyframe times are packet timestamps; `-ss` and the output
                // timeline count from the container's start time (often far
                // from zero in MPEG-TS).
                let offset = media.start_ms.unwrap_or(0);
                let relative: Vec<i64> = kf.iter().map(|t| t.saturating_sub(offset)).collect();
                let (boundaries, split_at) = keyframe_layout(&relative, duration_ms, target_ms);
                let seek_at = remux_seek_points(&boundaries, &relative, seeks_back(media));
                (boundaries, split_at, seek_at)
            }
            _ => {
                let (boundaries, split_at) = uniform_layout(duration_ms, target_ms);
                (boundaries.clone(), split_at, boundaries)
            }
        };

        Ok(Plan {
            duration_ms,
            boundaries,
            split_at,
            seek_at,
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
            dynamic_range_out: (!v.copy).then(|| v.hdr.map_or(DynamicRange::Sdr, |h| h.range)),
            dv_profile: src.and_then(|s| s.dv_profile),
            dv_bl_compat: src.and_then(|s| s.dv_bl_compat),
            tonemap: v.tonemap,
            encoder: if v.copy { None } else { encoder_override.or(v.encoder.as_ref()).map(|e| e.name.clone()) },
            decoder: Some(if v.copy { "copy".into() } else { v.hw_decode.clone().unwrap_or_else(|| "cpu".into()) }),
            pipeline: Some(
                if v.copy {
                    "copy"
                } else {
                    let pipeline =
                        if encoder_override.is_some() { v.pipeline.for_software_encoder() } else { v.pipeline };
                    match pipeline {
                        Pipeline::Gpu(_) | Pipeline::Vulkan { encode_on_gpu: true } => "gpu",
                        Pipeline::Vulkan { encode_on_gpu: false } => "hybrid",
                        Pipeline::Software if v.hw_decode.is_some() => "hybrid",
                        Pipeline::Software => "cpu",
                    }
                }
                .to_owned(),
            ),
            bitrate_kbps: (!v.copy).then_some(v.bitrate_kbps),
            subtitle_burn: self.burn.as_ref().map(|b| b.label().to_owned()),
            dolby_vision: v.dolby_vision,
            hdr10plus_stripped: v.strip_hdr10plus,
        })
    }

    /// What finished segments need done to their video by the transcoder
    /// itself (see [`crate::tsedit`]), when anything.
    pub fn segment_edits(&self) -> Option<Edits> {
        let v = self.video.as_ref().filter(|v| v.copy)?;
        let edits = Edits { to_81: v.dolby_vision == Some(DolbyVisionMode::To81), strip_hdr10plus: v.strip_hdr10plus };
        edits.any().then_some(edits)
    }

    pub fn audio_plan(&self, media: &MediaInfo) -> Option<AudioPlan> {
        let a = self.audio.as_ref()?;
        let src = media.stream(a.stream_index);
        Some(AudioPlan {
            mode: if a.copy { AudioMode::Copy } else { AudioMode::Transcode },
            codec_in: src.map(|s| s.codec.clone()).unwrap_or_default(),
            codec_out: if a.copy { src.map(|s| s.codec.clone()).unwrap_or_default() } else { a.codec.as_str().into() },
            channels_in: src.and_then(|s| s.channels),
            channels_out: Some(if a.copy { src.and_then(|s| s.channels).unwrap_or(2) } else { a.channels as i64 }),
            bitrate_kbps: (!a.copy).then_some(a.bitrate_kbps),
        })
    }

    /// ffmpeg arguments producing segments `[start, end)`.
    pub fn ffmpeg_args(&self, ctx: &ArgsContext<'_>, start: u32, end: u32) -> Vec<OsString> {
        let start_ms = self.boundaries[start as usize];
        let mut a = self.stream_args(ctx, start, end);

        // Segmenting. Relative split times count from the first video packet,
        // which is the boundary (a remux's seek point lies past it).
        let times: Vec<String> = ((start + 1)..end)
            .map(|k| {
                let at = self.split_at[k as usize];
                secs(if ctx.relative_split_times { at - start_ms } else { at })
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

    /// The whole media as one file at `output` (a converted download): the
    /// streams the segments would have, made in one go, and the subtitle
    /// tracks of `media` in their order, for the player to show itself. In
    /// Matroska every one as it is (MP4's text made SRT); in MP4 the text
    /// ones made MP4's own, the pictures left out.
    /// Players find a track by its place among the subtitle tracks, the
    /// original's as the file's.
    pub fn file_args(
        &self,
        ctx: &ArgsContext<'_>,
        media: &MediaInfo,
        output: &Path,
        container: FileContainer,
    ) -> Vec<OsString> {
        let mut a = self.stream_args(ctx, 0, self.segment_count());
        // Their languages and names (stream_args leaves the input's metadata out).
        let name = |a: &mut Args, out: &str, s: &MediaStream| {
            let key = format!("-metadata:s:{out}");
            if let Some(language) = s.language.as_deref().filter(|l| !l.is_empty()) {
                a.push_all([key.as_str(), &format!("language={language}")]);
            }
            if let Some(title) = s.title.as_deref().filter(|t| !t.is_empty()) {
                a.push_all([key.as_str(), &format!("title={title}")]);
            }
        };
        if let Some(sound) = self.audio.as_ref().and_then(|au| media.stream(au.stream_index)) {
            name(&mut a, "a:0", sound);
        }
        let mut n = 0;
        for s in media.streams.iter().filter(|s| s.kind == StreamKind::Subtitle) {
            let codec = match (container, s.codec.as_str()) {
                (FileContainer::Matroska, c) if MATROSKA_SUBTITLES.contains(&c) => "copy",
                (FileContainer::Matroska, "mov_text") => "srt",
                (FileContainer::Mp4, c) if MP4_TEXT_SUBTITLES.contains(&c) => "mov_text",
                // Teletext and the like, and pictures in MP4: no place for them.
                _ => continue,
            };
            a.push_all(["-map", &format!("0:{}", s.index), &format!("-c:s:{n}"), codec]);
            name(&mut a, &format!("s:{n}"), s);
            n += 1;
        }
        match container {
            FileContainer::Matroska => a.push_all(["-f", "matroska"]),
            FileContainer::Mp4 => {
                let hevc = self.video.as_ref().is_some_and(|v| {
                    if v.copy {
                        media.stream(v.stream_index).is_some_and(|s| s.codec == "hevc")
                    } else {
                        v.codec == VideoCodec::Hevc
                    }
                });
                // QuickTime plays HEVC tagged hvc1 only (FFmpeg writes hev1).
                if hevc {
                    a.push_all(["-tag:v", "hvc1"]);
                }
                // The index first: it plays as soon as it opens, from anywhere.
                a.push_all(["-movflags", "+faststart", "-f", "mp4"]);
            }
        }
        a.push_os(output.as_os_str().to_owned());
        a.0
    }

    /// Everything but the output: devices, inputs, the video and audio
    /// streams, from segment `start` up to `end`.
    fn stream_args(&self, ctx: &ArgsContext<'_>, start: u32, end: u32) -> Args {
        let start_ms = self.boundaries[start as usize];
        let end_ms = self.boundaries[end as usize];
        let copy = self.video.as_ref().is_some_and(|v| v.copy);
        // Remux seeks land on the keyframe at or before the seek point, which
        // sits a little past the boundary keyframe. The output starts at that
        // keyframe (the boundary), so the split times count from there.
        let seek_ms = if copy { self.seek_at[start as usize] } else { start_ms };
        let encoder = ctx.encoder.or_else(|| self.video.as_ref().and_then(|v| v.encoder.as_ref()));
        let pipeline = self.video.as_ref().map_or(Pipeline::Software, |v| {
            if ctx.encoder.is_some() { v.pipeline.for_software_encoder() } else { v.pipeline }
        });

        let mut a = Args::default();
        a.push_all(["-loglevel", "error", "-nostats", "-progress", "pipe:2", "-y"]);

        // Global hardware devices and decoding.
        if let Some(v) = self.video.as_ref().filter(|v| !v.copy) {
            match pipeline {
                Pipeline::Vulkan { .. } => a.push_all([
                    "-init_hw_device",
                    "vulkan=vk",
                    "-filter_hw_device",
                    "vk",
                    "-hwaccel",
                    "vulkan",
                    "-hwaccel_device",
                    "vk",
                    "-hwaccel_output_format",
                    "vulkan",
                ]),
                Pipeline::Gpu(HwAccel::Vaapi) => a.push_all([
                    "-hwaccel",
                    "vaapi",
                    "-hwaccel_device",
                    VAAPI_DEVICE,
                    "-hwaccel_output_format",
                    "vaapi",
                ]),
                Pipeline::Gpu(HwAccel::Qsv) => a.push_all(["-hwaccel", "qsv", "-hwaccel_output_format", "qsv"]),
                Pipeline::Gpu(HwAccel::Videotoolbox) => {
                    a.push_all(["-hwaccel", "videotoolbox", "-hwaccel_output_format", "videotoolbox_vld"])
                }
                Pipeline::Gpu(_) => {
                    a.push_all(["-hwaccel", "cuda", "-hwaccel_output_format", "cuda", "-extra_hw_frames", "8"])
                }
                Pipeline::Software => {
                    match v.tonemap {
                        Some(Tonemapper::Libplacebo) => {
                            a.push_all(["-init_hw_device", "vulkan=vk", "-filter_hw_device", "vk"])
                        }
                        Some(Tonemapper::Opencl) => {
                            a.push_all(["-init_hw_device", "opencl=ocl", "-filter_hw_device", "ocl"])
                        }
                        _ => {}
                    }
                    if encoder.is_some_and(|e| e.accel == HwAccel::Vaapi) {
                        a.push_all(["-vaapi_device", VAAPI_DEVICE]);
                    }
                    if let Some(dec) = &v.hw_decode {
                        a.push_all(["-hwaccel", dec.as_str()]);
                    }
                }
            }
        }

        // Main input.
        if self.throttle {
            let (rate, burst) = (trim_float(READRATE), (READRATE_BURST_MS / 1000).to_string());
            a.push_all(["-readrate", &rate, "-readrate_initial_burst", &burst]);
        }
        if seek_ms > 0 {
            // Converted audio starts at the keyframe like the copied video,
            // instead of at the seek point: no hole where an older process's
            // segment plays into this one's first.
            if copy {
                a.push("-noaccurate_seek");
            }
            a.push_all(["-ss", &secs(seek_ms)]);
        }
        a.push_all(MEDIA_INPUT_OPTIONS);
        a.push("-i");
        a.push_os(input_arg(ctx.input));

        // Optional bitmap subtitle file as a second input.
        if let Some(Burn::ImageFile { path, .. }) = &self.burn {
            if seek_ms > 0 {
                a.push_all(["-ss", &secs(seek_ms)]);
            }
            a.push_all(MEDIA_INPUT_OPTIONS);
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
                let graph = self.filter_graph(v, encoder, pipeline, start_ms, ctx.shader_cache);
                a.push_all(["-filter_complex", &graph, "-map", "[vout]"]);
                if let Some(enc) = encoder {
                    a.extend(encoder_args(enc, v, ctx.preset));
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
                        au.codec.as_str(),
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
            // (Counted from the seek point.)
            a.push_all(["-t", &secs(end_ms - seek_ms)]);
        }
        a
    }

    fn filter_graph(
        &self,
        v: &VideoOut,
        encoder: Option<&EncoderInfo>,
        pipeline: Pipeline,
        start_ms: i64,
        shader_cache: Option<&Path>,
    ) -> String {
        let input = format!("[0:{}]", v.stream_index);
        let (w, h) = (v.width, v.height);
        // The pixel format the encoder gets (from the CPU, or downloaded).
        let ten_bit = v.hdr.map(|_| ten_bit_format(encoder.map_or(HwAccel::None, |e| e.accel)));
        // Zero-copy: one GPU scaler converts to NV12 (P010 for HDR) for the encoder.
        if let Pipeline::Gpu(accel) = pipeline {
            let format = if v.hdr.is_some() { "p010le" } else { "nv12" };
            let scaler = match accel {
                HwAccel::Vaapi => format!("scale_vaapi=w={w}:h={h}:format={format}"),
                HwAccel::Qsv => format!("vpp_qsv=w={w}:h={h}:format={format}"),
                HwAccel::Videotoolbox => format!("scale_vt=w={w}:h={h}"),
                _ => format!("scale_cuda=w={w}:h={h}:format={format}"),
            };
            return format!("{input}{scaler}[vout]");
        }

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
                let vulkan = matches!(pipeline, Pipeline::Vulkan { .. });
                let format = match (vulkan, v.hdr.is_some()) {
                    (true, false) => "nv12",
                    (true, true) => "p010",
                    (false, false) => "yuv420p",
                    (false, true) => "p010le",
                };
                // To SDR (tone mapping), or to the HDR format kept.
                let target = match v.hdr {
                    Some(hdr) => format!("colorspace=bt2020nc:color_primaries=bt2020:color_trc={}", hdr.transfer()),
                    None => "colorspace=bt709:color_primaries=bt709:color_trc=bt709".to_owned(),
                };
                // Shaders compiled by earlier processes save ~0.2 s at the
                // start of each one.
                let cache = shader_cache.and_then(shader_cache_option).map(|o| format!(":{o}")).unwrap_or_default();
                pre.push(format!("libplacebo=w={w}:h={h}:format={format}:{target}:range=tv:tonemapping=auto{cache}"));
                match pipeline {
                    // Frames stay in Vulkan memory all the way into h264_vulkan.
                    Pipeline::Vulkan { encode_on_gpu: true } => return format!("{input}{}[vout]", pre.join(",")),
                    // One download of the (already scaled) frame.
                    Pipeline::Vulkan { encode_on_gpu: false } => pre.push(format!("hwdownload,format={format}")),
                    _ => {}
                }
                if overlay_needed && scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
            Some(Tonemapper::Opencl) => {
                pre.extend(v.tonemap_input.clone());
                pre.push(OPENCL_TONEMAP.into());
                if scale_needed {
                    post.push(format!("scale={}:{}:flags=bicubic", v.width, v.height));
                }
            }
            Some(Tonemapper::Zscale) => {
                pre.extend(v.tonemap_input.clone());
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
        let vulkan = matches!(pipeline, Pipeline::Vulkan { .. });
        match ten_bit {
            _ if encoder.is_some_and(|e| e.accel == HwAccel::Vaapi) => {
                post.push(format!("format={},hwupload", if ten_bit.is_some() { "p010" } else { "nv12" }));
            }
            // The Vulkan path already delivers NV12, which every encoder
            // accepts; P010 for HDR, which x265 doesn't.
            None if vulkan => {}
            Some("p010le") if vulkan => {}
            Some(format) => post.push(format!("format={format}")),
            None => post.push("format=yuv420p".into()),
        }

        let join = |parts: &[String]| if parts.is_empty() { "null".to_owned() } else { parts.join(",") };
        // Picture subtitles come on a canvas of their own (1080p on 4K discs):
        // it is fitted to the frame's width, keeping its shape, and laid on
        // the bottom of the frame.
        let overlay = |subs: &str| {
            format!(
                "{input}{}[base];{subs}scale={}:-1[subs];[base][subs]overlay=x=(W-w)/2:y=H-h:eof_action=pass:repeatlast=0[ov];[ov]{}[vout]",
                join(&pre),
                v.src_width,
                join(&post)
            )
        };
        match &self.burn {
            Some(Burn::Image { stream_index, .. }) => overlay(&format!("[0:{stream_index}]")),
            Some(Burn::ImageFile { .. }) => overlay("[1:0]"),
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
    /// The software encoder's preset for this process (see [`crate::adaptive`]).
    pub preset: Option<&'a str>,
    /// Folder for libplacebo's compiled shaders (see [`shader_cache_option`]).
    pub shader_cache: Option<&'a Path>,
}

/// Subtitle codecs a Matroska file holds as they are.
const MATROSKA_SUBTITLES: [&str; 7] =
    ["subrip", "ass", "ssa", "webvtt", "hdmv_pgs_subtitle", "dvd_subtitle", "dvb_subtitle"];

/// The text subtitles an MP4 conversion makes its own (`mov_text`).
const MP4_TEXT_SUBTITLES: [&str; 6] = ["subrip", "ass", "ssa", "webvtt", "mov_text", "text"];

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

fn encoder_args(enc: &EncoderInfo, v: &VideoOut, preset: Option<&str>) -> Vec<String> {
    let br = v.bitrate_kbps;
    // `auto` is the transcoder's to choose; one it didn't is a plain default.
    let preset = preset.unwrap_or(if v.preset == AUTO { "veryfast" } else { &v.preset }).to_owned();
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
    // HDR is kept in HEVC Main 10 (see `hdr_output`).
    let profile = match (h264, v.hdr.is_some()) {
        (true, _) => "high",
        (false, false) => "main",
        (false, true) => "main10",
    };
    let mut s: Vec<String> = vec!["-c:v".into(), enc.name.clone()];
    match enc.accel {
        HwAccel::Nvenc => {
            s.extend(
                ["-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", if h264 { "23" } else { "26" }].map(String::from),
            );
            rate(&mut s);
            s.extend(["-forced-idr", "1", "-no-scenecut", "1", "-spatial-aq", "1", "-g", "9999"].map(String::from));
            s.extend(["-profile:v", profile].map(String::from));
        }
        HwAccel::Qsv => {
            s.extend(["-preset", "veryfast", "-look_ahead", "0", "-forced_idr", "1", "-g", "9999"].map(String::from));
            rate(&mut s);
            s.extend(["-profile:v", profile].map(String::from));
        }
        HwAccel::Amf => {
            s.extend(
                ["-usage", "transcoding", "-quality", "balanced", "-rc", "vbr_peak", "-forced_idr", "1", "-g", "9999"]
                    .map(String::from),
            );
            rate(&mut s);
            s.extend(["-profile:v", profile].map(String::from));
        }
        HwAccel::Vaapi => {
            rate(&mut s);
            s.extend(["-g", "9999"].map(String::from));
            if v.hdr.is_some() {
                s.extend(["-profile:v", profile].map(String::from));
            }
        }
        HwAccel::Videotoolbox => {
            rate(&mut s);
            s.extend(["-allow_sw", "1", "-realtime", "1", "-g", "9999"].map(String::from));
            if v.hdr.is_some() {
                s.extend(["-profile:v", profile].map(String::from));
            }
        }
        HwAccel::Vulkan => {
            s.extend(["-rc_mode", "vbr", "-tune", "hq", "-usage", "transcode"].map(String::from));
            rate(&mut s);
            // No B-frames: Chrome's decoder never starts on h264_vulkan's
            // (the first frame shows, then playback waits for good).
            s.extend(["-profile:v", "high", "-bf", "0", "-g", "9999"].map(String::from));
        }
        HwAccel::None | HwAccel::Auto => {
            if h264 {
                s.extend(["-preset".into(), preset, "-crf".into(), "21".into()]);
                s.extend(["-maxrate".into(), format!("{br}k"), "-bufsize".into(), format!("{}k", br * 2)]);
                s.extend(
                    ["-profile:v", "high", "-sc_threshold", "0", "-forced-idr", "1", "-g", "9999"].map(String::from),
                );
            } else {
                s.extend(["-preset".into(), preset, "-crf".into(), "24".into()]);
                s.extend(["-maxrate".into(), format!("{br}k"), "-bufsize".into(), format!("{}k", br * 2)]);
                // Parameter sets (and the HDR10 metadata) at every keyframe:
                // each segment starts on one.
                s.extend(
                    ["-x265-params", "log-level=error:scenecut=0:open-gop=0:keyint=9999:repeat-headers=1"]
                        .map(String::from),
                );
                if v.hdr.is_some() {
                    // A source's Dolby Vision metadata stays out.
                    s.extend(["-profile:v", profile, "-dolbyvision", "0"].map(String::from));
                }
            }
        }
    }
    if let Some(hdr) = v.hdr {
        // Said in the stream itself, whatever the frames were tagged with.
        s.extend(
            ["-color_primaries", "bt2020", "-color_trc", hdr.transfer(), "-colorspace", "bt2020nc"].map(String::from),
        );
    }
    s
}

fn gpu_decoder_name(accel: HwAccel) -> &'static str {
    match accel {
        HwAccel::Vaapi => "vaapi",
        HwAccel::Qsv => "qsv",
        HwAccel::Videotoolbox => "videotoolbox",
        HwAccel::Vulkan => "vulkan",
        _ => "cuda",
    }
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

/// A codec as the transcoder's warnings name it.
fn codec_name(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Aac => "AAC",
        AudioCodec::Ac3 => "AC-3",
        AudioCodec::Eac3 => "E-AC-3",
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

/// libplacebo's `shader_cache` option for a folder, written for a
/// filtergraph. The value is a prefix of file names, hence the trailing
/// separator. A drive's colon is escaped twice: once for the filter's
/// options, once for the graph. Folders whose names hold other characters
/// that either level reads (`, ; [ ] ' : \`) are left without a cache.
pub fn shader_cache_option(dir: &Path) -> Option<String> {
    let path = dir.to_str()?;
    let path = if cfg!(windows) { path.replace('\\', "/") } else { path.to_owned() };
    let (drive, rest) = match path.as_bytes() {
        [letter, b':', ..] if letter.is_ascii_alphabetic() => (&path[..1], &path[2..]),
        _ => ("", path.as_str()),
    };
    if rest.is_empty() || rest.contains([',', ';', '[', ']', '\'', '"', ':', '\\']) {
        return None;
    }
    let drive = if drive.is_empty() { String::new() } else { format!("{drive}\\\\:") };
    let slash = if rest.ends_with('/') { "" } else { "/" };
    Some(format!("shader_cache={drive}{rest}{slash}"))
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

/// Longest nudge past a remux boundary: just more than FFmpeg's back-off of
/// 3/23 s (130 ms).
const SEEK_NUDGE_MS: i64 = 140;

/// Whether FFmpeg seeks this file 3/23 s before the point asked for: it does
/// when a stream has B-frames and the demuxer doesn't seek by presentation
/// time (all but MP4 and MOV), to be sure to land on a keyframe whose
/// decoding timestamp comes first. Unknown B-frames (media info from an
/// older probe) count as B-frames.
pub fn seeks_back(media: &MediaInfo) -> bool {
    let seeks_by_pts = match media.format_name.as_deref() {
        Some(name) => name.split(',').any(|n| n == "mov"),
        None => matches!(media.container.as_deref(), Some("mp4" | "mov" | "m4v")),
    };
    !seeks_by_pts && media.streams.iter().any(|s| s.kind == StreamKind::Video && s.has_b_frames != Some(false))
}

/// Where processes starting at each remux boundary seek. A seek lands on the
/// keyframe at or before its point, and stream copy keeps everything from
/// there, so a seek right at a boundary keyframe that FFmpeg moves back
/// (see [`seeks_back`]) would start one GOP early, in the previous segment.
/// It goes past the back-off then, but no further than halfway to the next
/// keyframe, which must not be the one found. Otherwise 1 ms past the
/// boundary is enough: rounding never selects the previous keyframe.
pub fn remux_seek_points(boundaries: &[i64], keyframes: &[i64], seeks_back: bool) -> Vec<i64> {
    let last = boundaries.len().saturating_sub(1);
    boundaries
        .iter()
        .enumerate()
        .map(|(k, &b)| {
            if k == 0 || k == last {
                return b;
            }
            if !seeks_back {
                return b + 1;
            }
            let next = keyframes.get(keyframes.partition_point(|&kf| kf <= b)).copied().unwrap_or(boundaries[last]);
            b + ((next - b) / 2).clamp(1, SEEK_NUDGE_MS)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::MEDIA_DEMUXERS;
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
        EncoderInfo {
            name: "libx264".into(),
            codec: VideoCodec::H264,
            hardware: false,
            accel: HwAccel::None,
            main10: false,
        }
    }

    fn nvenc() -> EncoderInfo {
        EncoderInfo {
            name: "h264_nvenc".into(),
            codec: VideoCodec::H264,
            hardware: true,
            accel: HwAccel::Nvenc,
            main10: false,
        }
    }

    fn hevc_nvenc() -> EncoderInfo {
        EncoderInfo { name: "hevc_nvenc".into(), codec: VideoCodec::Hevc, main10: true, ..nvenc() }
    }

    fn x265() -> EncoderInfo {
        EncoderInfo { name: "libx265".into(), codec: VideoCodec::Hevc, main10: true, ..x264() }
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
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: None,
            relative_split_times: true,
            preset: None,
            shader_cache: None,
        };
        plan.ffmpeg_args(&ctx, start, end)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_conversion_is_one_matroska_file_of_the_whole_media() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.options.throttle = false;
        let mut media = media(DynamicRange::Sdr, None);
        media.streams[1].language = Some("eng".into());
        (media.streams[3].language, media.streams[3].title) = (Some("fre".into()), Some("Forced".into()));
        media.streams.push(MediaStream {
            index: 4,
            kind: StreamKind::Subtitle,
            codec: "dvb_teletext".into(),
            ..Default::default()
        });
        media.streams.push(MediaStream {
            index: 5,
            kind: StreamKind::Subtitle,
            codec: "mov_text".into(),
            text_based: Some(true),
            ..Default::default()
        });
        let plan = Plan::build(&req, &media, &ff, None).unwrap();
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: None,
            relative_split_times: true,
            preset: Some("fast"),
            shader_cache: None,
        };
        let args: Vec<String> = plan
            .file_args(&ctx, &media, Path::new("/work/conversions/abc.mkv.part"), FileContainer::Matroska)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let line = args.join(" ");
        assert!(line.ends_with("-f matroska /work/conversions/abc.mkv.part"), "{line}");
        assert!(line.contains("-c:v libx264") && line.contains("-preset fast"), "{line}");
        assert!(line.contains("-c:a aac"), "{line}");
        // The subtitles as they are, in their order; teletext has no place in Matroska.
        assert!(
            line.contains(
                "-map 0:2 -c:s:0 copy -map 0:3 -c:s:1 copy -metadata:s:s:1 language=fre -metadata:s:s:1 title=Forced"
            ),
            "{line}"
        );
        assert!(line.contains("-map 0:5 -c:s:2 srt -f matroska"), "{line}");
        assert!(line.contains("-metadata:s:a:0 language=eng"), "{line}");
        for absent in ["-segment", "-ss ", "-t ", "-readrate", "-output_ts_offset"] {
            assert!(!line.contains(absent), "{absent} in {line}");
        }
    }

    #[test]
    fn a_conversion_to_mp4_makes_text_subtitles_its_own_and_tags_hevc_for_quicktime() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut media = media(DynamicRange::Sdr, None);
        media.streams.push(MediaStream {
            index: 4,
            kind: StreamKind::Subtitle,
            codec: "ass".into(),
            text_based: Some(true),
            ..Default::default()
        });
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: None,
            relative_split_times: true,
            preset: Some("fast"),
            shader_cache: None,
        };
        let line = |req: &JobRequest| {
            let plan = Plan::build(req, &media, &ff, Some(&[0])).unwrap();
            plan.file_args(&ctx, &media, Path::new("/work/conversions/abc.mp4.part"), FileContainer::Mp4)
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        };

        // The HEVC copied: tagged hvc1, as QuickTime wants it.
        let mut copy = request();
        copy.video.as_mut().unwrap().mode = VideoMode::Copy;
        let copied = line(&copy);
        assert!(copied.contains("-c:v copy"), "{copied}");
        assert!(copied.ends_with("-tag:v hvc1 -movflags +faststart -f mp4 /work/conversions/abc.mp4.part"), "{copied}");
        // SubRip and ASS made MP4's own text, in their order; the PGS pictures have no place.
        assert!(!copied.contains("0:2"), "{copied}");
        assert!(copied.contains("-map 0:3 -c:s:0 mov_text"), "{copied}");
        assert!(copied.contains("-map 0:4 -c:s:1 mov_text"), "{copied}");

        // Made H.264: no tag to change.
        let made = line(&request());
        assert!(made.contains("-c:v libx264"), "{made}");
        assert!(!made.contains("hvc1"), "{made}");
        assert!(made.ends_with("-movflags +faststart -f mp4 /work/conversions/abc.mp4.part"), "{made}");
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
    fn segment_counts_are_bounded() {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut m = media(DynamicRange::Sdr, None);
        // Damaged or crafted files: 31 years, 31 700 years.
        for secs in [1_000_000_000i64, 1_000_000_000_000] {
            m.duration_ms = Some(secs * 1000);
            let err = Plan::build(&request(), &m, &ff, None).unwrap_err();
            assert!(err.contains("not plausible"), "{err}");
        }
        // Nine days in 1 s segments would be 800 000 of them: segments get
        // longer instead.
        m.duration_ms = Some(MAX_DURATION_MS);
        let mut req = request();
        req.segment_seconds = Some(1.0);
        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        assert_eq!(i64::from(plan.segment_count()), MAX_SEGMENTS);
        assert_eq!(plan.segment_seconds, 4.0);
        assert!(args_string(&plan, 0, 2).contains("-force_key_frames expr:gte(t,n_forced*4)"));
        // Ordinary lengths keep the segments they asked for.
        m.duration_ms = Some(3 * 3_600_000);
        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        assert_eq!((plan.segment_seconds, plan.segment_count()), (1.0, 10_800));
    }

    #[test]
    fn segment_lookup() {
        let (boundaries, split_at) = uniform_layout(10_000, 4_000);
        let plan = Plan {
            duration_ms: 10_000,
            seek_at: boundaries.clone(),
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
        // Only media demuxers may open the file (no playlists pointing elsewhere).
        assert!(
            args.contains(&format!("-ss 12.000 -format_whitelist {MEDIA_DEMUXERS} -i file:/media/movie.mkv")),
            "{args}"
        );
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
    fn libplacebo_reuses_compiled_shaders() {
        // A prefix of file names in the folder; the drive's colon escaped for
        // the filter's options and again for the graph.
        assert_eq!(
            shader_cache_option(Path::new("C:/Users/Jo Smith/pley-transcoder/libplacebo")).as_deref(),
            Some(r"shader_cache=C\\:/Users/Jo Smith/pley-transcoder/libplacebo/")
        );
        assert_eq!(
            shader_cache_option(Path::new("/var/cache/pley-transcoder/libplacebo/")).as_deref(),
            Some("shader_cache=/var/cache/pley-transcoder/libplacebo/")
        );
        if cfg!(windows) {
            assert_eq!(
                shader_cache_option(Path::new(r"D:\cache\libplacebo")).as_deref(),
                Some(r"shader_cache=D\\:/cache/libplacebo/")
            );
        }
        // What the graph or the options would read: no cache rather than a failed job.
        for odd in ["/a,b/libplacebo", "/a;b", "/[x]", "/it's", "/a:b", "C:/a:b", ""] {
            assert_eq!(shader_cache_option(Path::new(odd)), None, "{odd}");
        }
        if !cfg!(windows) {
            assert_eq!(shader_cache_option(Path::new(r"/a\b")), None, "a backslash in a name");
        }

        let ff = ffmpeg_with(vec![nvenc(), x264()], vec![Tonemapper::Libplacebo]);
        let plan = Plan::build(&request(), &media(DynamicRange::Hdr10, None), &ff, None).unwrap();
        let cache = Path::new("/var/cache/pley-transcoder/libplacebo");
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: None,
            relative_split_times: true,
            preset: None,
            shader_cache: Some(cache),
        };
        let args: Vec<String> = plan.ffmpeg_args(&ctx, 0, 2).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(
            args.join(" ").contains("tonemapping=auto:shader_cache=/var/cache/pley-transcoder/libplacebo/,"),
            "{args:?}"
        );
        // Other tone mappers have no such cache.
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let plan = Plan::build(&request(), &media(DynamicRange::Hdr10, None), &ff, None).unwrap();
        let args: Vec<String> = plan.ffmpeg_args(&ctx, 0, 2).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!args.join(" ").contains("shader_cache"));
    }

    fn ten_bit(mut m: MediaInfo) -> MediaInfo {
        m.streams[0].bit_depth = Some(10);
        m
    }

    #[test]
    fn hdr_tone_maps_on_vulkan_and_encodes_with_the_usual_encoder() {
        let mut ff = ffmpeg_with(vec![nvenc(), x264()], vec![Tonemapper::Libplacebo]);
        ff.caps.vulkan_decode = vec!["hevc10".into()];
        ff.caps.vulkan_encode = true;
        let m = ten_bit(media(DynamicRange::DolbyVision, Some(5)));

        // Automatic: decoded and tone mapped on Vulkan, encoded with NVENC
        // after one download (browsers don't play h264_vulkan's 4K).
        let plan = Plan::build(&request(), &m, &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().pipeline, Pipeline::Vulkan { encode_on_gpu: false });
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("-hwaccel vulkan -hwaccel_device vk -hwaccel_output_format vulkan"), "{args}");
        assert!(args.contains("tonemapping=auto,hwdownload,format=nv12[vout]"), "{args}");
        assert!(args.contains("-c:v h264_nvenc"), "{args}");
        assert_eq!(plan.video_plan(&m, None).unwrap().pipeline.as_deref(), Some("hybrid"));

        // Asked for Vulkan: everything stays on it.
        let mut req = request();
        req.options.hwaccel = HwAccel::Vulkan;
        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        let v = plan.video.as_ref().unwrap();
        assert_eq!(v.pipeline, Pipeline::Vulkan { encode_on_gpu: true });
        assert_eq!(v.encoder.as_ref().unwrap().name, "h264_vulkan");
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("-hwaccel vulkan -hwaccel_device vk -hwaccel_output_format vulkan"), "{args}");
        assert!(args.contains("libplacebo=w=1920:h=1080:format=nv12"));
        assert!(!args.contains("hwdownload"));
        assert!(args.contains("-c:v h264_vulkan -rc_mode vbr"));
        assert!(args.contains("-bf 0"), "browsers don't play its B-frames: {args}");
        assert_eq!(plan.video_plan(&m, None).unwrap().pipeline.as_deref(), Some("gpu"));

        // An explicit NVENC preference keeps NVENC: one download after tone mapping.
        let mut req = request();
        req.options.hwaccel = HwAccel::Nvenc;
        let plan = Plan::build(&req, &m, &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().pipeline, Pipeline::Vulkan { encode_on_gpu: false });
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("tonemapping=auto,hwdownload,format=nv12[vout]"), "{args}");
        assert!(args.contains("-c:v h264_nvenc"));
        assert_eq!(plan.video_plan(&m, None).unwrap().pipeline.as_deref(), Some("hybrid"));

        // Codecs that failed the self-test stay on the copy-through path.
        let plan = Plan::build(&request(), &media(DynamicRange::Hdr10, None), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().pipeline, Pipeline::Software);
    }

    #[test]
    fn sdr_uses_cuda_zero_copy_and_falls_back_with_software_encoder() {
        let mut ff = ffmpeg_with(vec![nvenc(), x264()], vec![]);
        ff.caps.gpu_pipelines =
            vec![pley_transcoder_api::GpuPipeline { accel: HwAccel::Nvenc, codecs: vec!["hevc".into()] }];
        let plan = Plan::build(&request(), &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().pipeline, Pipeline::Gpu(HwAccel::Nvenc));
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("-hwaccel cuda -hwaccel_output_format cuda"), "{args}");
        assert!(args.contains("[0:0]scale_cuda=w=1920:h=1080:format=nv12[vout]"));
        assert!(args.contains("-c:v h264_nvenc"));

        // Hardware sessions exhausted → software encoder → no GPU frames.
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: Some(&ff.caps.encoders[1]),
            relative_split_times: true,
            preset: None,
            shader_cache: None,
        };
        let args: Vec<String> = plan.ffmpeg_args(&ctx, 0, 2).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let args = args.join(" ");
        assert!(!args.contains("hwaccel_output_format"), "{args}");
        assert!(args.contains("scale=1920:1080:flags=bicubic,format=yuv420p"));
        assert!(args.contains("-c:v libx264"));

        // Burn-in needs CPU frames.
        let mut req = request();
        req.subtitle = Some(SubtitleBurnRequest { stream_index: Some(2), external_path: None });
        let plan = Plan::build(&req, &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().pipeline, Pipeline::Software);
    }

    #[test]
    fn dolby_vision_profile5_requires_libplacebo() {
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(5)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Zscale));
        assert!(plan.warnings.iter().any(|w| w.contains("Dolby Vision")));
        // Its frames are untagged, which zimg refuses: they are tagged PQ
        // BT.2020 so the transcode works (with off colours, as warned).
        let args = args_string(&plan, 0, 2);
        assert!(
            args.contains("setparams=color_trc=smpte2084:color_primaries=bt2020:colorspace=bt2020nc,zscale=t=linear"),
            "{args}"
        );

        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Opencl, Tonemapper::Libplacebo]);
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(5)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Libplacebo));
        assert!(!args_string(&plan, 0, 2).contains("setparams"), "libplacebo reads the RPU");
        let plan = Plan::build(&request(), &media(DynamicRange::DolbyVision, Some(8)), &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Opencl));
        assert!(args_string(&plan, 0, 2).contains("setparams=color_trc=smpte2084"));
    }

    #[test]
    fn only_unknown_colour_properties_are_filled_in() {
        let mut stream = MediaStream { kind: StreamKind::Video, ..Default::default() };
        assert_eq!(
            untagged_hdr_fix(&stream).as_deref(),
            Some("setparams=color_trc=smpte2084:color_primaries=bt2020:colorspace=bt2020nc")
        );
        stream.color_transfer = Some("arib-std-b67".into());
        stream.color_primaries = Some("bt2020".into());
        stream.color_space = Some("unknown".into());
        assert_eq!(untagged_hdr_fix(&stream).as_deref(), Some("setparams=colorspace=bt2020nc"));
        stream.color_space = Some("bt2020nc".into());
        assert_eq!(untagged_hdr_fix(&stream), None, "HLG keeps its transfer");
        // Tagged HDR10 through zimg: nothing added.
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let mut m = media(DynamicRange::Hdr10, None);
        m.streams[0].color_transfer = Some("smpte2084".into());
        m.streams[0].color_primaries = Some("bt2020".into());
        m.streams[0].color_space = Some("bt2020nc".into());
        let plan = Plan::build(&request(), &m, &ff, None).unwrap();
        assert!(!args_string(&plan, 0, 2).contains("setparams"));
    }

    /// A request for HEVC in `range`, at 1080p.
    fn hdr_request(range: pley_transcoder_api::OutputRange) -> JobRequest {
        let mut req = request();
        let video = req.video.as_mut().unwrap();
        (video.codec, video.dynamic_range) = (VideoCodec::Hevc, range);
        req
    }

    /// NVENC for H.264 and HEVC (Main 10), x264 and x265, libplacebo on
    /// Vulkan, 10-bit HEVC decoded straight into CUDA and Vulkan memory.
    fn hdr_ffmpeg() -> Ffmpeg {
        let mut ff = ffmpeg_with(vec![nvenc(), hevc_nvenc(), x264(), x265()], vec![Tonemapper::Libplacebo]);
        ff.caps.vulkan_decode = vec!["hevc10".into()];
        ff.caps.gpu_pipelines =
            vec![pley_transcoder_api::GpuPipeline { accel: HwAccel::Nvenc, codecs: vec!["hevc10".into()] }];
        ff
    }

    #[test]
    fn hdr10_is_kept_in_hevc_main_10_on_the_gpu() {
        use pley_transcoder_api::OutputRange;
        let ff = hdr_ffmpeg();
        let m = ten_bit(media(DynamicRange::Hdr10, None));
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &m, &ff, None).unwrap();
        let v = plan.video.as_ref().unwrap();
        assert_eq!(v.hdr, Some(HdrOut { range: DynamicRange::Hdr10, convert: false }));
        assert_eq!((v.tonemap, v.pipeline), (None, Pipeline::Gpu(HwAccel::Nvenc)));
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
        let args = args_string(&plan, 0, 2);
        // Decoded, scaled to 10 bits and encoded without leaving the GPU;
        // the frames' HDR10 metadata goes with them to the encoder.
        assert!(args.contains("-hwaccel cuda -hwaccel_output_format cuda"), "{args}");
        assert!(args.contains("[0:0]scale_cuda=w=1920:h=1080:format=p010le[vout]"), "{args}");
        assert!(args.contains("-c:v hevc_nvenc"), "{args}");
        assert!(args.contains("-profile:v main10"), "{args}");
        assert!(args.contains("-color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc"), "{args}");
        assert!(!args.contains("libplacebo"), "{args}");
        let vp = plan.video_plan(&m, None).unwrap();
        assert_eq!((vp.codec_out.as_str(), vp.dynamic_range_out), ("hevc", Some(DynamicRange::Hdr10)));
        assert_eq!((vp.encoder.as_deref(), vp.tonemap), (Some("hevc_nvenc"), None));

        // Hardware sessions ran out: x265 in 10 bits, from the CPU.
        let ctx = ArgsContext {
            input: Path::new("/media/movie.mkv"),
            encoder: Some(&ff.caps.encoders[3]),
            relative_split_times: true,
            preset: None,
            shader_cache: None,
        };
        let args: Vec<String> = plan.ffmpeg_args(&ctx, 0, 2).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let args = args.join(" ");
        assert!(args.contains("scale=1920:1080:flags=bicubic,format=yuv420p10le[vout]"), "{args}");
        assert!(args.contains("-c:v libx265"), "{args}");
        assert!(args.contains("repeat-headers=1 -profile:v main10 -dolbyvision 0"), "{args}");
        assert!(args.contains("-color_trc smpte2084"), "{args}");

        // Without a zero-copy pipeline: decoded on the GPU, scaled on the CPU.
        let mut ff = hdr_ffmpeg();
        ff.caps.gpu_pipelines.clear();
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &m, &ff, None).unwrap();
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("-hwaccel cuda ") && !args.contains("hwaccel_output_format"), "{args}");
        assert!(args.contains("scale=1920:1080:flags=bicubic,format=p010le[vout]"), "{args}");

        // Dolby Vision 8.1 and 7 are their HDR10 base layer: kept as it is,
        // the Dolby Vision metadata left out.
        for profile in [8, 7] {
            let m = ten_bit(media(DynamicRange::DolbyVision, Some(profile)));
            let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &m, &hdr_ffmpeg(), None).unwrap();
            let v = plan.video.as_ref().unwrap();
            assert_eq!((v.hdr.map(|h| h.convert), v.tonemap), (Some(false), None), "P{profile}");
        }
    }

    #[test]
    fn other_hdr_formats_are_converted_by_libplacebo() {
        use pley_transcoder_api::OutputRange;
        let ff = hdr_ffmpeg();
        // Dolby Vision profile 5 to HDR10: libplacebo applies the RPU, on
        // Vulkan, then one 10-bit download for NVENC.
        let p5 = ten_bit(media(DynamicRange::DolbyVision, Some(5)));
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &p5, &ff, None).unwrap();
        let v = plan.video.as_ref().unwrap();
        assert_eq!(v.hdr, Some(HdrOut { range: DynamicRange::Hdr10, convert: true }));
        assert_eq!((v.tonemap, v.pipeline), (Some(Tonemapper::Libplacebo), Pipeline::Vulkan { encode_on_gpu: false }));
        let args = args_string(&plan, 0, 2);
        assert!(
            args.contains(
                "libplacebo=w=1920:h=1080:format=p010:colorspace=bt2020nc:color_primaries=bt2020:color_trc=smpte2084:\
                 range=tv:tonemapping=auto,hwdownload,format=p010[vout]"
            ),
            "{args}"
        );
        assert!(args.contains("-c:v hevc_nvenc") && args.contains("-profile:v main10"), "{args}");

        // HLG to HDR10 for a screen that shows only HDR10; kept as it is for
        // one that shows HLG.
        let hlg = ten_bit(media(DynamicRange::Hlg, None));
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &hlg, &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().hdr, Some(HdrOut { range: DynamicRange::Hdr10, convert: true }));
        let plan = Plan::build(&hdr_request(OutputRange::Hlg), &hlg, &ff, None).unwrap();
        let v = plan.video.as_ref().unwrap();
        assert_eq!((v.hdr, v.tonemap), (Some(HdrOut { range: DynamicRange::Hlg, convert: false }), None));
        assert!(args_string(&plan, 0, 2).contains("-color_trc arib-std-b67"));
        // Dolby Vision 8.4 is HLG underneath.
        let mut p84 = ten_bit(media(DynamicRange::DolbyVision, Some(8)));
        p84.streams[0].dv_bl_compat = Some(4);
        let plan = Plan::build(&hdr_request(OutputRange::Hlg), &p84, &ff, None).unwrap();
        assert_eq!(plan.video.as_ref().unwrap().hdr.map(|h| h.convert), Some(false));
        // HDR10 to HLG, for a screen that shows HLG alone, in software
        // (no Vulkan decoding): libplacebo outputs 10 bits.
        let mut ff = hdr_ffmpeg();
        ff.caps.vulkan_decode.clear();
        let plan =
            Plan::build(&hdr_request(OutputRange::Hlg), &ten_bit(media(DynamicRange::Hdr10, None)), &ff, None).unwrap();
        let args = args_string(&plan, 0, 2);
        assert!(
            args.contains("libplacebo=w=1920:h=1080:format=p010le:colorspace=bt2020nc:color_primaries=bt2020:color_trc=arib-std-b67"),
            "{args}"
        );
        assert!(args.contains("format=p010le[vout]"), "{args}");
    }

    #[test]
    fn hdr_is_tone_mapped_when_it_cant_be_kept() {
        use pley_transcoder_api::OutputRange;
        let p5 = ten_bit(media(DynamicRange::DolbyVision, Some(5)));
        let sdr = |plan: &Plan| plan.video.as_ref().unwrap().hdr.is_none();
        // H.264 asked for.
        let mut req = hdr_request(OutputRange::Hdr10);
        req.video.as_mut().unwrap().codec = VideoCodec::H264;
        let plan = Plan::build(&req, &p5, &hdr_ffmpeg(), None).unwrap();
        assert!(sdr(&plan) && plan.warnings.iter().any(|w| w.contains("HEVC only")), "{:?}", plan.warnings);
        assert_eq!(plan.video.as_ref().unwrap().encoder.as_ref().unwrap().name, "h264_nvenc");
        // Subtitles drawn onto the video.
        let mut req = hdr_request(OutputRange::Hdr10);
        req.subtitle = Some(SubtitleBurnRequest { stream_index: Some(3), external_path: None });
        let plan = Plan::build(&req, &p5, &hdr_ffmpeg(), None).unwrap();
        assert!(sdr(&plan) && plan.warnings.iter().any(|w| w.contains("subtitles")), "{:?}", plan.warnings);
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Libplacebo));
        // A conversion without libplacebo.
        let mut ff = hdr_ffmpeg();
        ff.caps.tonemappers = vec![Tonemapper::Zscale];
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &p5, &ff, None).unwrap();
        assert!(sdr(&plan) && plan.warnings.iter().any(|w| w.contains("libplacebo")), "{:?}", plan.warnings);
        assert_eq!(plan.video.as_ref().unwrap().tonemap, Some(Tonemapper::Zscale));
        // ...though HDR10 itself needs none.
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &ten_bit(media(DynamicRange::Hdr10, None)), &ff, None)
            .unwrap();
        assert!(!sdr(&plan));
        // No 10-bit HEVC encoder: HEVC in SDR.
        let ff =
            ffmpeg_with(vec![nvenc(), EncoderInfo { main10: false, ..hevc_nvenc() }], vec![Tonemapper::Libplacebo]);
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &p5, &ff, None).unwrap();
        assert!(sdr(&plan) && plan.warnings.iter().any(|w| w.contains("10-bit")), "{:?}", plan.warnings);
        let args = args_string(&plan, 0, 2);
        assert!(args.contains("color_trc=bt709") && args.contains("-profile:v main "), "{args}");
        assert_eq!(plan.video_plan(&p5, None).unwrap().dynamic_range_out, Some(DynamicRange::Sdr));
        // SDR video, and SDR asked for: nothing to say.
        let plan = Plan::build(&hdr_request(OutputRange::Hdr10), &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        assert!(sdr(&plan) && plan.warnings.is_empty());
        let plan = Plan::build(&hdr_request(OutputRange::Sdr), &p5, &hdr_ffmpeg(), None).unwrap();
        assert!(sdr(&plan) && plan.warnings.is_empty(), "{:?}", plan.warnings);
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
        // Matroska with B-frames (or none known): past FFmpeg's back-off,
        // with the split for 12 s counted from the 8 s keyframe.
        assert!(args.contains("-noaccurate_seek -ss 8.140"), "{args}");
        assert!(args.contains("-output_ts_offset 8.140"), "{args}");
        assert!(args.contains("-segment_times 3.500 "), "{args}");
        assert!(args.contains("-t 7.860"), "ends at the 16 s boundary: {args}");
        assert!(args.contains("-c:v copy"));
        assert!(!args.contains("-force_key_frames"));
        assert!(!args_string(&plan, 0, 2).contains("-ss"), "nothing to seek at the start");

        // MP4 seeks by presentation time: no back-off to get past.
        let mut mp4 = media(DynamicRange::Sdr, None);
        mp4.format_name = Some("mov,mp4,m4a,3gp,3g2,mj2".into());
        let plan = Plan::build(&req, &mp4, &ff, Some(&kf)).unwrap();
        assert!(args_string(&plan, 2, 4).contains("-noaccurate_seek -ss 8.001"));
        // Neither does Matroska without B-frames.
        let mut no_b = media(DynamicRange::Sdr, None);
        no_b.streams[0].has_b_frames = Some(false);
        let plan = Plan::build(&req, &no_b, &ff, Some(&kf)).unwrap();
        assert!(args_string(&plan, 2, 4).contains("-ss 8.001"));

        // MPEG-TS style start time: keyframe timestamps are absolute, the layout is not.
        let mut ts = media(DynamicRange::Sdr, None);
        ts.start_ms = Some(600_000);
        let kf: Vec<i64> = (0..30).map(|i| 600_000 + i * 2_000).collect();
        let plan = Plan::build(&req, &ts, &ff, Some(&kf)).unwrap();
        assert!(plan.video.as_ref().unwrap().copy);
        assert_eq!(&plan.boundaries[..3], &[0, 4_000, 8_000]);
        assert_eq!(plan.segment_count(), 15);
    }

    #[test]
    fn remux_seeks_stay_between_the_boundary_and_the_next_keyframe() {
        let kf = [0, 1_000, 1_100, 1_200, 4_000, 4_200, 9_000];
        let boundaries = [0, 1_000, 4_000, 9_000, 10_000];
        // Past the 130 ms back-off where the next keyframe allows it,
        // halfway to it where it is closer.
        assert_eq!(remux_seek_points(&boundaries, &kf, true), vec![0, 1_050, 4_100, 9_140, 10_000]);
        assert_eq!(remux_seek_points(&boundaries, &kf, false), vec![0, 1_001, 4_001, 9_001, 10_000]);

        let mut m = media(DynamicRange::Sdr, None);
        assert!(seeks_back(&m), "Matroska, B-frames unknown");
        m.streams[0].has_b_frames = Some(true);
        m.format_name = Some("mpegts".into());
        assert!(seeks_back(&m));
        m.format_name = Some("mov,mp4,m4a,3gp,3g2,mj2".into());
        assert!(!seeks_back(&m), "MP4 seeks by presentation time");
        m.format_name = None;
        m.container = Some("mp4".into());
        assert!(!seeks_back(&m));
        m.container = Some("mkv".into());
        m.streams[0].has_b_frames = Some(false);
        assert!(!seeks_back(&m), "no B-frames, no back-off");
    }

    /// FFmpeg, when it is on the PATH (it isn't in CI).
    fn real_ffmpeg() -> bool {
        let found = std::process::Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success());
        if !found {
            eprintln!("ffmpeg not found: skipped");
        }
        found
    }

    /// First packet time of each stream kind (`v`, `a`) in an MPEG-TS file.
    fn first_packet(file: &Path, stream: &str) -> f64 {
        let out = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", stream, "-show_entries", "packet=pts_time", "-of", "csv=p=0"])
            .arg(file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().trim_end_matches(',').parse::<f64>().ok())
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn remux_restarts_start_their_segments_on_their_boundaries() {
        if !real_ffmpeg() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("b-frames.mkv");
        // H.264 with B-frames and a keyframe every second, AC3 audio.
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "lavfi", "-i", "testsrc2=s=320x240:r=24:d=12", "-f", "lavfi", "-i", "sine=r=48000:d=12"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-bf", "2", "-g", "24", "-sc_threshold", "0"])
            .args(["-c:a", "ac3"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(made.success());
        let probed = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
            .arg(&input)
            .output()
            .unwrap();
        let m = crate::probe::parse_ffprobe_json(&probed.stdout, &input).unwrap();
        assert_eq!(m.video().unwrap().has_b_frames, Some(true));

        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.video = Some(VideoRequest { mode: VideoMode::Copy, ..Default::default() });
        req.audio = Some(AudioRequest { mode: AudioMode::Transcode, ..Default::default() });
        req.segment_seconds = Some(2.0);
        req.options.throttle = false;
        let kf: Vec<i64> = (0..12).map(|s| s * 1_000).collect();
        let plan = Plan::build(&req, &m, &ff, Some(&kf)).unwrap();
        assert_eq!(&plan.boundaries[..4], &[0, 2_000, 4_000, 6_000]);

        // The whole file from the start, then from the boundary at 4 s.
        let ctx =
            ArgsContext { input: &input, encoder: None, relative_split_times: true, preset: None, shader_cache: None };
        let run = |name: &str, start: u32| {
            let out = dir.path().join(name);
            std::fs::create_dir(&out).unwrap();
            let args = plan.ffmpeg_args(&ctx, start, plan.segment_count());
            let ok = std::process::Command::new("ffmpeg").args(&args).current_dir(&out).output().unwrap();
            assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
            out
        };
        let whole = run("whole", 0);
        let resumed = run("resumed", 2);
        for n in 2..plan.segment_count() {
            let (a, b) = (whole.join(format!("{n}.ts")), resumed.join(format!("{n}.ts")));
            let (expected, got) = (first_packet(&a, "v"), first_packet(&b, "v"));
            assert!((expected - got).abs() < 0.001, "segment {n} starts at {got}, not {expected}");
        }
        // The converted audio starts with the video, not at the seek point.
        let first = resumed.join("2.ts");
        let (video, audio) = (first_packet(&first, "v"), first_packet(&first, "a"));
        assert!((audio - video).abs() < 0.1, "audio at {audio}, video at {video}");
    }

    #[test]
    fn image_subtitles_are_overlaid() {
        let ff = ffmpeg_with(vec![x264()], vec![Tonemapper::Zscale]);
        let mut req = request();
        req.subtitle = Some(SubtitleBurnRequest { stream_index: Some(2), external_path: None });
        let plan = Plan::build(&req, &media(DynamicRange::Sdr, None), &ff, None).unwrap();
        let args = args_string(&plan, 0, 2);
        // The subtitles' canvas fills the 4K frame's width, then both go to 1080p.
        assert!(args.contains("[0:2]scale=3840:-1[subs];[base][subs]overlay=x=(W-w)/2:y=H-h"), "{args}");
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

    /// `media` with its audio replaced by one track of `codec`.
    fn with_audio(codec: &str, channels: i64, sample_rate: i64) -> MediaInfo {
        let mut m = media(DynamicRange::Sdr, None);
        let a = &mut m.streams[1];
        (a.codec, a.channels, a.sample_rate) = (codec.into(), Some(channels), Some(sample_rate));
        m
    }

    fn audio_args(codec: AudioCodec, channels: Option<u32>, source: &MediaInfo) -> (Plan, String) {
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.audio = Some(AudioRequest { codec, channels, ..Default::default() });
        let plan = Plan::build(&req, source, &ff, None).unwrap();
        let args = args_string(&plan, 0, 2);
        (plan, args)
    }

    #[test]
    fn audio_is_converted_to_e_ac_3_or_ac_3_in_5_1() {
        let truehd = with_audio("truehd", 8, 48_000);
        let (plan, args) = audio_args(AudioCodec::Eac3, Some(6), &truehd);
        assert!(args.contains("-map 0:1 -c:a eac3 -ac 6 -b:a 640k"), "{args}");
        assert!(!args.contains("-ar 48000"), "already 48 kHz");
        let out = plan.audio_plan(&truehd).unwrap();
        assert_eq!((out.codec_out.as_str(), out.channels_out, out.bitrate_kbps), ("eac3", Some(6), Some(640)));

        // DTS at 96 kHz: AC-3 takes 48 kHz at most.
        let dts = with_audio("dts", 6, 96_000);
        let (_, args) = audio_args(AudioCodec::Ac3, Some(6), &dts);
        assert!(args.contains("-c:a ac3 -ac 6 -b:a 640k -ar 48000"), "{args}");

        // 7.1 asked for: the AC-3 family stops at 5.1, AAC doesn't.
        let (_, args) = audio_args(AudioCodec::Eac3, Some(8), &truehd);
        assert!(args.contains("-c:a eac3 -ac 6 -b:a 640k"), "{args}");
        let (_, args) = audio_args(AudioCodec::Aac, Some(8), &truehd);
        assert!(args.contains("-c:a aac -ac 8 -b:a 512k"), "{args}");

        // Never more channels than the source; stereo is 192 kbit/s.
        let stereo = with_audio("flac", 2, 44_100);
        let (_, args) = audio_args(AudioCodec::Eac3, Some(6), &stereo);
        assert!(args.contains("-c:a eac3 -ac 2 -b:a 192k"), "{args}");

        // Copies stay copies, and the default is still stereo AAC.
        let ff = ffmpeg_with(vec![x264()], vec![]);
        let mut req = request();
        req.audio = Some(AudioRequest { mode: AudioMode::Copy, codec: AudioCodec::Eac3, ..Default::default() });
        let plan = Plan::build(&req, &with_audio("ac3", 6, 48_000), &ff, None).unwrap();
        assert!(args_string(&plan, 0, 2).contains("-c:a copy"));
        let plan = Plan::build(&req, &truehd, &ff, None).unwrap();
        assert_eq!(plan.warnings, ["truehd audio cannot be copied into HLS, transcoding to E-AC-3"]);
        let (_, args) = audio_args(AudioCodec::default(), None, &truehd);
        assert!(args.contains("-c:a aac -ac 2 -b:a 192k"), "{args}");
    }

    /// One stream's fields as ffprobe reports them: `codec_name=eac3`, ….
    fn probe_stream(file: &Path, stream: &str, entries: &str) -> std::collections::HashMap<String, String> {
        let out = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", stream, "-show_entries"])
            .arg(format!("stream={entries}"))
            .args(["-of", "default=noprint_wrappers=1"])
            .arg(file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
            .collect()
    }

    #[test]
    fn surround_sources_become_e_ac_3_5_1_segments() {
        if !real_ffmpeg() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // Local samples made here: H.264 with a keyframe every second, and
        // 5.1 audio as AC-3 or DTS.
        for (name, codec) in [("ac3.mkv", "ac3"), ("dts.mkv", "dca")] {
            let input = dir.path().join(name);
            let made = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y"])
                .args(["-f", "lavfi", "-i", "testsrc2=s=320x240:r=24:d=6"])
                .args(["-f", "lavfi", "-i", "sine=f=440:r=48000:d=6"])
                .args(["-filter_complex", "[1:a]pan=5.1(side)|FL=c0|FR=c0|FC=c0|LFE=c0|SL=c0|SR=c0[a]"])
                .args(["-map", "0:v", "-map", "[a]"])
                .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "24", "-sc_threshold", "0"])
                .args(["-c:a", codec, "-strict", "-2"])
                .arg(&input)
                .status()
                .unwrap();
            assert!(made.success(), "{name}");
            let probed = std::process::Command::new("ffprobe")
                .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
                .arg(&input)
                .output()
                .unwrap();
            let m = crate::probe::parse_ffprobe_json(&probed.stdout, &input).unwrap();
            let source = m.default_audio().unwrap();
            assert_eq!((source.codec.as_str(), source.channels), (if codec == "dca" { "dts" } else { codec }, Some(6)));

            // A remux for a TV with a 5.1 output: the video copied, the audio
            // converted to E-AC-3 5.1.
            let ff = ffmpeg_with(vec![x264()], vec![]);
            let mut req = request();
            req.video = Some(VideoRequest { mode: VideoMode::Copy, ..Default::default() });
            req.audio = Some(AudioRequest { codec: AudioCodec::Eac3, channels: Some(6), ..Default::default() });
            req.segment_seconds = Some(2.0);
            req.options.throttle = false;
            let kf: Vec<i64> = (0..6).map(|s| s * 1_000).collect();
            let plan = Plan::build(&req, &m, &ff, Some(&kf)).unwrap();
            let out = dir.path().join(name.replace(".mkv", ""));
            std::fs::create_dir(&out).unwrap();
            let ctx = ArgsContext {
                input: &input,
                encoder: None,
                relative_split_times: true,
                preset: None,
                shader_cache: None,
            };
            let args = plan.ffmpeg_args(&ctx, 0, plan.segment_count());
            let ran = std::process::Command::new("ffmpeg").args(&args).current_dir(&out).output().unwrap();
            assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));

            for n in 0..plan.segment_count() {
                let segment = out.join(format!("{n}.ts"));
                let audio = probe_stream(&segment, "a", "codec_name,channels,channel_layout,sample_rate,bit_rate");
                assert_eq!(audio.get("codec_name").map(String::as_str), Some("eac3"), "{name} {n}: {audio:?}");
                assert_eq!(audio.get("channels").map(String::as_str), Some("6"), "{name} {n}: {audio:?}");
                assert!(audio.get("channel_layout").is_some_and(|l| l.starts_with("5.1")), "{audio:?}");
                assert_eq!(audio.get("sample_rate").map(String::as_str), Some("48000"), "{name} {n}");
                assert_eq!(audio.get("bit_rate").map(String::as_str), Some("640000"), "{name} {n}");
                let video = probe_stream(&segment, "v", "codec_name");
                assert_eq!(video.get("codec_name").map(String::as_str), Some("h264"), "copied");
            }
        }
    }
}
