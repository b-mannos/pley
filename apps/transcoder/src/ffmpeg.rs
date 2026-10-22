// SPDX-License-Identifier: LicenseRef-Restricted
//! Discovery of the local FFmpeg installation: version, working encoders
//! (test-encodes, not just "compiled in"), tone mappers, throttling support and
//! segment muxer semantics. Results are cached on disk keyed by the binaries'
//! size and modification time because FFmpeg start-up can be slow (antivirus
//! scanning of large static builds).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use pley_transcoder_api::{Capabilities, EncoderInfo, GpuPipeline, HwAccel, TonemapMode, Tonemapper, VideoCodec};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Semaphore;

use crate::util::{configure_child, resolve_executable};

/// Bump when detection logic changes so stale caches are ignored.
const DETECTION_REVISION: u32 = 5;

pub struct Ffmpeg {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub caps: Capabilities,
    /// `-segment_times` are measured from the first output packet rather than as
    /// absolute timestamps (true for every FFmpeg version tested so far).
    pub segment_times_relative: bool,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    key: String,
    caps: Capabilities,
    segment_times_relative: bool,
}

impl Ffmpeg {
    pub async fn detect(ffmpeg: &Path, ffprobe: &Path, cache_path: &Path, force: bool) -> Self {
        let ffmpeg = resolve_executable(ffmpeg).unwrap_or_else(|| ffmpeg.to_path_buf());
        let ffprobe = resolve_executable(ffprobe).unwrap_or_else(|| ffprobe.to_path_buf());
        let key = cache_key(&ffmpeg, &ffprobe);

        if !force {
            if let Some(cached) = read_cache(cache_path, &key) {
                tracing::info!(
                    "ffmpeg {} (cached capabilities): encoders [{}], tone mapping [{}]",
                    cached.caps.ffmpeg_version.as_deref().unwrap_or("?"),
                    encoder_names(&cached.caps),
                    tonemapper_names(&cached.caps),
                );
                return Self {
                    ffmpeg,
                    ffprobe,
                    caps: cached.caps,
                    segment_times_relative: cached.segment_times_relative,
                };
            }
        }

        let started = Instant::now();
        let (caps, relative) = detect_capabilities(&ffmpeg, &ffprobe).await;
        if caps.ffmpeg_available {
            tracing::info!(
                "ffmpeg {} detected in {:.1}s: encoders [{}], tone mapping [{}], dolby vision: {}",
                caps.ffmpeg_version.as_deref().unwrap_or("?"),
                started.elapsed().as_secs_f32(),
                encoder_names(&caps),
                tonemapper_names(&caps),
                caps.dolby_vision,
            );
            let file = CacheFile { key, caps: caps.clone(), segment_times_relative: relative };
            if let Ok(json) = serde_json::to_vec_pretty(&file) {
                let _ = std::fs::write(cache_path, json);
            }
        } else {
            tracing::error!("ffmpeg not found or not working at {}", ffmpeg.display());
        }
        Self { ffmpeg, ffprobe, caps, segment_times_relative: relative }
    }

    /// An ffmpeg command with the common flags applied.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.ffmpeg);
        configure_child(&mut cmd);
        cmd.arg("-hide_banner").arg("-nostdin");
        cmd
    }

    pub fn probe_command(&self) -> Command {
        let mut cmd = Command::new(&self.ffprobe);
        configure_child(&mut cmd);
        cmd.arg("-hide_banner");
        cmd
    }

    pub fn software_encoder(&self, codec: VideoCodec) -> Option<&EncoderInfo> {
        self.caps.encoders.iter().find(|e| e.codec == codec && !e.hardware)
    }

    /// Choose an encoder for `codec` honouring the requested acceleration.
    /// Returns the encoder plus a warning when the request could not be met.
    pub fn pick_encoder(&self, codec: VideoCodec, accel: HwAccel) -> (Option<&EncoderInfo>, Option<String>) {
        let hardware = |a: Option<HwAccel>| {
            self.caps.encoders.iter().find(|e| e.codec == codec && e.hardware && a.is_none_or(|a| e.accel == a))
        };
        match accel {
            HwAccel::None => (self.software_encoder(codec), None),
            HwAccel::Auto => (hardware(None).or_else(|| self.software_encoder(codec)), None),
            wanted => match hardware(Some(wanted)) {
                Some(enc) => (Some(enc), None),
                None => (
                    self.software_encoder(codec),
                    Some(format!("hardware encoder for {wanted:?} is not available, using software encoding")),
                ),
            },
        }
    }

    /// Choose a tone mapper. `dolby_vision_p5` requests a mapper that applies
    /// the Dolby Vision RPU (only libplacebo does).
    pub fn pick_tonemapper(&self, mode: TonemapMode, dolby_vision_p5: bool) -> (Option<Tonemapper>, Vec<String>) {
        let available = &self.caps.tonemappers;
        let mut warnings = Vec::new();
        let auto = || {
            if dolby_vision_p5 && available.contains(&Tonemapper::Libplacebo) {
                Some(Tonemapper::Libplacebo)
            } else {
                available.first().copied()
            }
        };
        let choice = match mode {
            TonemapMode::None => {
                warnings.push("tone mapping disabled: HDR video will look washed out".into());
                None
            }
            TonemapMode::Auto => auto(),
            specific => {
                let wanted = match specific {
                    TonemapMode::Libplacebo => Tonemapper::Libplacebo,
                    TonemapMode::Opencl => Tonemapper::Opencl,
                    _ => Tonemapper::Zscale,
                };
                if available.contains(&wanted) {
                    Some(wanted)
                } else {
                    warnings.push(format!("tone mapper {} is not available", wanted.as_str()));
                    auto()
                }
            }
        };
        if choice.is_none() && mode != TonemapMode::None {
            warnings.push("no tone mapper available: HDR video will look washed out".into());
        }
        if dolby_vision_p5 && choice.is_some() && choice != Some(Tonemapper::Libplacebo) {
            warnings.push("Dolby Vision profile 5 needs libplacebo (Vulkan) for correct colors".into());
        }
        (choice, warnings)
    }
}

fn encoder_names(caps: &Capabilities) -> String {
    caps.encoders.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ")
}

fn tonemapper_names(caps: &Capabilities) -> String {
    caps.tonemappers.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", ")
}

fn cache_key(ffmpeg: &Path, ffprobe: &Path) -> String {
    let stamp = |p: &Path| {
        std::fs::metadata(p)
            .map(|m| {
                let mtime =
                    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                format!("{}:{}:{}", p.display(), m.len(), mtime)
            })
            .unwrap_or_else(|_| format!("{}:missing", p.display()))
    };
    format!("rev{DETECTION_REVISION}|{}|{}|{}", env!("CARGO_PKG_VERSION"), stamp(ffmpeg), stamp(ffprobe))
}

fn read_cache(path: &Path, key: &str) -> Option<CacheFile> {
    let bytes = std::fs::read(path).ok()?;
    let file: CacheFile = serde_json::from_slice(&bytes).ok()?;
    (file.key == key && file.caps.ffmpeg_available).then_some(file)
}

struct Output {
    success: bool,
    text: String,
}

async fn run(program: &Path, args: &[&str], timeout: Duration) -> Option<Output> {
    let mut cmd = Command::new(program);
    configure_child(&mut cmd);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn().ok()?;
    let output = tokio::time::timeout(timeout, child.wait_with_output()).await.ok()?.ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Some(Output { success: output.status.success(), text })
}

/// Encoder candidates in order of preference, with the extra arguments needed
/// to drive a test encode.
fn encoder_candidates() -> Vec<(&'static str, VideoCodec, HwAccel)> {
    let mut list = Vec::new();
    for (codec, prefix, sw) in [(VideoCodec::H264, "h264", "libx264"), (VideoCodec::Hevc, "hevc", "libx265")] {
        if cfg!(any(windows, target_os = "linux")) {
            list.push((leak(format!("{prefix}_nvenc")), codec, HwAccel::Nvenc));
            list.push((leak(format!("{prefix}_qsv")), codec, HwAccel::Qsv));
            list.push((leak(format!("{prefix}_amf")), codec, HwAccel::Amf));
        }
        if cfg!(target_os = "linux") {
            list.push((leak(format!("{prefix}_vaapi")), codec, HwAccel::Vaapi));
        }
        if cfg!(target_os = "macos") {
            list.push((leak(format!("{prefix}_videotoolbox")), codec, HwAccel::Videotoolbox));
        }
        list.push((sw, codec, HwAccel::None));
    }
    list
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub const VAAPI_DEVICE: &str = "/dev/dri/renderD128";

async fn test_encoder(ffmpeg: &Path, name: &str, accel: HwAccel) -> bool {
    let mut args: Vec<&str> = vec!["-hide_banner", "-nostdin", "-loglevel", "error"];
    if accel == HwAccel::Vaapi {
        args.extend(["-vaapi_device", VAAPI_DEVICE]);
    }
    args.extend(["-f", "lavfi", "-i", "color=black:s=320x240:r=25:d=0.2", "-frames:v", "3"]);
    if accel == HwAccel::Vaapi {
        args.extend(["-vf", "format=nv12,hwupload"]);
    } else {
        args.extend(["-pix_fmt", "yuv420p"]);
    }
    args.extend(["-c:v", name, "-f", "null", "-"]);
    matches!(run(ffmpeg, &args, Duration::from_secs(30)).await, Some(Output { success: true, .. }))
}

const HDR_TAGS: &str = "setparams=color_trc=smpte2084:color_primaries=bt2020:colorspace=bt2020nc";

async fn test_filter(ffmpeg: &Path, pre: &[&str], vf: &str) -> bool {
    let mut args: Vec<&str> = vec!["-hide_banner", "-nostdin", "-loglevel", "error"];
    args.extend_from_slice(pre);
    args.extend([
        "-f",
        "lavfi",
        "-i",
        "color=gray:s=320x240:r=25:d=0.1",
        "-vf",
        vf,
        "-frames:v",
        "1",
        "-f",
        "null",
        "-",
    ]);
    matches!(run(ffmpeg, &args, Duration::from_secs(30)).await, Some(Output { success: true, .. }))
}

/// Determine whether `-segment_times` are relative to the first packet. A 6 s
/// source is cut from 2 s with a single split at 2.5: relative semantics end
/// the first segment around 5 s, absolute semantics around 3 s.
async fn test_segment_semantics(ffmpeg: &Path) -> bool {
    let dir = std::env::temp_dir().join(format!("pley-selftest-{}", uuid::Uuid::new_v4().simple()));
    if std::fs::create_dir_all(&dir).is_err() {
        return true;
    }
    let pattern = dir.join("%d.ts");
    let pattern = pattern.to_string_lossy();
    let args = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-ss",
        "2",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=64x64:rate=10:duration=6",
        "-output_ts_offset",
        "2",
        "-c:v",
        "mpeg2video",
        "-g",
        "1000",
        "-force_key_frames",
        "expr:gte(t,n_forced*1)",
        "-f",
        "segment",
        "-segment_format",
        "mpegts",
        "-segment_times",
        "2.5",
        "-segment_list",
        "pipe:1",
        "-segment_list_type",
        "csv",
        &pattern,
    ];
    let result = run(ffmpeg, &args, Duration::from_secs(30)).await;
    let _ = std::fs::remove_dir_all(&dir);
    let Some(out) = result.filter(|o| o.success) else {
        tracing::warn!("segment muxer self-test failed; assuming relative segment times");
        return true;
    };
    let first_end = out
        .text
        .lines()
        .find(|l| l.starts_with("0.ts,"))
        .and_then(|l| l.split(',').nth(2))
        .and_then(|v| v.trim().parse::<f64>().ok());
    match first_end {
        Some(end) => end > 4.0,
        None => true,
    }
}

async fn detect_capabilities(ffmpeg: &Path, ffprobe: &Path) -> (Capabilities, bool) {
    let mut caps = Capabilities {
        ffmpeg_path: ffmpeg.display().to_string(),
        ffprobe_path: ffprobe.display().to_string(),
        ..Default::default()
    };
    let timeout = Duration::from_secs(30);
    let (version, probe, encoders, hwaccels, filters, muxers) = tokio::join!(
        run(ffmpeg, &["-hide_banner", "-version"], timeout),
        run(ffprobe, &["-hide_banner", "-version"], timeout),
        run(ffmpeg, &["-hide_banner", "-encoders"], timeout),
        run(ffmpeg, &["-hide_banner", "-hwaccels"], timeout),
        run(ffmpeg, &["-hide_banner", "-filters"], timeout),
        run(ffmpeg, &["-hide_banner", "-muxers"], timeout),
    );
    let Some(version) = version.filter(|o| o.success) else {
        return (caps, true);
    };
    caps.ffmpeg_available = true;
    caps.ffprobe_available = probe.is_some_and(|o| o.success);
    caps.ffmpeg_version = version
        .text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("ffmpeg version "))
        .and_then(|rest| rest.split_whitespace().next())
        .map(str::to_owned);

    let encoder_names = second_column(encoders.map(|o| o.text).unwrap_or_default());
    let filter_names = second_column(filters.map(|o| o.text).unwrap_or_default());
    caps.chromaprint = second_column(muxers.map(|o| o.text).unwrap_or_default()).contains("chromaprint");
    caps.hwaccels = hwaccels
        .map(|o| o.text)
        .unwrap_or_default()
        .lines()
        .skip_while(|l| !l.starts_with("Hardware acceleration methods"))
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();

    // Run the (slow) test encodes concurrently, a few at a time.
    let limiter = Arc::new(Semaphore::new(6));
    let candidates: Vec<_> =
        encoder_candidates().into_iter().filter(|(name, ..)| encoder_names.contains(*name)).collect();
    let encoder_tests = futures::future::join_all(candidates.iter().map(|(name, codec, accel)| {
        let limiter = limiter.clone();
        async move {
            let _permit = limiter.acquire().await.ok();
            let ok = test_encoder(ffmpeg, name, *accel).await;
            (
                EncoderInfo {
                    name: (*name).to_owned(),
                    codec: *codec,
                    hardware: *accel != HwAccel::None,
                    accel: *accel,
                },
                ok,
            )
        }
    }));

    let has = |f: &str| filter_names.contains(f);
    let libplacebo_test = async {
        if !has("libplacebo") {
            return false;
        }
        let vf = format!(
            "{HDR_TAGS},libplacebo=w=160:h=120:format=yuv420p:colorspace=bt709:color_primaries=bt709:color_trc=bt709:tonemapping=auto"
        );
        test_filter(ffmpeg, &["-init_hw_device", "vulkan=vk", "-filter_hw_device", "vk"], &vf).await
    };
    let opencl_test = async {
        if !has("tonemap_opencl") {
            return false;
        }
        let vf = format!(
            "{HDR_TAGS},format=p010,hwupload,tonemap_opencl=tonemap=hable:desat=0:t=bt709:m=bt709:p=bt709:format=nv12,hwdownload,format=nv12"
        );
        test_filter(ffmpeg, &["-init_hw_device", "opencl=ocl", "-filter_hw_device", "ocl"], &vf).await
    };
    let zscale_test = async {
        if !(has("zscale") && has("tonemap")) {
            return false;
        }
        let vf = format!("{HDR_TAGS},{}", crate::plan::ZSCALE_TONEMAP);
        test_filter(ffmpeg, &[], &vf).await
    };
    let readrate_test = async {
        let args = [
            "-hide_banner",
            "-nostdin",
            "-loglevel",
            "error",
            "-readrate",
            "50",
            "-readrate_initial_burst",
            "1",
            "-f",
            "lavfi",
            "-i",
            "color=black:s=64x64:r=25:d=0.1",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ];
        matches!(run(ffmpeg, &args, timeout).await, Some(Output { success: true, .. }))
    };

    let (encoder_results, libplacebo, opencl, zscale, readrate, relative) = tokio::join!(
        encoder_tests,
        libplacebo_test,
        opencl_test,
        zscale_test,
        readrate_test,
        test_segment_semantics(ffmpeg),
    );

    // Hardware encoders first (in candidate order), then software.
    let working: Vec<EncoderInfo> = encoder_results.into_iter().filter(|(_, ok)| *ok).map(|(e, _)| e).collect();
    caps.encoders =
        working.iter().filter(|e| e.hardware).cloned().chain(working.iter().filter(|e| !e.hardware).cloned()).collect();

    for (ok, mapper) in
        [(libplacebo, Tonemapper::Libplacebo), (opencl, Tonemapper::Opencl), (zscale, Tonemapper::Zscale)]
    {
        if ok {
            caps.tonemappers.push(mapper);
        }
    }
    caps.dolby_vision = libplacebo;
    caps.subtitle_burn_image = has("overlay");
    caps.subtitle_burn_text = has("subtitles");
    caps.readrate = readrate;

    let gpu = detect_gpu_pipelines(ffmpeg, &encoder_names, &filter_names, &caps, libplacebo).await;
    caps.vulkan_decode = gpu.vulkan_decode;
    caps.vulkan_encode = gpu.vulkan_encode;
    caps.gpu_pipelines = gpu.pipelines;
    (caps, relative)
}

#[derive(Default)]
struct GpuDetection {
    vulkan_decode: Vec<String>,
    vulkan_encode: bool,
    pipelines: Vec<GpuPipeline>,
}

/// Encoder arguments for the tiny clips used to probe hardware decoding,
/// keyed like [`pley_transcoder_api::decode_key`].
fn test_clip_specs(encoders: &HashSet<String>) -> Vec<(&'static str, Vec<&'static str>)> {
    let mut specs = Vec::new();
    if encoders.contains("libx264") {
        specs.push(("h264", vec!["-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p"]));
    }
    if encoders.contains("libx265") {
        let x265 = ["-c:v", "libx265", "-preset", "ultrafast", "-x265-params", "log-level=error"];
        specs.push(("hevc", [x265.as_slice(), &["-pix_fmt", "yuv420p"]].concat()));
        specs.push(("hevc10", [x265.as_slice(), &["-pix_fmt", "yuv420p10le"]].concat()));
    }
    if encoders.contains("libsvtav1") {
        specs.push(("av1", vec!["-c:v", "libsvtav1", "-preset", "12", "-pix_fmt", "yuv420p10le"]));
    } else if encoders.contains("libaom-av1") {
        specs.push(("av1", vec!["-c:v", "libaom-av1", "-cpu-used", "8", "-pix_fmt", "yuv420p"]));
    }
    if encoders.contains("libvpx-vp9") {
        specs.push((
            "vp9",
            vec!["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-pix_fmt", "yuv420p"],
        ));
    }
    specs.push(("mpeg2video", vec!["-c:v", "mpeg2video", "-pix_fmt", "yuv420p"]));
    specs
}

/// Self-test zero-copy GPU pipelines per input codec. A pipeline only counts
/// for a codec if decoding, filtering and encoding all stay on the GPU; when
/// the decoder cannot handle a codec, ffmpeg falls back to system-memory
/// frames and the GPU-only filter fails, so the test is conservative.
async fn detect_gpu_pipelines(
    ffmpeg: &Path,
    encoders: &HashSet<String>,
    filters: &HashSet<String>,
    caps: &Capabilities,
    libplacebo: bool,
) -> GpuDetection {
    let working = |name: &str| caps.encoders.iter().any(|e| e.name == name);
    let hwaccel = |name: &str| caps.hwaccels.iter().any(|h| h == name);
    let mut candidates: Vec<(HwAccel, Vec<&str>, &str, Vec<&str>)> = Vec::new();
    if working("h264_nvenc") && hwaccel("cuda") && filters.contains("scale_cuda") {
        candidates.push((
            HwAccel::Nvenc,
            vec!["-hwaccel", "cuda", "-hwaccel_output_format", "cuda", "-extra_hw_frames", "4"],
            "scale_cuda=160:120:format=nv12",
            vec!["-c:v", "h264_nvenc"],
        ));
    }
    if cfg!(target_os = "linux") && working("h264_vaapi") && hwaccel("vaapi") && filters.contains("scale_vaapi") {
        candidates.push((
            HwAccel::Vaapi,
            vec!["-hwaccel", "vaapi", "-hwaccel_device", VAAPI_DEVICE, "-hwaccel_output_format", "vaapi"],
            "scale_vaapi=w=160:h=120:format=nv12",
            vec!["-c:v", "h264_vaapi"],
        ));
    }
    if working("h264_qsv") && hwaccel("qsv") && filters.contains("vpp_qsv") {
        candidates.push((
            HwAccel::Qsv,
            vec!["-hwaccel", "qsv", "-hwaccel_output_format", "qsv"],
            "vpp_qsv=w=160:h=120:format=nv12",
            vec!["-c:v", "h264_qsv"],
        ));
    }
    if cfg!(target_os = "macos")
        && working("h264_videotoolbox")
        && hwaccel("videotoolbox")
        && filters.contains("scale_vt")
    {
        candidates.push((
            HwAccel::Videotoolbox,
            vec!["-hwaccel", "videotoolbox", "-hwaccel_output_format", "videotoolbox_vld"],
            "scale_vt=w=160:h=120",
            vec!["-c:v", "h264_videotoolbox"],
        ));
    }
    let vulkan = libplacebo && hwaccel("vulkan");
    if candidates.is_empty() && !vulkan {
        return GpuDetection::default();
    }

    let dir = std::env::temp_dir().join(format!("pley-gpu-selftest-{}", uuid::Uuid::new_v4().simple()));
    if std::fs::create_dir_all(&dir).is_err() {
        return GpuDetection::default();
    }
    let limiter = Arc::new(Semaphore::new(6));
    let clips: Vec<(&str, PathBuf)> =
        futures::future::join_all(test_clip_specs(encoders).into_iter().map(|(key, args)| {
            let dir = dir.clone();
            let limiter = limiter.clone();
            async move {
                let _permit = limiter.acquire().await.ok();
                let out = dir.join(format!("{key}.mkv"));
                let out_arg = out.to_string_lossy().into_owned();
                let mut cmd = vec![
                    "-hide_banner",
                    "-nostdin",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=320x240:rate=25:duration=0.4",
                ];
                cmd.extend(args);
                cmd.push(&out_arg);
                let ok = matches!(run(ffmpeg, &cmd, Duration::from_secs(60)).await, Some(Output { success: true, .. }));
                ok.then_some((key, out))
            }
        }))
        .await
        .into_iter()
        .flatten()
        .collect();

    let run_test = |args: Vec<String>| {
        let limiter = limiter.clone();
        async move {
            let _permit = limiter.acquire().await.ok();
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            matches!(run(ffmpeg, &refs, Duration::from_secs(30)).await, Some(Output { success: true, .. }))
        }
    };
    let base = |pre: &[&str], clip: &Path| -> Vec<String> {
        let mut v: Vec<String> =
            ["-hide_banner", "-nostdin", "-loglevel", "error"].iter().map(|s| s.to_string()).collect();
        v.extend(pre.iter().map(|s| s.to_string()));
        v.push("-i".into());
        v.push(clip.to_string_lossy().into_owned());
        v
    };

    // Vulkan decode → libplacebo (HDR/Dolby Vision tone mapping path).
    const VK: [&str; 10] = [
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
    ];
    let vulkan_tests = futures::future::join_all(clips.iter().filter(|_| vulkan).map(|(key, clip)| {
        let mut args = base(&VK, clip);
        args.extend(
            ["-vf", "libplacebo=w=160:h=120:format=nv12,hwdownload,format=nv12", "-f", "null", "-"].map(String::from),
        );
        let test = run_test(args);
        async move { (key.to_string(), test.await) }
    }));
    let vulkan_encode_test = async {
        let Some((_, clip)) = clips.iter().find(|(k, _)| *k == "hevc10").or(clips.first()) else { return false };
        if !(vulkan && encoders.contains("h264_vulkan")) {
            return false;
        }
        let mut args = base(&VK, clip);
        args.extend(
            [
                "-vf",
                "libplacebo=w=160:h=120:format=nv12",
                "-c:v",
                "h264_vulkan",
                "-rc_mode",
                "vbr",
                "-b:v",
                "1M",
                "-force_key_frames",
                "expr:gte(t,n_forced*0.2)",
                "-f",
                "null",
                "-",
            ]
            .map(String::from),
        );
        run_test(args).await
    };
    let pipeline_tests = futures::future::join_all(candidates.iter().map(|(accel, pre, vf, enc)| {
        let tests = futures::future::join_all(clips.iter().map(|(key, clip)| {
            let mut args = base(pre, clip);
            args.extend(["-vf".to_string(), vf.to_string()]);
            args.extend(enc.iter().map(|s| s.to_string()));
            args.extend(["-f", "null", "-"].map(String::from));
            let test = run_test(args);
            async move { (key.to_string(), test.await) }
        }));
        async move { (*accel, tests.await) }
    }));
    let (vulkan_results, vulkan_encode, pipeline_results) =
        tokio::join!(vulkan_tests, vulkan_encode_test, pipeline_tests);
    let _ = std::fs::remove_dir_all(&dir);

    let ok_keys =
        |results: Vec<(String, bool)>| results.into_iter().filter(|(_, ok)| *ok).map(|(k, _)| k).collect::<Vec<_>>();
    GpuDetection {
        vulkan_decode: ok_keys(vulkan_results),
        vulkan_encode,
        pipelines: pipeline_results
            .into_iter()
            .map(|(accel, results)| GpuPipeline { accel, codecs: ok_keys(results) })
            .filter(|p| !p.codecs.is_empty())
            .collect(),
    }
}

/// Names in the second whitespace-separated column of `ffmpeg -encoders` /
/// `-filters` / `-muxers` output.
fn second_column(text: String) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            let name = parts.next()?;
            (flags.len() <= 7 && flags.chars().all(|c| c == '.' || c.is_ascii_uppercase())).then(|| name.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_list() {
        let text = " V....D libx264              libx264 H.264\n V....D h264_nvenc           NVIDIA NVENC\n ------\n A....D aac  AAC\n".to_owned();
        let names = second_column(text);
        assert!(names.contains("libx264"));
        assert!(names.contains("h264_nvenc"));
        assert!(names.contains("aac"));
    }

    #[test]
    fn parses_muxer_list() {
        let text = "File formats:\n D. = Demuxing supported\n .E = Muxing supported\n ---\n  E chromaprint     Chromaprint\n  E mp4             MP4 (MPEG-4 Part 14)\n".to_owned();
        let names = second_column(text);
        assert!(names.contains("chromaprint"));
        assert!(names.contains("mp4"));
    }

    #[test]
    fn parses_filter_list() {
        let text = " ... libplacebo        N->N       Apply various GPU filters\n TSC zscale            V->V       Apply resizing\n".to_owned();
        let names = second_column(text);
        assert!(names.contains("libplacebo"));
        assert!(names.contains("zscale"));
    }

    fn fake(encoders: Vec<EncoderInfo>, tonemappers: Vec<Tonemapper>) -> Ffmpeg {
        Ffmpeg {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            caps: Capabilities { encoders, tonemappers, ..Default::default() },
            segment_times_relative: true,
        }
    }

    fn enc(name: &str, accel: HwAccel) -> EncoderInfo {
        EncoderInfo { name: name.into(), codec: VideoCodec::H264, hardware: accel != HwAccel::None, accel }
    }

    #[test]
    fn encoder_selection() {
        let ff = fake(vec![enc("h264_nvenc", HwAccel::Nvenc), enc("libx264", HwAccel::None)], vec![]);
        assert_eq!(ff.pick_encoder(VideoCodec::H264, HwAccel::Auto).0.unwrap().name, "h264_nvenc");
        assert_eq!(ff.pick_encoder(VideoCodec::H264, HwAccel::None).0.unwrap().name, "libx264");
        let (enc, warning) = ff.pick_encoder(VideoCodec::H264, HwAccel::Qsv);
        assert_eq!(enc.unwrap().name, "libx264");
        assert!(warning.is_some());
    }

    #[test]
    fn tonemapper_selection_prefers_libplacebo_for_dv() {
        let ff = fake(vec![], vec![Tonemapper::Opencl, Tonemapper::Libplacebo, Tonemapper::Zscale]);
        assert_eq!(ff.pick_tonemapper(TonemapMode::Auto, true).0, Some(Tonemapper::Libplacebo));
        assert_eq!(ff.pick_tonemapper(TonemapMode::Auto, false).0, Some(Tonemapper::Opencl));
        let ff = fake(vec![], vec![Tonemapper::Zscale]);
        let (choice, warnings) = ff.pick_tonemapper(TonemapMode::Auto, true);
        assert_eq!(choice, Some(Tonemapper::Zscale));
        assert!(warnings.iter().any(|w| w.contains("Dolby Vision")));
    }
}
