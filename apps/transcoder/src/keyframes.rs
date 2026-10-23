// SPDX-License-Identifier: LicenseRef-Restricted
//! Keyframe index used to segment stream-copied (remuxed) video exactly on
//! keyframes. Building an index reads the whole file, so it runs in the
//! background one file at a time and is cached on disk; jobs fall back to
//! transcoding until the index exists.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, bail};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::ffmpeg::Ffmpeg;
use crate::metrics::Metrics;
use crate::util::{file_key, input_arg};

const MEMORY_ENTRIES: usize = 256;

pub struct KeyframeStore {
    dir: PathBuf,
    memory: Mutex<HashMap<String, Arc<Vec<i64>>>>,
    pending: Mutex<HashSet<String>>,
    queue: mpsc::UnboundedSender<(String, PathBuf)>,
}

impl KeyframeStore {
    pub fn new(ff: Arc<Ffmpeg>, metrics: Arc<Metrics>, dir: PathBuf) -> Arc<Self> {
        let _ = std::fs::create_dir_all(&dir);
        let (tx, mut rx) = mpsc::unbounded_channel::<(String, PathBuf)>();
        let store = Arc::new(Self { dir, memory: Mutex::default(), pending: Mutex::default(), queue: tx });
        let worker = Arc::downgrade(&store);
        tokio::spawn(async move {
            while let Some((key, path)) = rx.recv().await {
                let Some(store) = worker.upgrade() else { break };
                Metrics::inc(&metrics.keyframe_scans);
                let started = std::time::Instant::now();
                let cues = {
                    let path = path.clone();
                    tokio::task::spawn_blocking(move || crate::matroska::cue_keyframes(&path)).await.ok().flatten()
                };
                let result = match cues {
                    Some(times) => Ok(times),
                    None => scan(&ff, &path).await,
                };
                match result {
                    Ok(times) => {
                        tracing::debug!(
                            "keyframe index for {} ({} keyframes) built in {:.1}s",
                            path.display(),
                            times.len(),
                            started.elapsed().as_secs_f32()
                        );
                        store.save(&key, times);
                    }
                    Err(err) => tracing::warn!("keyframe scan failed for {}: {err:#}", path.display()),
                }
                store.pending.lock().remove(&key);
            }
        });
        store
    }

    /// Cached keyframe times (ms) for `path`, if an index exists.
    pub fn get(&self, path: &Path) -> Option<Arc<Vec<i64>>> {
        let key = file_key(path).ok()?;
        if let Some(hit) = self.memory.lock().get(&key) {
            return Some(hit.clone());
        }
        let text = std::fs::read_to_string(self.dir.join(format!("{key}.txt"))).ok()?;
        let times: Vec<i64> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
        if times.is_empty() {
            return None;
        }
        let times = Arc::new(times);
        self.remember(key, times.clone());
        Some(times)
    }

    /// Like [`get`](Self::get), but for Matroska files without a cached index
    /// first tries the file's Cues element, which takes milliseconds.
    pub async fn get_or_cues(&self, path: &Path) -> Option<Arc<Vec<i64>>> {
        if let Some(hit) = self.get(path) {
            return Some(hit);
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "mkv" | "webm" | "mk3d") {
            return None;
        }
        let key = file_key(path).ok()?;
        let file = path.to_path_buf();
        let times = tokio::task::spawn_blocking(move || crate::matroska::cue_keyframes(&file)).await.ok().flatten()?;
        tracing::debug!("keyframe index for {} read from Matroska cues ({} keyframes)", path.display(), times.len());
        self.save(&key, times.clone());
        Some(Arc::new(times))
    }

    /// Queue an index build unless one exists or is already queued.
    pub fn request(&self, path: &Path) {
        let Ok(key) = file_key(path) else { return };
        if self.memory.lock().contains_key(&key) || self.dir.join(format!("{key}.txt")).exists() {
            return;
        }
        if self.pending.lock().insert(key.clone()) {
            let _ = self.queue.send((key, path.to_path_buf()));
        }
    }

    fn save(&self, key: &str, times: Vec<i64>) {
        if times.is_empty() {
            return;
        }
        let text: String = times.iter().map(|t| format!("{t}\n")).collect();
        let tmp = self.dir.join(format!("{key}.tmp"));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join(format!("{key}.txt")));
        }
        self.remember(key.to_owned(), Arc::new(times));
    }

    fn remember(&self, key: String, times: Arc<Vec<i64>>) {
        let mut memory = self.memory.lock();
        if memory.len() >= MEMORY_ENTRIES {
            memory.clear();
        }
        memory.insert(key, times);
    }
}

/// Read every video packet header and collect keyframe timestamps.
async fn scan(ff: &Ffmpeg, path: &Path) -> anyhow::Result<Vec<i64>> {
    let mut cmd = ff.probe_command();
    cmd.args(["-v", "error", "-select_streams", "v:0", "-show_entries", "packet=pts_time,flags", "-of", "csv=p=0"])
        .arg(input_arg(path))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().context("spawning ffprobe")?;
    let stdout = child.stdout.take().context("no stdout")?;
    let mut lines = BufReader::with_capacity(1 << 16, stdout).lines();
    let mut times = Vec::new();
    while let Some(line) = lines.next_line().await? {
        if let Some(t) = parse_packet_line(&line) {
            times.push(t);
        }
    }
    let status = child.wait().await?;
    if !status.success() && times.is_empty() {
        bail!("ffprobe exited with {status}");
    }
    times.sort_unstable();
    times.dedup();
    Ok(times)
}

/// `"12.345000,K__"` → `Some(12345)` for keyframes.
fn parse_packet_line(line: &str) -> Option<i64> {
    let (pts, flags) = line.split_once(',')?;
    if !flags.starts_with('K') {
        return None;
    }
    let secs: f64 = pts.trim().parse().ok()?;
    (secs.is_finite() && secs >= 0.0).then(|| (secs * 1000.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packet_lines() {
        assert_eq!(parse_packet_line("12.345000,K__"), Some(12_345));
        assert_eq!(parse_packet_line("12.345000,___"), None);
        assert_eq!(parse_packet_line("N/A,K__"), None);
        assert_eq!(parse_packet_line("garbage"), None);
        assert_eq!(parse_packet_line("-0.042000,K__"), None, "before the start");
        assert_eq!(parse_packet_line(" 0.5 ,KD_"), Some(500));
    }

    fn store(dir: &Path) -> Arc<KeyframeStore> {
        let ff = Arc::new(Ffmpeg {
            ffmpeg: dir.join("no-ffmpeg"),
            ffprobe: dir.join("no-ffprobe"),
            caps: Default::default(),
            segment_times_relative: true,
        });
        KeyframeStore::new(ff, Arc::new(Metrics::default()), dir.join("keyframes"))
    }

    #[tokio::test]
    async fn indexes_are_kept_per_file_version() {
        let dir = tempfile::tempdir().unwrap();
        let movie = dir.path().join("movie.mp4");
        std::fs::write(&movie, b"frames").unwrap();
        let keyframes = store(dir.path());
        assert!(keyframes.get(&movie).is_none());
        keyframes.save(&file_key(&movie).unwrap(), vec![0, 2_002, 4_004]);
        assert_eq!(keyframes.get(&movie).as_deref(), Some(&vec![0, 2_002, 4_004]));
        // On disk too: a new store (a restarted transcoder) finds it.
        let restarted = store(dir.path());
        assert_eq!(restarted.get(&movie).as_deref(), Some(&vec![0, 2_002, 4_004]));
        // An index exists: no scan is queued.
        restarted.request(&movie);
        assert!(restarted.pending.lock().is_empty());
        // A replaced file needs a new index; empty ones are never stored.
        std::fs::write(&movie, b"other frames").unwrap();
        assert!(restarted.get(&movie).is_none());
        restarted.save(&file_key(&movie).unwrap(), vec![]);
        assert!(restarted.get(&movie).is_none());
        // Only Matroska files have cues to read; others wait for a scan.
        assert!(restarted.get_or_cues(&movie).await.is_none());
    }
}
