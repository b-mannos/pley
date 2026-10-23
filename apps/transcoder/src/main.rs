// SPDX-License-Identifier: LicenseRef-Restricted
//
// pley-transcoder — FFmpeg media engine for the pley media server.
// Copyright (c) 2026 the Pley authors. All rights reserved. See LICENSE.

//! Standalone transcoding service. It is the only pley component that runs
//! FFmpeg; the media server talks to it over HTTP (docs/TRANSCODER.md).

mod error;
mod extract;
mod ffmpeg;
mod fingerprint;
mod http;
mod job;
mod keyframes;
mod matroska;
mod metrics;
mod mp4;
mod plan;
mod probe;
mod trickplay;
mod util;

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use pley_transcoder_api::{API_VERSION, Announce, Limits, TOKEN_ENV, TranscoderInfo};
use tokio::io::AsyncReadExt;

const LICENSE_NOTICE: &str = "pley-transcoder  Copyright (c) 2026 the Pley authors. All rights reserved.
Restricted: its licence terms are to be determined, and no licence to copy,
modify or distribute it is granted until then. It comes with ABSOLUTELY NO
WARRANTY. See the LICENSE file.";

#[derive(Debug, Parser)]
#[command(
    name = "pley-transcoder",
    version,
    about = "pley transcoder — FFmpeg media engine for the pley media server"
)]
struct Args {
    /// Address to listen on (port 0 picks a free port)
    #[arg(long, env = "PLEY_TRANSCODER_LISTEN", default_value = "127.0.0.1:7540")]
    listen: SocketAddr,

    /// Shared secret; clients must send `Authorization: Bearer <token>`
    #[arg(long, env = TOKEN_ENV, hide_env_values = true)]
    token: Option<String>,

    /// Directory for segments, caches and the capability cache
    #[arg(long, env = "PLEY_TRANSCODER_WORK_DIR")]
    work_dir: Option<PathBuf>,

    /// ffmpeg binary
    #[arg(long, env = "PLEY_FFMPEG", default_value = "ffmpeg")]
    ffmpeg: PathBuf,

    /// ffprobe binary
    #[arg(long, env = "PLEY_FFPROBE", default_value = "ffprobe")]
    ffprobe: PathBuf,

    /// Maximum concurrent ffmpeg transcode processes
    #[arg(long, env = "PLEY_TRANSCODER_MAX_JOBS", default_value_t = 8)]
    max_jobs: u32,

    /// Maximum concurrent hardware encoder sessions; extra jobs use software encoding
    #[arg(long, env = "PLEY_TRANSCODER_MAX_HW_JOBS", default_value_t = 8)]
    max_hw_jobs: u32,

    /// Concurrent probes / extractions
    #[arg(long, default_value_t = 8)]
    probe_concurrency: usize,

    /// Print `{"event":"listening","addr":"…"}` on stdout once ready
    #[arg(long)]
    announce: bool,

    /// Exit when stdin reaches EOF (used when supervised by the pley server)
    #[arg(long)]
    exit_with_parent: bool,

    /// Ignore the cached FFmpeg capability detection
    #[arg(long)]
    redetect: bool,

    /// Print the license notice and exit
    #[arg(long)]
    license: bool,

    /// Log filter, e.g. `info` or `debug`
    #[arg(long, env = "PLEY_LOG", default_value = "info")]
    log: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.license {
        println!("{LICENSE_NOTICE}");
        return Ok(());
    }
    let filter = if args.log.contains('=') { args.log.clone() } else { format!("{},hyper=warn", args.log) };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_new(filter).unwrap_or_else(|_| "info".into()))
        .with_target(false)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr)
        .compact()
        .init();
    tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("transcoder").build()?.block_on(run(args))
}

async fn run(args: Args) -> anyhow::Result<()> {
    util::kill_children_on_exit();
    let work_dir = args.work_dir.clone().unwrap_or_else(|| std::env::temp_dir().join("pley-transcoder"));
    std::fs::create_dir_all(&work_dir)?;
    let jobs_dir = work_dir.join("jobs");
    let _ = std::fs::remove_dir_all(&jobs_dir);
    std::fs::create_dir_all(&jobs_dir)?;

    if args.token.is_none() && !args.listen.ip().is_loopback() {
        tracing::warn!("listening on {} without a token: anyone who can reach it can use this transcoder", args.listen);
    }

    let ff = Arc::new(
        ffmpeg::Ffmpeg::detect(&args.ffmpeg, &args.ffprobe, &work_dir.join("capabilities.json"), args.redetect).await,
    );
    let metrics = Arc::new(metrics::Metrics::default());
    let prober = Arc::new(probe::Prober::new(ff.clone(), metrics.clone(), args.probe_concurrency));
    let keyframes = keyframes::KeyframeStore::new(ff.clone(), metrics.clone(), work_dir.join("keyframes"));
    let extractor = Arc::new(extract::Extractor::new(
        ff.clone(),
        prober.clone(),
        metrics.clone(),
        work_dir.join("subtitles"),
        args.probe_concurrency.div_ceil(2),
    ));
    let fingerprinter = Arc::new(fingerprint::Fingerprinter::new(ff.clone(), 2));
    let trickplay = Arc::new(trickplay::Trickplay::new(ff.clone(), prober.clone(), work_dir.join("trickplay")));
    let limits = Limits { max_jobs: args.max_jobs.max(1), max_hw_jobs: args.max_hw_jobs };
    let jobs = job::JobManager::new(
        ff.clone(),
        prober.clone(),
        keyframes.clone(),
        extractor.clone(),
        metrics.clone(),
        jobs_dir.clone(),
        limits.clone(),
    );
    jobs.spawn_reaper();

    let info = TranscoderInfo {
        name: "pley-transcoder".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        api_version: API_VERSION,
        license: "LicenseRef-Restricted".into(),
        source: None,
        started_at_ms: util::now_ms(),
        capabilities: ff.caps.clone(),
        limits,
    };
    let state = Arc::new(http::AppState::new(
        jobs.clone(),
        prober,
        extractor,
        keyframes,
        fingerprinter,
        trickplay,
        metrics,
        info,
        args.token.clone(),
        work_dir.clone(),
    ));
    state.spawn_disk_usage();

    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    let addr = listener.local_addr()?;
    if args.announce {
        let line = serde_json::to_string(&Announce { event: "listening".into(), addr: addr.to_string() })?;
        println!("{line}");
        use std::io::Write;
        std::io::stdout().flush()?;
    }
    tracing::info!("pley-transcoder {} listening on http://{addr}", env!("CARGO_PKG_VERSION"));

    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let exit_with_parent = args.exit_with_parent;
    tokio::spawn(async move {
        shutdown_signal(exit_with_parent).await;
        let _ = stop_tx.send(true);
    });
    let mut drain = stop_rx.clone();
    let server = axum::serve(listener, http::router(state))
        .with_graceful_shutdown(async move {
            let _ = drain.wait_for(|stop| *stop).await;
        })
        .into_future();
    tokio::pin!(server);
    let mut stop = stop_rx;
    tokio::select! {
        result = &mut server => result?,
        _ = stop.wait_for(|stop| *stop) => {
            // Stop ffmpeg first: requests waiting for segments then fail right
            // away instead of keeping the graceful drain open.
            jobs.shutdown().await;
            if tokio::time::timeout(Duration::from_secs(5), &mut server).await.is_err() {
                tracing::warn!("some requests were still running at shutdown");
            }
        }
    }

    jobs.shutdown().await;
    let _ = std::fs::remove_dir_all(&jobs_dir);
    tracing::info!("pley-transcoder stopped");
    Ok(())
}

async fn shutdown_signal(exit_with_parent: bool) {
    let parent_gone = async {
        if exit_with_parent {
            let mut stdin = tokio::io::stdin();
            let mut buf = [0u8; 64];
            loop {
                match stdin.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            tracing::info!("parent process went away");
        } else {
            std::future::pending::<()>().await;
        }
    };
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = parent_gone => {}
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
