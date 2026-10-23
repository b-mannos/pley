// SPDX-License-Identifier: LicenseRef-Restricted
//! `ffprobe` wrapper producing a normalised [`MediaInfo`], including HDR10, HLG
//! and Dolby Vision detection. Results are cached in memory per file version.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use parking_lot::Mutex;
use pley_transcoder_api::{Chapter, DynamicRange, MediaInfo, MediaStream, StreamKind, is_text_subtitle_codec};
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::ffmpeg::Ffmpeg;
use crate::metrics::Metrics;
use crate::util::{MEDIA_INPUT_OPTIONS, file_key, input_arg};

const CACHE_ENTRIES: usize = 4096;

pub struct Prober {
    ff: Arc<Ffmpeg>,
    metrics: Arc<Metrics>,
    limiter: Semaphore,
    cache: Mutex<ProbeCache>,
}

#[derive(Default)]
struct ProbeCache {
    map: HashMap<String, Arc<MediaInfo>>,
    order: VecDeque<String>,
}

impl Prober {
    pub fn new(ff: Arc<Ffmpeg>, metrics: Arc<Metrics>, concurrency: usize) -> Self {
        Self { ff, metrics, limiter: Semaphore::new(concurrency.max(1)), cache: Mutex::default() }
    }

    pub async fn probe(&self, path: &Path) -> anyhow::Result<Arc<MediaInfo>> {
        let key = file_key(path).with_context(|| format!("cannot access {}", path.display()))?;
        if let Some(hit) = self.cache.lock().map.get(&key) {
            return Ok(hit.clone());
        }
        let _permit = self.limiter.acquire().await?;
        let started = Instant::now();
        Metrics::inc(&self.metrics.probes);
        let result = self.run_ffprobe(path).await;
        self.metrics
            .probe_ms_total
            .fetch_add(started.elapsed().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
        match result {
            Ok(info) => {
                let info = Arc::new(info);
                let mut cache = self.cache.lock();
                if cache.map.insert(key.clone(), info.clone()).is_none() {
                    cache.order.push_back(key);
                    while cache.order.len() > CACHE_ENTRIES {
                        if let Some(old) = cache.order.pop_front() {
                            cache.map.remove(&old);
                        }
                    }
                }
                Ok(info)
            }
            Err(err) => {
                Metrics::inc(&self.metrics.probe_failures);
                Err(err)
            }
        }
    }

    async fn run_ffprobe(&self, path: &Path) -> anyhow::Result<MediaInfo> {
        if !self.ff.caps.ffprobe_available {
            bail!("ffprobe is not available");
        }
        let mut cmd = self.ff.probe_command();
        cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters"])
            .args(MEDIA_INPUT_OPTIONS)
            .arg(input_arg(path))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = cmd.spawn().context("spawning ffprobe")?;
        let output = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
            .await
            .context("ffprobe timed out")??;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Rejected(format!("ffprobe failed: {}", stderr.trim())).into());
        }
        parse_ffprobe_json(&output.stdout, path).map_err(|e| Rejected(format!("{e:#}")).into())
    }
}

/// ffprobe ran and could not read the file, as opposed to a timeout or a
/// problem starting it. Only this kind of failure is worth remembering.
#[derive(Debug)]
pub struct Rejected(pub String);

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Rejected {}

#[derive(Deserialize)]
struct RawOutput {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
    #[serde(default)]
    chapters: Vec<RawChapter>,
}

#[derive(Deserialize)]
struct RawFormat {
    format_name: Option<String>,
    duration: Option<String>,
    start_time: Option<String>,
    bit_rate: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RawChapter {
    start_time: Option<String>,
    end_time: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RawStream {
    index: i64,
    codec_type: Option<String>,
    codec_name: Option<String>,
    profile: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    pix_fmt: Option<String>,
    level: Option<i64>,
    field_order: Option<String>,
    bits_per_raw_sample: Option<String>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    color_space: Option<String>,
    channels: Option<i64>,
    channel_layout: Option<String>,
    sample_rate: Option<String>,
    bit_rate: Option<String>,
    duration: Option<String>,
    #[serde(default)]
    disposition: HashMap<String, i64>,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    side_data_list: Vec<serde_json::Value>,
}

pub fn parse_ffprobe_json(json: &[u8], path: &Path) -> anyhow::Result<MediaInfo> {
    let raw: RawOutput = serde_json::from_slice(json).context("parsing ffprobe output")?;
    let format = raw.format.as_ref();
    let format_name = format.and_then(|f| f.format_name.clone());

    let mut has_cover_art = false;
    let mut streams = Vec::new();
    let mut stream_durations = Vec::new();
    for s in &raw.streams {
        let kind = match s.codec_type.as_deref() {
            Some("video") => StreamKind::Video,
            Some("audio") => StreamKind::Audio,
            Some("subtitle") => StreamKind::Subtitle,
            _ => continue,
        };
        let disposition = |k: &str| s.disposition.get(k).copied().unwrap_or(0) == 1;
        if kind == StreamKind::Video && (disposition("attached_pic") || is_image_codec(s.codec_name.as_deref())) {
            has_cover_art = true;
            continue;
        }
        let tags = lowercase_tags(&s.tags);
        let codec = s.codec_name.clone().unwrap_or_else(|| "unknown".into());
        if let Some(d) = s.duration.as_deref().and_then(parse_secs_ms) {
            stream_durations.push(d);
        }
        let mut stream = MediaStream {
            index: s.index,
            kind,
            language: tags.get("language").filter(|l| !l.is_empty() && l.as_str() != "und").cloned(),
            title: tags.get("title").filter(|t| !t.is_empty()).cloned(),
            default: disposition("default"),
            forced: disposition("forced"),
            bitrate: s
                .bit_rate
                .as_deref()
                .and_then(|b| b.parse().ok())
                .or_else(|| tags.get("bps").and_then(|b| b.parse().ok())),
            profile: s.profile.clone().filter(|p| !p.is_empty() && p != "unknown"),
            codec: codec.clone(),
            ..Default::default()
        };
        match kind {
            StreamKind::Video => {
                stream.width = s.width;
                stream.height = s.height;
                stream.frame_rate =
                    parse_rate(s.avg_frame_rate.as_deref()).or_else(|| parse_rate(s.r_frame_rate.as_deref()));
                stream.pix_fmt = s.pix_fmt.clone();
                stream.bit_depth = bit_depth(s.pix_fmt.as_deref(), s.bits_per_raw_sample.as_deref());
                stream.level = s.level.filter(|l| *l > 0);
                stream.interlaced = s.field_order.as_deref().map(|f| !matches!(f, "progressive" | "unknown"));
                stream.color_transfer = s.color_transfer.clone();
                stream.color_primaries = s.color_primaries.clone();
                stream.color_space = s.color_space.clone();
                let dovi = s.side_data_list.iter().find(|sd| {
                    sd.get("side_data_type").and_then(|t| t.as_str()).is_some_and(|t| t.contains("DOVI configuration"))
                });
                let range = if let Some(dovi) = dovi {
                    let get = |k: &str| dovi.get(k).and_then(|v| v.as_i64());
                    stream.dv_profile = get("dv_profile");
                    stream.dv_level = get("dv_level");
                    stream.dv_bl_compat = get("dv_bl_signal_compatibility_id");
                    DynamicRange::DolbyVision
                } else {
                    match s.color_transfer.as_deref() {
                        Some("smpte2084") => {
                            let hdr10plus = s.side_data_list.iter().any(|sd| {
                                sd.get("side_data_type").and_then(|t| t.as_str()).is_some_and(|t| t.contains("2094-40"))
                            });
                            if hdr10plus { DynamicRange::Hdr10plus } else { DynamicRange::Hdr10 }
                        }
                        Some("arib-std-b67") => DynamicRange::Hlg,
                        // Untagged transfer but 10+ bit BT.2020: in practice this is PQ (HDR10).
                        None | Some("unknown") | Some("unspecified")
                            if stream.bit_depth.unwrap_or(8) >= 10
                                && (s.color_primaries.as_deref() == Some("bt2020")
                                    || s.color_space.as_deref().is_some_and(|c| c.starts_with("bt2020"))) =>
                        {
                            DynamicRange::Hdr10
                        }
                        _ => DynamicRange::Sdr,
                    }
                };
                stream.dynamic_range = Some(range);
                stream.hdr = Some(range.is_hdr());
            }
            StreamKind::Audio => {
                stream.channels = s.channels;
                stream.channel_layout = s.channel_layout.clone();
                stream.sample_rate = s.sample_rate.as_deref().and_then(|r| r.parse().ok());
            }
            StreamKind::Subtitle => {
                stream.text_based = Some(is_text_subtitle_codec(&codec));
            }
        }
        streams.push(stream);
    }

    let duration_ms = format
        .and_then(|f| f.duration.as_deref())
        .and_then(parse_secs_ms)
        .or_else(|| stream_durations.into_iter().max());
    let bitrate = format.and_then(|f| f.bit_rate.as_deref()).and_then(|b| b.parse().ok());
    let tags: BTreeMap<String, String> =
        format.map(|f| lowercase_tags(&f.tags)).unwrap_or_default().into_iter().collect();
    let chapters = raw
        .chapters
        .iter()
        .filter_map(|c| {
            Some(Chapter {
                start_ms: parse_secs_ms(c.start_time.as_deref()?)?,
                end_ms: parse_secs_ms(c.end_time.as_deref()?)?,
                title: c.tags.iter().find(|(k, _)| k.eq_ignore_ascii_case("title")).map(|(_, v)| v.clone()),
            })
        })
        .collect();

    let start_ms = format
        .and_then(|f| f.start_time.as_deref())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map(|v| (v * 1000.0).round() as i64);

    Ok(MediaInfo {
        container: normalize_container(format_name.as_deref(), path, &streams),
        format_name,
        duration_ms,
        start_ms,
        bitrate,
        streams,
        tags,
        chapters,
        has_cover_art,
    })
}

fn lowercase_tags(tags: &HashMap<String, String>) -> HashMap<String, String> {
    tags.iter().map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned())).collect()
}

fn is_image_codec(codec: Option<&str>) -> bool {
    matches!(codec, Some("mjpeg" | "png" | "bmp" | "gif" | "webp"))
}

fn parse_secs_ms(value: &str) -> Option<i64> {
    let secs: f64 = value.trim().parse().ok()?;
    (secs.is_finite() && secs >= 0.0).then(|| (secs * 1000.0).round() as i64)
}

fn parse_rate(value: Option<&str>) -> Option<f64> {
    let value = value?;
    let (num, den) = value.split_once('/').unwrap_or((value, "1"));
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den <= 0.0 || num <= 0.0 {
        return None;
    }
    let rate = num / den;
    (rate > 0.0 && rate < 1000.0).then(|| (rate * 1000.0).round() / 1000.0)
}

/// Bits per component from the pixel format (`yuv420p10le` → 10, `p010le` → 10,
/// `nv12` → 8), falling back to `bits_per_raw_sample`.
fn bit_depth(pix_fmt: Option<&str>, raw: Option<&str>) -> Option<i64> {
    if let Some(fmt) = pix_fmt {
        let f = fmt.trim_end_matches("le").trim_end_matches("be");
        if f.starts_with("nv") {
            return Some(8);
        }
        if let Some(depth) = f.strip_prefix("p0").and_then(|d| d.parse().ok()) {
            return Some(depth);
        }
        let digits: String = f.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let digits: String = digits.chars().rev().collect();
        return Some(digits.parse::<i64>().ok().filter(|d| (9..=16).contains(d)).unwrap_or(8));
    }
    raw.and_then(|r| r.parse().ok())
}

/// Map FFmpeg's demuxer names to the short container names clients understand.
fn normalize_container(format_name: Option<&str>, path: &Path, streams: &[MediaStream]) -> Option<String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let format_name = format_name?;
    let first = format_name.split(',').next().unwrap_or(format_name);
    let container = match first {
        "matroska" => {
            let webm_codecs = streams
                .iter()
                .all(|s| matches!(s.codec.as_str(), "vp8" | "vp9" | "av1" | "opus" | "vorbis" | "webvtt"));
            if ext == "webm" && webm_codecs { "webm" } else { "mkv" }
        }
        "mov" => match ext.as_str() {
            "m4v" | "m4a" | "mov" | "3gp" | "m4b" => leak_ext(&ext),
            _ => "mp4",
        },
        "mpegts" => "ts",
        "mpeg" => "mpeg",
        "asf" => {
            if streams.iter().any(|s| s.kind == StreamKind::Video) {
                "wmv"
            } else {
                "wma"
            }
        }
        "ogg" => "ogg",
        "flac" => "flac",
        "mp3" => "mp3",
        "wav" => "wav",
        "aiff" => "aiff",
        "ape" => "ape",
        "wv" => "wv",
        "avi" => "avi",
        "flv" => "flv",
        "aac" => "aac",
        other => return Some(other.to_owned()),
    };
    Some(container.to_owned())
}

fn leak_ext(ext: &str) -> &'static str {
    match ext {
        "m4v" => "m4v",
        "m4a" => "m4a",
        "mov" => "mov",
        "3gp" => "3gp",
        "m4b" => "m4b",
        _ => "mp4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "streams": [
        {"index":0,"codec_name":"hevc","codec_type":"video","profile":"Main 10","width":3840,"height":2160,
         "pix_fmt":"yuv420p10le","level":153,"color_transfer":"smpte2084","color_primaries":"bt2020","color_space":"bt2020nc",
         "field_order":"progressive","r_frame_rate":"24000/1001","avg_frame_rate":"24000/1001",
         "disposition":{"default":1,"forced":0},
         "side_data_list":[{"side_data_type":"DOVI configuration record","dv_version_major":1,"dv_version_minor":0,
           "dv_profile":8,"dv_level":6,"rpu_present_flag":1,"el_present_flag":0,"bl_present_flag":1,"dv_bl_signal_compatibility_id":1}],
         "tags":{"language":"und"}},
        {"index":1,"codec_name":"eac3","codec_type":"audio","channels":6,"channel_layout":"5.1(side)","sample_rate":"48000",
         "disposition":{"default":1},"tags":{"language":"eng","title":"Surround 5.1","BPS":"640000"}},
        {"index":2,"codec_name":"subrip","codec_type":"subtitle","disposition":{"default":0,"forced":1},"tags":{"language":"eng"}},
        {"index":3,"codec_name":"hdmv_pgs_subtitle","codec_type":"subtitle","disposition":{"default":0},"tags":{"language":"fre"}},
        {"index":4,"codec_name":"mjpeg","codec_type":"video","disposition":{"attached_pic":1}}
      ],
      "format": {"format_name":"matroska,webm","start_time":"1.400000","duration":"7265.123000","bit_rate":"25000000","tags":{"TITLE":"Movie"}},
      "chapters": [{"start_time":"0.000000","end_time":"300.500000","tags":{"title":"Opening"}}]
    }"#;

    #[test]
    fn parses_streams_and_dolby_vision() {
        let info = parse_ffprobe_json(SAMPLE.as_bytes(), Path::new("/m/Movie.mkv")).unwrap();
        assert_eq!(info.container.as_deref(), Some("mkv"));
        assert_eq!(info.duration_ms, Some(7_265_123));
        assert_eq!(info.start_ms, Some(1_400));
        assert_eq!(info.bitrate, Some(25_000_000));
        assert!(info.has_cover_art);
        assert_eq!(info.streams.len(), 4);
        assert_eq!(info.tags.get("title").map(String::as_str), Some("Movie"));
        assert_eq!(info.chapters.len(), 1);

        let video = info.video().unwrap();
        assert_eq!(video.dynamic_range, Some(DynamicRange::DolbyVision));
        assert_eq!(video.dv_profile, Some(8));
        assert_eq!(video.dv_bl_compat, Some(1));
        assert_eq!(video.bit_depth, Some(10));
        assert_eq!(video.hdr, Some(true));
        assert_eq!(video.interlaced, Some(false));
        assert!((video.frame_rate.unwrap() - 23.976).abs() < 0.001);
        assert_eq!(video.language, None);

        let audio = info.default_audio().unwrap();
        assert_eq!(audio.channels, Some(6));
        assert_eq!(audio.bitrate, Some(640_000));
        assert_eq!(audio.title.as_deref(), Some("Surround 5.1"));

        let srt = info.stream(2).unwrap();
        assert!(srt.forced);
        assert_eq!(srt.text_based, Some(true));
        let pgs = info.stream(3).unwrap();
        assert_eq!(pgs.text_based, Some(false));
    }

    #[test]
    fn container_names() {
        let s = |codec: &str, kind| MediaStream { codec: codec.into(), kind, ..Default::default() };
        let vp9 = vec![s("vp9", StreamKind::Video), s("opus", StreamKind::Audio)];
        assert_eq!(normalize_container(Some("matroska,webm"), Path::new("a.webm"), &vp9).as_deref(), Some("webm"));
        assert_eq!(normalize_container(Some("matroska,webm"), Path::new("a.mkv"), &vp9).as_deref(), Some("mkv"));
        assert_eq!(
            normalize_container(Some("mov,mp4,m4a,3gp,3g2,mj2"), Path::new("a.m4v"), &[]).as_deref(),
            Some("m4v")
        );
        assert_eq!(
            normalize_container(Some("mov,mp4,m4a,3gp,3g2,mj2"), Path::new("a.MP4"), &[]).as_deref(),
            Some("mp4")
        );
        assert_eq!(normalize_container(Some("mpegts"), Path::new("a.m2ts"), &[]).as_deref(), Some("ts"));
    }

    #[test]
    fn bit_depths() {
        assert_eq!(bit_depth(Some("yuv420p"), None), Some(8));
        assert_eq!(bit_depth(Some("yuv420p10le"), None), Some(10));
        assert_eq!(bit_depth(Some("yuv444p12be"), None), Some(12));
        assert_eq!(bit_depth(Some("p010le"), None), Some(10));
        assert_eq!(bit_depth(Some("nv12"), None), Some(8));
        assert_eq!(bit_depth(Some("rgb24"), None), Some(8));
        assert_eq!(bit_depth(None, Some("10")), Some(10));
    }

    #[test]
    fn hdr_classification() {
        let json = r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le","color_transfer":"arib-std-b67"}],"format":{"format_name":"mov,mp4","duration":"10"}}"#;
        let info = parse_ffprobe_json(json.as_bytes(), Path::new("a.mp4")).unwrap();
        assert_eq!(info.video().unwrap().dynamic_range, Some(DynamicRange::Hlg));
    }

    fn video_range(stream: &str) -> Option<DynamicRange> {
        let json = format!(r#"{{"streams":[{stream}],"format":{{"format_name":"matroska","duration":"10"}}}}"#);
        parse_ffprobe_json(json.as_bytes(), Path::new("a.mkv")).unwrap().video().unwrap().dynamic_range
    }

    #[test]
    fn more_dynamic_ranges() {
        let hdr10plus = r#"{"index":0,"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le","color_transfer":"smpte2084",
            "side_data_list":[{"side_data_type":"HDR Dynamic Metadata SMPTE2094-40 (HDR10+)"}]}"#;
        assert_eq!(video_range(hdr10plus), Some(DynamicRange::Hdr10plus));
        // Untagged transfer, 10-bit BT.2020: PQ in practice.
        let untagged = r#"{"index":0,"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le","color_primaries":"bt2020"}"#;
        assert_eq!(video_range(untagged), Some(DynamicRange::Hdr10));
        let eight_bit =
            r#"{"index":0,"codec_type":"video","codec_name":"h264","pix_fmt":"yuv420p","color_primaries":"bt2020"}"#;
        assert_eq!(video_range(eight_bit), Some(DynamicRange::Sdr));
        let bt709 =
            r#"{"index":0,"codec_type":"video","codec_name":"h264","pix_fmt":"yuv420p","color_transfer":"bt709"}"#;
        assert_eq!(video_range(bt709), Some(DynamicRange::Sdr));
    }

    #[test]
    fn odd_ffprobe_output() {
        let json = r#"{
          "streams": [
            {"index":0,"codec_type":"video","codec_name":"mpeg2video","field_order":"tt","avg_frame_rate":"0/0","r_frame_rate":"30000/1001","tags":{"LANGUAGE":"und"}},
            {"index":1,"codec_type":"audio","codec_name":"mp2","duration":"95.5","tags":{"language":""}},
            {"index":2,"codec_type":"data","codec_name":"bin_data"},
            {"index":3,"codec_type":"attachment","codec_name":"ttf"},
            {"index":4,"codec_type":"audio","codec_name":"ac3","duration":"120.25","profile":"unknown"}
          ],
          "format": {"format_name":"mpeg","start_time":"N/A"},
          "chapters": [{"start_time":"0.0","end_time":"60.0","tags":{"TITLE":"Part 1"}}, {"start_time":"60.0"}]
        }"#;
        let info = parse_ffprobe_json(json.as_bytes(), Path::new("old.vob")).unwrap();
        assert_eq!(info.container.as_deref(), Some("mpeg"));
        assert_eq!(
            info.streams.iter().map(|s| s.index).collect::<Vec<_>>(),
            [0, 1, 4],
            "data and attachments are not streams"
        );
        let video = info.video().unwrap();
        assert_eq!(video.interlaced, Some(true));
        assert_eq!(video.frame_rate, Some(29.97), "the real frame rate when the average is unknown");
        assert_eq!(video.language, None, "und is no language");
        assert_eq!(info.stream(1).unwrap().language, None);
        assert_eq!(info.stream(4).unwrap().profile, None);
        assert_eq!(info.duration_ms, Some(120_250), "the longest stream without a format duration");
        assert_eq!(info.start_ms, None);
        assert_eq!(info.chapters.len(), 1, "a chapter without an end is dropped");
        assert_eq!(info.chapters[0].title.as_deref(), Some("Part 1"));
        assert!(parse_ffprobe_json(b"not json", Path::new("a.mkv")).is_err());
    }

    #[test]
    fn frame_rates_and_windows_media() {
        assert_eq!(parse_rate(Some("25")), Some(25.0));
        assert_eq!(parse_rate(Some("24000/1001")), Some(23.976));
        assert_eq!(parse_rate(Some("0/0")), None);
        assert_eq!(parse_rate(Some("90000/1")), None, "a time base, not a frame rate");
        assert_eq!(parse_rate(Some("N/A")), None);
        assert_eq!(parse_secs_ms("-1.5"), None);
        assert_eq!(parse_secs_ms(" 1.0006 "), Some(1_001));
        let s = |kind| MediaStream { kind, ..Default::default() };
        assert_eq!(
            normalize_container(Some("asf"), Path::new("a.wmv"), &[s(StreamKind::Video)]).as_deref(),
            Some("wmv")
        );
        assert_eq!(
            normalize_container(Some("asf"), Path::new("a.wma"), &[s(StreamKind::Audio)]).as_deref(),
            Some("wma")
        );
        assert_eq!(normalize_container(Some("matroska,webm"), Path::new("a.webm"), &[]).as_deref(), Some("webm"));
        assert_eq!(
            normalize_container(Some("dv"), Path::new("a.dv"), &[]).as_deref(),
            Some("dv"),
            "unknown names pass through"
        );
        assert_eq!(normalize_container(None, Path::new("a.mkv"), &[]), None);
    }
}
