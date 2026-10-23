//! Starting and stopping the server: what shutdown waits for, and for how
//! long. Servers here are stopped through their tray context's Quit (see
//! `tests/shutdown.rs` for `pley::request_shutdown`).

mod common;

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use pley::Config;
use pley::desktop::{Command, TrayContext, TrayLauncher};
use pley::events::ServerEvent;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use common::TestApp;

fn config(data: &std::path::Path) -> Config {
    let mut config = Config::for_data_dir(data);
    config.transcoder_path = Some(data.join("no-transcoder-in-tests"));
    config
}

/// pley running in this process: its address (`127.0.0.1:port`), the tray
/// context it handed out once it listened, and the task running it.
async fn start(config: Config) -> (String, TrayContext, JoinHandle<anyhow::Result<()>>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let launcher: TrayLauncher = Box::new(move |ctx: TrayContext| {
        let _ = tx.send(ctx);
        None
    });
    let server = tokio::spawn(pley::run_with_tray(config, Some(launcher)));
    let ctx = match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(ctx)) => ctx,
        _ => panic!("pley did not start: {:?}", tokio::time::timeout(Duration::from_secs(5), server).await),
    };
    let port: u16 = ctx.url.rsplit(':').next().and_then(|p| p.parse().ok()).expect("a port in the URL");
    (format!("127.0.0.1:{port}"), ctx, server)
}

async fn stopped(server: JoinHandle<anyhow::Result<()>>, within: Duration) -> anyhow::Result<()> {
    tokio::time::timeout(within, server).await.expect("pley stopped in time").expect("pley did not panic")
}

#[tokio::test]
async fn the_event_stream_ends_when_the_server_stops() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let request = Request::builder()
        .uri("/api/events")
        .header("authorization", format!("Bearer {admin}"))
        .body(Body::empty())
        .unwrap();
    let response = tower::ServiceExt::oneshot(app.api.clone(), request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = response.into_body().into_data_stream();

    app.state.events.send(ServerEvent::LibrariesChanged);
    let event = tokio::time::timeout(Duration::from_secs(5), stream.next()).await.expect("an event");
    let event = event.expect("the stream is open").unwrap();
    assert!(String::from_utf8_lossy(&event).contains("libraries_changed"));

    app.state.stopping.cancel();
    let end = tokio::time::timeout(Duration::from_secs(5), stream.next()).await.expect("the stream ends");
    assert!(end.is_none(), "{end:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tray_quit_stops_its_server() {
    let data = tempfile::tempdir().unwrap();
    let (addr, ctx, server) = start(config(data.path())).await;
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let health = http.get(format!("http://{addr}/api/health")).send().await.unwrap();
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    assert_ne!(ctx.status(), "shutting down");

    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();
    assert!(http.get(format!("http://{addr}/api/health")).send().await.is_err(), "the listener is closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requests_in_progress_get_a_grace_period_and_no_more() {
    let data = tempfile::tempdir().unwrap();
    let mut config = config(data.path());
    config.shutdown_grace = Duration::from_millis(500);
    let (addr, ctx, server) = start(config).await;

    // A request whose body never comes keeps its connection busy, as a film
    // playing directly does. "100 Continue" shows the handler reads the body.
    let mut upload = tokio::net::TcpStream::connect(&addr).await.unwrap();
    upload
        .write_all(
            b"POST /api/setup HTTP/1.1\r\nhost: pley\r\ncontent-type: application/json\r\n\
              content-length: 64\r\nexpect: 100-continue\r\n\r\n",
        )
        .await
        .unwrap();
    let mut reply = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(5), upload.read(&mut reply)).await.unwrap().unwrap();
    let reply = String::from_utf8_lossy(&reply[..n]);
    assert!(reply.starts_with("HTTP/1.1 100"), "{reply}");

    let asked = Instant::now();
    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();
    assert!(asked.elapsed() >= Duration::from_millis(500), "the request in progress had its grace period");
}
