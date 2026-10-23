//! Playback through a transcoder: pley in external-transcoder mode talking
//! to a local stand-in that speaks the transcoder API (docs/TRANSCODER.md)
//! without running FFmpeg. It probes files by name, hands out HLS jobs with
//! fake segments, and can lose its jobs or change their layout, as a
//! restarted transcoder would.

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use pley_transcoder_api::{
    API_VERSION, AudioMode, AudioPlan, Capabilities, EncoderInfo, ErrorBody, HwAccel, JobCreated, JobRequest,
    JobStatus, Limits, MediaInfo, MediaStream, ProbeRequest, StreamKind, SubtitleRequest, TranscoderInfo,
    TranscoderStatus, TrickplayInfo, TrickplayRequest, VideoCodec, VideoMode, VideoPlan,
};
use serde_json::{Value, json};

use common::{TestApp, encode};

const TOKEN: &str = "shared-secret";

// ------------------------------------------------------------------ the stand-in

#[derive(Clone, Default)]
struct Fake {
    /// Live jobs by id.
    jobs: Arc<Mutex<HashMap<String, (JobRequest, u32)>>>,
    /// Every job request, in order.
    created: Arc<Mutex<Vec<JobRequest>>>,
    deleted: Arc<Mutex<Vec<String>>>,
    probes: Arc<Mutex<Vec<String>>>,
    subtitles: Arc<Mutex<Vec<SubtitleRequest>>>,
    thumbnails: Arc<AtomicUsize>,
    /// Paths seek previews were asked for.
    previews: Arc<Mutex<Vec<String>>>,
    next_id: Arc<AtomicUsize>,
    /// Segment count of new jobs instead of one per 4 s.
    layout: Arc<Mutex<Option<u32>>>,
    /// How long creating a job takes.
    job_delay: Arc<Mutex<Duration>>,
    /// Creating a job fails, as when ffmpeg can't read the file.
    fail_jobs: Arc<AtomicBool>,
}

impl Fake {
    /// The transcoder restarted: every job is gone.
    fn lose_jobs(&self) {
        self.jobs.lock().clear();
    }

    fn created(&self) -> usize {
        self.created.lock().len()
    }

    fn last_request(&self) -> JobRequest {
        self.created.lock().last().cloned().expect("a job request")
    }
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(ErrorBody { code: code.into(), message: format!("{code} (stand-in)") })).into_response()
}

fn video(codec: &str) -> MediaStream {
    MediaStream {
        index: 0,
        kind: StreamKind::Video,
        codec: codec.into(),
        width: Some(1920),
        height: Some(1080),
        bit_depth: Some(8),
        ..Default::default()
    }
}

fn audio(index: i64, codec: &str, language: &str, default: bool) -> MediaStream {
    MediaStream {
        index,
        kind: StreamKind::Audio,
        codec: codec.into(),
        language: Some(language.into()),
        channels: Some(if codec == "aac" { 2 } else { 6 }),
        default,
        ..Default::default()
    }
}

fn subtitle(index: i64, codec: &str, language: &str, text: bool) -> MediaStream {
    MediaStream {
        index,
        kind: StreamKind::Subtitle,
        codec: codec.into(),
        language: Some(language.into()),
        text_based: Some(text),
        ..Default::default()
    }
}

/// What ffprobe would say, decided by the file name: MP4s are H.264/AAC,
/// MKVs H.264 with AC-3, a second audio track and two subtitle tracks.
fn probe_result(path: &str) -> Option<MediaInfo> {
    if path.contains("Broken") {
        return None;
    }
    let mkv = path.ends_with(".mkv");
    let streams = if mkv {
        vec![
            video("h264"),
            audio(1, "ac3", "eng", true),
            audio(2, "aac", "fre", false),
            subtitle(3, "hdmv_pgs_subtitle", "eng", false),
            subtitle(4, "subrip", "fre", true),
        ]
    } else {
        vec![video("h264"), audio(1, "aac", "eng", true)]
    };
    Some(MediaInfo {
        container: Some(if mkv { "mkv" } else { "mp4" }.into()),
        duration_ms: Some(60_000),
        bitrate: Some(4_000_000),
        start_ms: Some(0),
        streams,
        ..Default::default()
    })
}

fn job_status(id: &str, req: &JobRequest, segments: u32) -> JobStatus {
    JobStatus {
        id: id.into(),
        label: req.label.clone(),
        owner: req.owner.clone(),
        input: req.input.clone(),
        duration_ms: 60_000,
        segment_seconds: 4.0,
        segment_count: segments,
        video: req.video.as_ref().map(|v| VideoPlan {
            mode: v.mode,
            codec_in: "h264".into(),
            codec_out: "h264".into(),
            width_in: Some(1920),
            height_in: Some(1080),
            width_out: Some(1920),
            height_out: Some(v.max_height.map(i64::from).unwrap_or(1080)),
            ..Default::default()
        }),
        audio: req.audio.as_ref().map(|a| AudioPlan {
            mode: a.mode,
            codec_in: "ac3".into(),
            codec_out: "aac".into(),
            channels_in: Some(6),
            channels_out: Some(2),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn authorized(headers: &HeaderMap) -> bool {
    headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {TOKEN}"))
}

async fn start_fake() -> (Fake, String) {
    let fake = Fake::default();
    let app = Router::new()
        .route(
            "/v1/info",
            get(|headers: HeaderMap| async move {
                if !authorized(&headers) {
                    return error(StatusCode::UNAUTHORIZED, "unauthorized");
                }
                Json(TranscoderInfo {
                    name: "pley-transcoder".into(),
                    version: "9.9.9".into(),
                    api_version: API_VERSION,
                    license: "LicenseRef-Restricted".into(),
                    capabilities: Capabilities {
                        ffmpeg_available: true,
                        ffprobe_available: true,
                        ffmpeg_version: Some("stand-in".into()),
                        encoders: vec![EncoderInfo {
                            name: "libx264".into(),
                            codec: VideoCodec::H264,
                            hardware: false,
                            accel: HwAccel::None,
                        }],
                        ..Default::default()
                    },
                    limits: Limits { max_jobs: 8, max_hw_jobs: 0 },
                    ..Default::default()
                })
                .into_response()
            }),
        )
        .route(
            "/v1/status",
            get(|State(fake): State<Fake>| async move {
                let jobs = fake.jobs.lock().iter().map(|(id, (req, n))| job_status(id, req, *n)).collect();
                Json(TranscoderStatus { version: "9.9.9".into(), max_jobs: 8, jobs, ..Default::default() })
            }),
        )
        .route(
            "/v1/probe",
            post(|State(fake): State<Fake>, Json(req): Json<ProbeRequest>| async move {
                fake.probes.lock().push(req.path.clone());
                match probe_result(&req.path) {
                    Some(info) => Json(info).into_response(),
                    None => error(StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg_failed"),
                }
            }),
        )
        .route(
            "/v1/jobs",
            post(|State(fake): State<Fake>, Json(req): Json<JobRequest>| async move {
                let delay = *fake.job_delay.lock();
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                if fake.fail_jobs.load(Ordering::SeqCst) {
                    return error(StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg_failed");
                }
                let n = fake.next_id.fetch_add(1, Ordering::SeqCst) + 1;
                let id = format!("job{n}");
                let segments = fake.layout.lock().unwrap_or(15);
                let status = job_status(&id, &req, segments);
                fake.created.lock().push(req.clone());
                fake.jobs.lock().insert(id.clone(), (req, segments));
                Json(JobCreated {
                    id,
                    duration_ms: 60_000,
                    segment_seconds: 4.0,
                    segment_count: segments,
                    video: status.video,
                    audio: status.audio,
                    warnings: vec![],
                })
                .into_response()
            }),
        )
        .route(
            "/v1/jobs/{id}",
            get(|State(fake): State<Fake>, Path(id): Path<String>| async move {
                match fake.jobs.lock().get(&id) {
                    Some((req, n)) => Json(job_status(&id, req, *n)).into_response(),
                    None => error(StatusCode::NOT_FOUND, "job_not_found"),
                }
            })
            .delete(|State(fake): State<Fake>, Path(id): Path<String>| async move {
                fake.deleted.lock().push(id.clone());
                match fake.jobs.lock().remove(&id) {
                    Some(_) => StatusCode::NO_CONTENT.into_response(),
                    None => error(StatusCode::NOT_FOUND, "job_not_found"),
                }
            }),
        )
        .route(
            "/v1/jobs/{id}/touch",
            post(|State(fake): State<Fake>, Path(id): Path<String>| async move {
                if fake.jobs.lock().contains_key(&id) {
                    StatusCode::NO_CONTENT.into_response()
                } else {
                    error(StatusCode::NOT_FOUND, "job_not_found")
                }
            }),
        )
        .route(
            "/v1/jobs/{id}/{file}",
            get(|State(fake): State<Fake>, Path((id, file)): Path<(String, String)>| async move {
                let Some(segments) = fake.jobs.lock().get(&id).map(|(_, n)| *n) else {
                    return error(StatusCode::NOT_FOUND, "job_not_found");
                };
                if file == "index.m3u8" {
                    let mut playlist = String::from("#EXTM3U\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-TARGETDURATION:4\n");
                    for n in 0..segments {
                        playlist.push_str(&format!("#EXTINF:4.000,\n{n}.ts\n"));
                    }
                    playlist.push_str("#EXT-X-ENDLIST\n");
                    return ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], playlist).into_response();
                }
                match file.strip_suffix(".ts").and_then(|n| n.parse::<u32>().ok()) {
                    Some(n) if n < segments => {
                        ([(header::CONTENT_TYPE, "video/mp2t")], format!("{id}:{n}")).into_response()
                    }
                    _ => error(StatusCode::NOT_FOUND, "not_found"),
                }
            }),
        )
        .route(
            "/v1/subtitles",
            // Stream 99 can't be converted; stream 5 takes a while (a big file).
            post(|State(fake): State<Fake>, Json(req): Json<SubtitleRequest>| async move {
                let text = format!("WEBVTT\n\n00:00.000 --> 00:01.000\n{}#{:?}\n", req.path, req.stream_index);
                let fails = req.stream_index == Some(99);
                let slow = matches!(req.stream_index, Some(5 | 6));
                fake.subtitles.lock().push(req);
                if slow {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                if fails {
                    return error(StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg_failed");
                }
                ([(header::CONTENT_TYPE, "text/vtt")], text).into_response()
            }),
        )
        .route(
            "/v1/thumbnail",
            post(|State(fake): State<Fake>| async move {
                fake.thumbnails.fetch_add(1, Ordering::SeqCst);
                let mut jpeg = Vec::new();
                image::RgbImage::from_pixel(64, 36, image::Rgb([200, 100, 50]))
                    .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
                    .unwrap();
                ([(header::CONTENT_TYPE, "image/jpeg")], jpeg)
            }),
        )
        .route("/v1/keyframes", post(|| async { StatusCode::ACCEPTED }))
        // Previews of the MP4 work: one sheet of 5×2 tiles of 8×4. Others fail.
        .route(
            "/v1/trickplay",
            post(|State(fake): State<Fake>, Json(req): Json<TrickplayRequest>| async move {
                fake.previews.lock().push(req.path.clone());
                if !req.path.ends_with(".mp4") {
                    return error(StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg_failed");
                }
                Json(TrickplayInfo {
                    id: "previews1".into(),
                    interval_ms: 10_000,
                    width: 8,
                    height: 4,
                    columns: 5,
                    rows: 2,
                    count: 6,
                    sheets: 1,
                })
                .into_response()
            }),
        )
        .route(
            "/v1/trickplay/{id}/{sheet}",
            get(|Path((id, sheet)): Path<(String, String)>| async move {
                if id != "previews1" || sheet != "0.jpg" {
                    return error(StatusCode::NOT_FOUND, "not_found");
                }
                let mut jpeg = Vec::new();
                image::RgbImage::from_fn(40, 8, |x, y| image::Rgb([(x * 6) as u8, (y * 30) as u8, 90]))
                    .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
                    .unwrap();
                ([(header::CONTENT_TYPE, "image/jpeg")], jpeg).into_response()
            }),
        )
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (fake, format!("http://{addr}"))
}

// ------------------------------------------------------------------ pley

struct World {
    app: TestApp,
    fake: Fake,
    base: String,
    admin: String,
    /// Title → id of the movies.
    movies: HashMap<String, i64>,
}

/// pley with the stand-in as its transcoder and a scanned movie library:
/// "Feature" (MKV, with an English sidecar subtitle), "Clip" (MP4) and
/// "Broken" (ffprobe can't read it).
async fn world() -> World {
    let (fake, base) = start_fake().await;
    let url = base.clone();
    let app = TestApp::with_config(|c| {
        c.transcoder_url = Some(url);
        c.transcoder_token = Some(TOKEN.into());
    })
    .await;
    app.state.transcoder.start();
    assert!(app.state.transcoder.wait_ready(Duration::from_secs(10)).await, "the stand-in answers /v1/info");
    let admin = app.setup_admin().await;
    // No seek previews in the background: only what the tests ask for.
    let settings = app.put("/api/settings", &admin, json!({"seek_previews": false})).await;
    assert_eq!(settings.status, StatusCode::OK);
    // Different sizes: identical files would share their media folder.
    app.media_file("movies/Feature (2021)/Feature (2021).mkv", 4096);
    app.media_file("movies/Feature (2021)/Feature (2021).en.srt", 64);
    app.media_file("movies/Clip (2020).mp4", 3072);
    app.media_file("movies/Broken (2019).mkv", 2048);
    let library = app.library(&admin, "Movies", "movies", "movies").await;
    let movies = app.ids(library, &admin).await;
    World { app, fake, base, admin, movies }
}

fn browser() -> Value {
    json!({"containers": ["mp4", "webm"], "video_codecs": ["h264"], "audio_codecs": ["aac", "mp3"]})
}

async fn play(app: &TestApp, token: &str, body: Value) -> common::Reply {
    app.post("/api/playback", token, body).await
}

/// Waits for background work of the server (job deletions) to reach the stand-in.
async fn eventually(check: impl Fn() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn scans_keep_what_the_transcoder_probed() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let feature = app.get(&format!("/api/items/{}", w.movies["Feature"]), admin).await;
    let file = &feature.body["files"][0];
    assert_eq!(
        (&file["container"], &file["video_codec"], &file["audio_codec"], &file["duration_ms"], &file["height"]),
        (&json!("mkv"), &json!("h264"), &json!("ac3"), &json!(60_000), &json!(1080))
    );
    let streams = file["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 6, "five embedded streams and the sidecar");
    assert_eq!(streams[5]["external"], json!(true));
    assert_eq!(feature.body["duration_ms"], json!(60_000), "the item takes the file's duration");
    // ffprobe could not read this one: listed, without media details.
    let broken = app.get(&format!("/api/items/{}", w.movies["Broken"]), admin).await;
    assert_eq!(broken.body["files"][0]["video_codec"], Value::Null);

    // Unchanged files are not probed again, unreadable ones neither; a forced scan probes everything.
    let probes = || w.fake.probes.lock().len();
    assert_eq!(probes(), 3);
    let library = feature.body["library_id"].as_i64().unwrap();
    app.post(&format!("/api/libraries/{library}/scan"), admin, json!({})).await;
    app.wait_for_scans().await;
    assert_eq!(probes(), 3);
    app.post(&format!("/api/libraries/{library}/scan"), admin, json!({"force": true})).await;
    app.wait_for_scans().await;
    assert_eq!(probes(), 6);

    let system = app.get("/api/system", admin).await;
    let transcoder = &system.body["transcoder"];
    assert_eq!((&transcoder["available"], &transcoder["managed"]), (&json!(true), &json!(false)));
    assert_eq!((&transcoder["version"], &transcoder["url"]), (&json!("9.9.9"), &json!(w.base)));
    assert_eq!(transcoder["active_encoder"], json!("libx264"));
}

#[tokio::test]
async fn remux_sessions_stream_hls_from_the_transcoder() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let feature = w.movies["Feature"];
    let started = play(app, admin, json!({"item_id": feature, "caps": browser(), "client_id": "tab"})).await;
    assert_eq!(started.status, StatusCode::OK, "{:?}", started.body);
    let info = &started.body;
    assert_eq!((&info["method"], &info["hls"], &info["duration_ms"]), (&json!("remux"), &json!(true), &json!(60_000)));
    assert_eq!(info["audio_converted"], json!(true), "AC-3 doesn't play in browsers");
    let session = info["session_id"].as_str().unwrap().to_owned();
    assert_eq!(info["url"], json!(format!("/api/playback/{session}/index.m3u8")));
    assert_eq!(
        info["reasons"],
        json!(["This player can't open MKV files.", "This player can't play AC3 audio."]),
        "one plain sentence each"
    );
    assert_eq!(info["reason"], json!("This player can't open MKV files. This player can't play AC3 audio."));

    // The player's details: the session's transcode, live.
    let status = app.get(&format!("/api/playback/{session}"), admin).await;
    assert_eq!(status.status, StatusCode::OK, "{}", status.text());
    assert_eq!(status.body["method"], "remux");
    let live = &status.body["transcode"];
    assert_eq!((&live["video"], &live["audio"]), (&json!("copy"), &json!("transcode")));
    assert_eq!((&live["audio_in"], &live["audio_out"]), (&json!("AC3 5.1"), &json!("AAC 2.0")));
    let (other, _) = app.user(admin, "nosy", json!({})).await;
    assert_eq!(app.get(&format!("/api/playback/{session}"), &other).await.status, StatusCode::FORBIDDEN);
    let subtitles = info["subtitles"].as_array().unwrap();
    let urls: Vec<&Value> = subtitles.iter().map(|s| &s["url"]).collect();
    assert_eq!(subtitles.len(), 3);
    assert_eq!(urls[0], &Value::Null, "image subtitles have no WebVTT");
    assert!(urls[1].as_str().unwrap().ends_with("/subtitles/4.vtt"));

    // What the transcoder was asked for.
    let job = w.fake.last_request();
    assert!(job.input.ends_with("Feature (2021).mkv"));
    assert_eq!((job.label.as_deref(), job.owner.as_deref()), (Some("Feature (2021)"), Some("admin")));
    assert_eq!(job.video.as_ref().unwrap().mode, VideoMode::Copy);
    let audio = job.audio.as_ref().unwrap();
    assert_eq!((audio.mode, audio.stream_index), (AudioMode::Transcode, Some(1)));
    assert_eq!(job.media.as_ref().and_then(|m| m.duration_ms), Some(60_000), "probe data is passed on");

    let playlist = app.get(&format!("/api/playback/{session}/index.m3u8"), admin).await;
    assert_eq!(playlist.status, StatusCode::OK);
    assert_eq!(playlist.headers[header::CONTENT_TYPE], "application/vnd.apple.mpegurl");
    assert!(playlist.text().contains("14.ts\n#EXT-X-ENDLIST"));
    let segment = app.get(&format!("/api/playback/{session}/3.ts"), admin).await;
    assert_eq!((segment.status, segment.text()), (StatusCode::OK, "job1:3".to_owned()));
    assert_eq!(segment.headers[header::CONTENT_TYPE], "video/mp2t");
    // Out of range is not a lost job: nothing is recreated.
    assert_eq!(app.get(&format!("/api/playback/{session}/99.ts"), admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&format!("/api/playback/{session}/x.ts"), admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(w.fake.created(), 1);

    let activity = app.get("/api/activity", admin).await;
    let listed = &activity.body["sessions"][0];
    assert_eq!((&listed["method"], &listed["item"]["title"]), (&json!("remux"), &json!("Feature")));
    assert_eq!(listed["transcode"]["video"], json!("copy"));
    assert_eq!(listed["transcode"]["audio_out"], json!("AAC 2.0"));

    // Stopping ends the transcoder job.
    let stopped = app.delete(&format!("/api/playback/{session}"), admin).await;
    assert_eq!(stopped.status, StatusCode::NO_CONTENT);
    eventually(|| w.fake.deleted.lock().contains(&"job1".to_owned())).await;
    let gone = app.get(&format!("/api/playback/{session}/index.m3u8"), admin).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    assert_eq!(gone.body["message"], json!("playback session expired"));
}

#[tokio::test]
async fn a_lost_job_is_recreated_once_with_the_same_layout() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let feature = w.movies["Feature"];
    let started = play(app, admin, json!({"item_id": feature, "caps": browser()})).await;
    let session = started.body["session_id"].as_str().unwrap().to_owned();
    let progress = json!({"position_ms": 20_000, "duration_ms": 60_000, "state": "playing", "session_id": session});
    assert_eq!(
        app.post(&format!("/api/items/{feature}/progress"), admin, progress).await.status,
        StatusCode::NO_CONTENT
    );

    // The transcoder restarts; two segment requests arrive at once.
    w.fake.lose_jobs();
    let five = format!("/api/playback/{session}/5.ts");
    let six = format!("/api/playback/{session}/6.ts");
    let (a, b) = tokio::join!(app.get(&five, admin), app.get(&six, admin));
    assert_eq!((a.status, a.text()), (StatusCode::OK, "job2:5".to_owned()));
    assert_eq!((b.status, b.text()), (StatusCode::OK, "job2:6".to_owned()));
    assert_eq!(w.fake.created(), 2, "one replacement for both requests");
    let replacement = w.fake.last_request();
    assert_eq!(replacement.start_ms, Some(20_000), "pre-started at the playhead");
    assert_eq!(replacement.video.unwrap().mode, VideoMode::Copy, "the same effective mode");
    assert_eq!(app.get(&format!("/api/playback/{session}/index.m3u8"), admin).await.status, StatusCode::OK);

    // This time it can't give the same segments: the session is over.
    *w.fake.layout.lock() = Some(10);
    w.fake.lose_jobs();
    let lost = app.get(&five, admin).await;
    assert_eq!(lost.status, StatusCode::NOT_FOUND);
    assert_eq!(lost.body["message"], json!("playback session expired"));
    eventually(|| w.fake.deleted.lock().contains(&"job3".to_owned())).await;
    assert!(w.fake.jobs.lock().is_empty(), "the mismatched job is not left running");
    assert_eq!(app.get(&six, admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(w.fake.created(), 3, "no further attempts");
}

#[tokio::test]
async fn the_transcode_limit_counts_live_hls_sessions() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    app.put("/api/settings", admin, json!({"max_transcodes": 1})).await;
    let feature = w.movies["Feature"];
    let start = |client: &str| json!({"item_id": feature, "caps": browser(), "client_id": client});

    let tab = play(app, admin, start("tab")).await;
    assert_eq!(tab.status, StatusCode::OK);
    let tv = play(app, admin, start("tv")).await;
    assert_eq!(tv.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(tv.body["code"], json!("unavailable"));
    assert!(tv.body["message"].as_str().unwrap().contains("maximum of 1"));
    // Direct play never counts.
    let clip = play(app, admin, json!({"item_id": w.movies["Clip"], "caps": browser(), "client_id": "tv"})).await;
    assert_eq!((clip.status, clip.body["method"].clone()), (StatusCode::OK, json!("direct")));

    // The same client switching quality replaces its own session.
    let switched =
        play(app, admin, json!({"item_id": feature, "caps": browser(), "client_id": "tab", "max_height": 720})).await;
    assert_eq!(switched.status, StatusCode::OK, "{:?}", switched.body);
    assert_eq!(switched.body["method"], json!("transcode"));
    eventually(|| w.fake.deleted.lock().contains(&"job1".to_owned())).await;

    // A stopped player frees its slot.
    let session = switched.body["session_id"].as_str().unwrap();
    let stop = json!({"position_ms": 1_000, "state": "stopped", "session_id": session});
    app.post(&format!("/api/items/{feature}/progress"), admin, stop).await;
    assert_eq!(play(app, admin, start("tv")).await.status, StatusCode::OK);
}

#[tokio::test]
async fn simultaneous_play_requests_stay_within_the_transcode_limit() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    app.put("/api/settings", admin, json!({"max_transcodes": 1})).await;
    let feature = w.movies["Feature"];
    let start = |client: &str| json!({"item_id": feature, "caps": browser(), "client_id": client});

    // Creating a job takes a while: all three requests are waiting at once.
    *w.fake.job_delay.lock() = Duration::from_millis(300);
    let (a, b, c) =
        tokio::join!(play(app, admin, start("a")), play(app, admin, start("b")), play(app, admin, start("c")));
    let started: Vec<_> = [&a, &b, &c].into_iter().filter(|r| r.status == StatusCode::OK).collect();
    assert_eq!(started.len(), 1, "{:?}", [a.status, b.status, c.status]);
    let refused = [&a, &b, &c].into_iter().filter(|r| r.status == StatusCode::SERVICE_UNAVAILABLE).count();
    assert_eq!(refused, 2);
    assert_eq!(w.fake.created(), 1, "only one job was asked for");

    // A request whose job can't be created gives its slot back.
    let session = started[0].body["session_id"].as_str().unwrap();
    assert_eq!(app.delete(&format!("/api/playback/{session}"), admin).await.status, StatusCode::NO_CONTENT);
    w.fake.fail_jobs.store(true, Ordering::SeqCst);
    assert_eq!(play(app, admin, start("d")).await.status, StatusCode::INTERNAL_SERVER_ERROR);
    w.fake.fail_jobs.store(false, Ordering::SeqCst);
    assert_eq!(play(app, admin, start("e")).await.status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_belong_to_their_user() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let (bob, _) = app.user(admin, "bob", json!({})).await;
    let (alice, _) = app.user(admin, "alice", json!({})).await;
    let started = play(app, &bob, json!({"item_id": w.movies["Feature"], "caps": browser()})).await;
    let session = started.body["session_id"].as_str().unwrap().to_owned();
    let playlist = format!("/api/playback/{session}/index.m3u8");

    assert_eq!(app.get(&playlist, &alice).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.delete(&format!("/api/playback/{session}"), &alice).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get(&playlist, admin).await.status, StatusCode::OK, "administrators may look in");
    assert_eq!(app.get(&playlist, &bob).await.status, StatusCode::OK);
    assert_eq!(app.get("/api/playback/nope/index.m3u8", &bob).await.status, StatusCode::NOT_FOUND);

    let sessions = |token: &str| {
        let app = &w.app;
        let token = token.to_owned();
        async move { app.get("/api/activity", &token).await.body["sessions"].as_array().unwrap().len() }
    };
    assert_eq!((sessions(&bob).await, sessions(&alice).await, sessions(admin).await), (1, 0, 1));
}

#[tokio::test]
async fn subtitles_are_converted_once() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let file = app.file_of(w.movies["Feature"]).await;
    let embedded = format!("/api/files/{file}/subtitles/4.vtt");
    let first = app.get(&embedded, admin).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.headers[header::CONTENT_TYPE], "text/vtt; charset=utf-8");
    assert!(first.text().starts_with("WEBVTT"));
    assert_eq!(app.get(&embedded, admin).await.text(), first.text(), "from the cache");
    assert_eq!(w.fake.subtitles.lock().len(), 1);
    let request = w.fake.subtitles.lock()[0].clone();
    assert!(request.path.ends_with("Feature (2021).mkv"));
    assert_eq!(request.stream_index, Some(4));

    // A sidecar is converted from its own file.
    let sidecar = app.get(&format!("/api/files/{file}/subtitles/1000.vtt"), admin).await;
    assert_eq!(sidecar.status, StatusCode::OK);
    let request = w.fake.subtitles.lock()[1].clone();
    assert!(request.path.ends_with("Feature (2021).en.srt"));
    assert_eq!(request.stream_index, None);
    for missing in ["1001.vtt", "4.srt", "x.vtt"] {
        let reply = app.get(&format!("/api/files/{file}/subtitles/{missing}"), admin).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{missing}");
    }
    assert_eq!(w.fake.subtitles.lock().len(), 2);
}

#[tokio::test]
async fn subtitles_are_pulled_out_once_however_many_ask_and_whoever_leaves() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let file = app.file_of(w.movies["Feature"]).await;
    let asked = |stream: i64| w.fake.subtitles.lock().iter().filter(|r| r.stream_index == Some(stream)).count();

    // Players asking at once (a restart, another tab) wait for one extraction.
    let slow = format!("/api/files/{file}/subtitles/5.vtt");
    let replies = futures::future::join_all((0..3).map(|_| app.get(&slow, admin))).await;
    assert!(replies.iter().all(|r| r.status == StatusCode::OK && r.text() == replies[0].text()));
    assert_eq!(asked(5), 1);

    // A player that stops waiting doesn't stop it: what it asked for is cached.
    let left = format!("/api/files/{file}/subtitles/6.vtt");
    assert!(tokio::time::timeout(Duration::from_millis(100), app.get(&left, admin)).await.is_err());
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(app.get(&left, admin).await.status, StatusCode::OK);
    assert_eq!(asked(6), 1, "served from the cache");
}

#[tokio::test]
async fn subtitles_are_cached_whole_and_follow_their_sidecar() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let file = app.file_of(w.movies["Feature"]).await;
    let mtime: i64 = sqlx::query_scalar("SELECT mtime FROM media_files WHERE id = ?")
        .bind(file)
        .fetch_one(&app.state.db.read)
        .await
        .unwrap();
    let cache = app.state.config.cache_dir().join("subtitles");
    let converted = || w.fake.subtitles.lock().len();

    // Left cut short by a crash (earlier versions wrote in place): made again.
    let embedded = cache.join(format!("{file}_4_{mtime}.vtt"));
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(&embedded, b"").unwrap();
    let reply = app.get(&format!("/api/files/{file}/subtitles/4.vtt"), admin).await;
    assert!(reply.text().starts_with("WEBVTT"), "{}", reply.text());
    assert_eq!(converted(), 1);
    assert_eq!(std::fs::read_to_string(&embedded).unwrap(), reply.text());

    // A conversion that fails leaves nothing behind.
    let failed = app.get(&format!("/api/files/{file}/subtitles/99.vtt"), admin).await;
    assert!(failed.status.is_server_error(), "{}", failed.status);
    let names: Vec<String> =
        std::fs::read_dir(&cache).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, [format!("{file}_4_{mtime}.vtt")]);

    // A sidecar replaced by a better one (as subtitle downloaders do) is
    // converted again, though its video did not change.
    let sidecar = format!("/api/files/{file}/subtitles/1000.vtt");
    app.get(&sidecar, admin).await;
    app.get(&sidecar, admin).await;
    assert_eq!(converted(), 3, "the failed one, then the sidecar once");
    let srt = app.media.path().join("movies/Feature (2021)/Feature (2021).en.srt");
    std::fs::write(&srt, b"1\n00:00:00,000 --> 00:00:01,000\nBetter\n").unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(10);
    std::fs::File::options().write(true).open(&srt).unwrap().set_modified(later).unwrap();
    assert_eq!(app.get(&sidecar, admin).await.status, StatusCode::OK);
    assert_eq!(converted(), 4);
}

#[tokio::test]
async fn image_subtitles_and_other_tracks_need_a_transcode() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let feature = w.movies["Feature"];
    let burned = play(app, admin, json!({"item_id": feature, "caps": browser(), "subtitle_index": 3})).await;
    assert_eq!(burned.status, StatusCode::OK, "{:?}", burned.body);
    assert_eq!((&burned.body["method"], &burned.body["burn_subtitles"]), (&json!("transcode"), &json!(true)));
    assert_eq!(burned.body["subtitle_index"], json!(3));
    let job = w.fake.last_request();
    assert_eq!(job.subtitle.as_ref().and_then(|s| s.stream_index), Some(3));
    assert_eq!(job.video.unwrap().mode, VideoMode::Transcode, "burning needs new frames");

    // The French AAC track plays in a remux with the audio copied.
    let french = play(app, admin, json!({"item_id": feature, "caps": browser(), "audio_index": 2})).await;
    assert_eq!((french.body["method"].clone(), french.body["audio_index"].clone()), (json!("remux"), json!(2)));
    assert_eq!(french.body["audio_converted"], json!(false));
    let audio = w.fake.last_request().audio.unwrap();
    assert_eq!((audio.mode, audio.stream_index), (AudioMode::Copy, Some(2)));
    // And starting later starts the transcoder there.
    play(app, admin, json!({"item_id": feature, "caps": browser(), "start_ms": 42_000})).await;
    assert_eq!(w.fake.last_request().start_ms, Some(42_000));
}

#[tokio::test]
async fn frame_grabs_are_made_once_per_file() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let thumb = format!("/api/items/{}/images/thumb?w=320", w.movies["Clip"]);
    let first = app.get(&thumb, admin).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(first.headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(app.get(&thumb, admin).await.status, StatusCode::OK);
    assert_eq!(w.fake.thumbnails.load(Ordering::SeqCst), 1, "kept in the file's media folder");
}

#[tokio::test]
async fn plex_clients_stream_through_the_universal_transcoder() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let feature = w.movies["Feature"];
    let file = app.file_of(feature).await;
    let path = encode(&format!("/library/metadata/{feature}"));
    let client = [("x-plex-client-identifier", "living-room"), ("x-plex-product", "Plex for TV")];

    let start = format!(
        "/video/:/transcode/universal/start.m3u8?path={path}&offset=30&maxVideoBitrate=2000&videoResolution=1280x720&audioStreamID={}",
        file * 10_000 + 2
    );
    let master = app.plex(Method::GET, &start, admin, &client).await;
    assert_eq!(master.status, StatusCode::OK, "{}", master.text());
    let text = master.text();
    assert!(text.contains("BANDWIDTH=2000000,RESOLUTION=1280x720"), "{text}");
    let variant = text.lines().find(|l| l.starts_with("session/")).unwrap().to_owned();
    let job = w.fake.last_request();
    assert_eq!(job.start_ms, Some(30_000));
    let video = job.video.unwrap();
    assert_eq!((video.mode, video.max_height, video.max_bitrate_kbps), (VideoMode::Transcode, Some(720), Some(1_808)));
    assert_eq!(job.audio.unwrap().stream_index, Some(2), "Plex stream ids map to file streams");

    // The session URLs need no token.
    let base = format!("/video/:/transcode/universal/{}", variant.trim_end_matches("index.m3u8"));
    let playlist = app.plex(Method::GET, &format!("{base}index.m3u8"), "", &[]).await;
    assert_eq!(playlist.status, StatusCode::OK);
    let segment = app.plex(Method::GET, &format!("{base}0.ts"), "", &[]).await;
    assert_eq!((segment.status, segment.text()), (StatusCode::OK, "job1:0".to_owned()));

    // It shows up as a transcode session.
    let sessions = app.plex_json("/transcode/sessions", admin).await;
    assert_eq!(sessions["size"], json!(1));
    assert_eq!(sessions["TranscodeSession"]["videoDecision"], json!("transcode"));

    // directStream=0 forces transcoding where a remux would do.
    let plain = format!("/video/:/transcode/universal/start.m3u8?path={path}&directStream=0");
    app.plex(Method::GET, &plain, admin, &[("x-plex-client-identifier", "phone")]).await;
    assert_eq!(w.fake.last_request().video.unwrap().mode, VideoMode::Transcode);
    let plain = format!("/video/:/transcode/universal/start.m3u8?path={path}");
    app.plex(Method::GET, &plain, admin, &[("x-plex-client-identifier", "tablet")]).await;
    assert_eq!(w.fake.last_request().video.unwrap().mode, VideoMode::Copy);

    // Stopping by client ends that client's session and its job.
    let stop = app.plex(Method::GET, "/video/:/transcode/universal/stop?session=living-room", admin, &[]).await;
    assert_eq!(stop.status, StatusCode::OK);
    eventually(|| w.fake.deleted.lock().contains(&"job1".to_owned())).await;
    assert_eq!(app.plex(Method::GET, &format!("{base}index.m3u8"), "", &[]).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn seek_previews_are_made_once_per_file() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    app.put("/api/settings", admin, json!({"seek_previews": true})).await;
    let clip = app.file_of(w.movies["Clip"]).await;
    let previews = format!("/api/files/{clip}/trickplay");

    // Asking queues them; they are fetched from the transcoder in the background.
    assert_eq!(app.get(&previews, admin).await.status, StatusCode::NOT_FOUND, "being made");
    let mut info = Value::Null;
    for _ in 0..200 {
        let reply = app.get(&previews, admin).await;
        if reply.status == StatusCode::OK {
            info = reply.body;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!((&info["count"], &info["columns"], &info["sheets"]), (&json!(6), &json!(5), &json!(1)), "{info}");
    let sheet = app.get(&format!("{previews}/0.jpg?v={}", info["id"].as_str().unwrap()), admin).await;
    assert_eq!(sheet.status, StatusCode::OK);
    assert_eq!(sheet.headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(sheet.headers[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));
    assert_eq!(app.get(&format!("{previews}/1.jpg"), admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&format!("{previews}/x.png"), admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(w.fake.previews.lock().len(), 1, "made once");

    // A file the transcoder can't do is not asked for again (until a restart).
    let feature = app.file_of(w.movies["Feature"]).await;
    app.get(&format!("/api/files/{feature}/trickplay"), admin).await;
    eventually(|| w.fake.previews.lock().len() == 2).await;
    eventually(|| app.state.trickplay.pending() == 0).await;
    let again = app.get(&format!("/api/files/{feature}/trickplay"), admin).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(app.state.trickplay.pending(), 0, "not queued again");
    // Files ffprobe couldn't read have no video to preview.
    let broken = app.file_of(w.movies["Broken"]).await;
    assert_eq!(app.get(&format!("/api/files/{broken}/trickplay"), admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.state.trickplay.pending(), 0);
    assert_eq!(w.fake.previews.lock().len(), 2);
}
