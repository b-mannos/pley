//! Settings → Transcoder against what the transcoder found (docs/API.md,
//! "System"): `/api/system` says which choices can work, and
//! `PUT /api/settings` refuses one that can't, while a choice saved before
//! (another GPU, another FFmpeg) keeps loading.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use pley_transcoder_api::{API_VERSION, Capabilities, EncoderInfo, HwAccel, Tonemapper, TranscoderInfo, VideoCodec};
use serde_json::json;

use common::TestApp;

fn encoder(name: &str, accel: HwAccel) -> EncoderInfo {
    EncoderInfo { name: name.into(), codec: VideoCodec::H264, hardware: accel != HwAccel::None, accel, main10: false }
}

/// A transcoder elsewhere, on a computer with an Intel graphics chip, whose
/// FFmpeg has zscale but neither libplacebo nor Chromaprint.
async fn intel_transcoder() -> String {
    let app = Router::new().route(
        "/v1/info",
        get(|| async {
            Json(TranscoderInfo {
                api_version: API_VERSION,
                capabilities: Capabilities {
                    ffmpeg_available: true,
                    encoders: vec![encoder("h264_qsv", HwAccel::Qsv), encoder("libx264", HwAccel::None)],
                    hwaccels: vec!["qsv".into(), "d3d11va".into()],
                    tonemappers: vec![Tonemapper::Zscale],
                    readrate: true,
                    ..Default::default()
                },
                ..Default::default()
            })
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

async fn with_intel() -> (TestApp, String) {
    let base = intel_transcoder().await;
    let app = TestApp::with_config(|c| c.transcoder_url = Some(base)).await;
    app.state.transcoder.start();
    assert!(app.state.transcoder.wait_ready(Duration::from_secs(10)).await);
    let admin = app.setup_admin().await;
    (app, admin)
}

#[tokio::test]
async fn the_system_says_what_the_transcoder_can_do() {
    let (app, admin) = with_intel().await;
    let tc = app.get("/api/system", &admin).await.body["transcoder"].clone();
    assert_eq!(tc["managed"], json!(false));
    assert_eq!(tc["accels"], json!(["qsv"]), "{tc}");
    assert_eq!(tc["hardware_decoding"], json!(true), "Quick Sync decodes through D3D11");
    assert_eq!(tc["tonemapping"], json!(["zscale"]));
    assert_eq!(tc["chromaprint"], json!(false));
    assert_eq!(tc["throttling"], json!(true));
    assert_eq!(tc["active_encoder"], json!("h264_qsv"), "Automatic");

    // Nothing is known without a transcoder.
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let tc = app.get("/api/system", &admin).await.body["transcoder"].clone();
    assert_eq!((tc["available"].clone(), tc["accels"].clone()), (json!(false), json!([])));
    assert_eq!((tc["hardware_decoding"].clone(), tc["chromaprint"].clone()), (json!(false), json!(false)));
}

#[tokio::test]
async fn a_choice_that_cant_work_here_is_refused() {
    let (app, admin) = with_intel().await;
    let put = |body| app.put("/api/settings", &admin, body);

    let refused = put(json!({"hwaccel": "nvenc"})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.mentions("NVIDIA graphics card"), "{:?}", refused.body);
    // A transcoder elsewhere may be on Linux: its encoders say no to VAAPI.
    let refused = put(json!({"hwaccel": "vaapi"})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.mentions("Intel or AMD graphics chip"), "{:?}", refused.body);
    let refused = put(json!({"tonemapping": "libplacebo", "server_name": "Den"})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.mentions("libplacebo"), "{:?}", refused.body);
    assert_ne!(app.get("/api/settings", &admin).await.body["server_name"], json!("Den"), "nothing changed");

    let saved = put(json!({"hwaccel": "qsv", "tonemapping": "zscale"})).await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!((saved.body["hwaccel"].clone(), saved.body["tonemapping"].clone()), (json!("qsv"), json!("zscale")));
    for always in ["auto", "none"] {
        assert_eq!(put(json!({"hwaccel": always, "tonemapping": always})).await.status, StatusCode::OK);
    }
}

#[tokio::test]
async fn a_choice_saved_before_keeps_loading_and_saving() {
    let (app, admin) = with_intel().await;
    // Saved when the computer had an NVIDIA card and FFmpeg libplacebo.
    let before = json!({"hwaccel": "nvenc", "tonemapping": "libplacebo"});
    app.state.settings.update(&app.state.db, serde_json::from_value(before).unwrap()).await.unwrap();

    let settings = app.get("/api/settings", &admin).await;
    assert_eq!(settings.body["hwaccel"], json!("nvenc"));
    // The transcoder encodes in software instead.
    assert_eq!(app.get("/api/system", &admin).await.body["transcoder"]["active_encoder"], json!("libx264"));
    // A form sent whole, with the choice as it was, saves its other changes.
    let mut form = settings.body.clone();
    form["server_name"] = json!("Den");
    let saved = app.put("/api/settings", &admin, form).await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.body);
    assert_eq!((saved.body["server_name"].clone(), saved.body["hwaccel"].clone()), (json!("Den"), json!("nvenc")));
    assert_eq!(saved.body["tonemapping"], json!("libplacebo"));
}

#[tokio::test]
async fn a_managed_transcoder_is_on_this_computers_system() {
    // Not running (as in these tests): its encoders aren't known, its system is.
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (foreign, why) =
        if cfg!(target_os = "linux") { ("videotoolbox", "macOS only") } else { ("vaapi", "Linux only") };
    let refused = app.put("/api/settings", &admin, json!({"hwaccel": foreign})).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.mentions(why), "{:?}", refused.body);
    let native = if cfg!(target_os = "macos") { "videotoolbox" } else { "nvenc" };
    assert_eq!(app.put("/api/settings", &admin, json!({"hwaccel": native})).await.status, StatusCode::OK);
    assert_eq!(app.put("/api/settings", &admin, json!({"tonemapping": "libplacebo"})).await.status, StatusCode::OK);
}
