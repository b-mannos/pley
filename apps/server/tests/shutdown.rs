//! `pley::run` stops when asked to while the web UI holds its event stream
//! open, as it does whenever someone is signed in.
//!
//! The only test of this binary: [`pley::request_shutdown`] stops whichever
//! server of the process sees it first. Other start and stop tests are in
//! `tests/lifecycle.rs`.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::{Value, json};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_shutdown_ends_the_event_stream() {
    let data = tempfile::tempdir().unwrap();
    let mut config = pley::Config::for_data_dir(data.path());
    config.transcoder_path = Some(data.path().join("no-transcoder-in-tests"));
    let port = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap().port();
    config.bind = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let base = format!("http://127.0.0.1:{port}");
    let mut server = tokio::spawn(pley::run(config));

    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut ready = false;
    for _ in 0..400 {
        if server.is_finished() {
            panic!("pley stopped: {:?}", (&mut server).await);
        }
        if http.get(format!("{base}/api/health")).send().await.is_ok_and(|r| r.status().is_success()) {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "pley did not start");
    let setup: Value = http
        .post(format!("{base}/api/setup"))
        .json(&json!({"username": "admin", "password": "secret1"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = setup["token"].as_str().expect("a session token");
    let mut events = http.get(format!("{base}/api/events")).bearer_auth(token).send().await.unwrap();
    assert_eq!(events.status(), reqwest::StatusCode::OK);

    pley::request_shutdown();
    // Well within the grace period for requests in progress: the stream
    // ended, it did not hold the shutdown up until it was given up on.
    let stopped = tokio::time::timeout(Duration::from_secs(5), &mut server).await;
    stopped.expect("pley still runs 5 s after request_shutdown() with /api/events open").unwrap().unwrap();
    let end = tokio::time::timeout(Duration::from_secs(5), events.chunk()).await.expect("the event stream ended");
    assert!(matches!(end, Ok(None)), "the stream ends cleanly: {end:?}");
}
