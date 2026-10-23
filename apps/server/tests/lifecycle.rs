//! Starting and stopping the server: one server per data directory, nothing
//! touched before the ports are bound, what shutdown waits for and for how
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
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use common::TestApp;

fn config(data: &std::path::Path) -> Config {
    let mut config = Config::for_data_dir(data);
    config.transcoder_path = Some(data.join("no-transcoder-in-tests"));
    config
}

/// The `pley` executable, without the `PLEY_*` settings of whoever runs the
/// tests.
fn pley_exe() -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_pley"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("PLEY_") {
            command.env_remove(key);
        }
    }
    command.kill_on_drop(true);
    command
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_server_per_data_directory() {
    let data = tempfile::tempdir().unwrap();
    let (_, ctx, server) = start(config(data.path())).await;

    // A second copy gives up at once, before it opens the database.
    let second = tokio::time::timeout(Duration::from_secs(10), pley::run(config(data.path()))).await;
    let message =
        format!("{:#}", second.expect("the second copy gives up at once").expect_err("the directory is taken"));
    assert!(message.starts_with(&format!("another pley (process {}) uses", std::process::id())), "{message}");

    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();
    // The lock went with the server that held it.
    let (_, ctx, server) = start(config(data.path())).await;
    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
async fn a_taken_port_stops_the_start_before_the_database_is_opened() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut config = config(data.path());
    config.bind = taken.local_addr().unwrap();
    let message = format!("{:#}", pley::run(config).await.expect_err("the port is taken"));
    assert!(message.contains(&format!("cannot listen on {}", taken.local_addr().unwrap())), "{message}");
    for made_by_the_start in ["pley.db", "cache", "transcode"] {
        assert!(!data.path().join(made_by_the_start).exists(), "{made_by_the_start} exists");
    }
}

/// Runs `pley serve` with `args` and the data directory `data`, on a port
/// that is taken (so it never runs for real, whatever happens), and returns
/// its exit status, its standard error and its log file.
async fn failed_start(data: &std::path::Path, args: &[&str]) -> (std::process::ExitStatus, String, String) {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let run = pley_exe()
        .args(["--no-tray", "--plex-bind", "off", "--bind"])
        .arg(taken.local_addr().unwrap().to_string())
        .arg("--data-dir")
        .arg(data)
        .args(args)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(60), run).await.expect("pley exits").unwrap();
    let log = std::fs::read_to_string(data.join("logs").join("pley.log")).unwrap_or_default();
    (output.status, String::from_utf8_lossy(&output.stderr).into_owned(), log)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn why_the_server_could_not_start_is_in_its_log() {
    // The port is taken.
    let data = tempfile::tempdir().unwrap();
    let (status, stderr, log) = failed_start(data.path(), &[]).await;
    assert!(!status.success());
    assert!(log.contains("ERROR pley could not start: cannot listen on 127.0.0.1:"), "{log}");
    assert_eq!(stderr.matches("cannot listen on").count(), 1, "on the console once: {stderr}");

    // Another pley uses the data directory (here, this process).
    let busy = tempfile::tempdir().unwrap();
    let (_, ctx, server) = start(config(busy.path())).await;
    let (status, _, log) = failed_start(busy.path(), &[]).await;
    assert!(!status.success());
    let holder = format!("pley could not start: another pley (process {}) uses the data", std::process::id());
    assert!(log.contains(&holder), "{log}");
    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();

    // pley.toml can't be read: the log of the data directory it is in.
    let broken = tempfile::tempdir().unwrap();
    std::fs::write(broken.path().join("pley.toml"), "bindd = \"8080\"\n").unwrap();
    let (status, _, log) = failed_start(broken.path(), &[]).await;
    assert!(!status.success());
    assert!(log.contains("ERROR pley could not start: parsing config file"), "{log}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_password_works_next_to_a_running_server() {
    let data = tempfile::tempdir().unwrap();
    let (addr, ctx, server) = start(config(data.path())).await;

    let run =
        pley_exe().args(["reset-password", "admin", "password1", "--admin", "--data-dir"]).arg(data.path()).output();
    let output = tokio::time::timeout(Duration::from_secs(60), run).await.expect("pley exits").unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("created user 'admin' (admin)"));
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let login = http
        .post(format!("http://{addr}/api/auth/login"))
        .json(&json!({"username": "admin", "password": "password1"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), reqwest::StatusCode::OK, "the running server sees the new account");

    ctx.send(Command::Quit);
    stopped(server, Duration::from_secs(5)).await.unwrap();
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
