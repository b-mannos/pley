//! Files kept to play while the server is away. An item page's Download (a
//! link to the server's `/api/files/{id}/stream`, named by the page) is
//! caught by the window, and the app fetches the file itself, one at a
//! time, into Videos/pley: after a dropped connection or a restart it goes
//! on where it stopped. The server's page hears how its own downloads go
//! (`pley:download` events, apps/web/src/lib/appDownloads.ts); the picker
//! lists them all, to play in the system's player or show in their folder.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;
use url::Url;

/// What may be kept: audio and video files only, so that nothing a server
/// names runs as a program when it is opened.
const MEDIA: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "avi", "wmv", "webm", "ts", "m2ts", "mts", "mpg", "mpeg", "vob", "flv", "ogv", "3gp",
    "mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "aif", "aiff", "ape", "wv", "mka", "dsf", "dff",
];

/// The event the server's page hears (apps/web/src/lib/appDownloads.ts).
const PAGE_EVENT: &str = "pley:download";
/// How often the page hears how a download goes.
const PROGRESS_EVERY: Duration = Duration::from_millis(500);
/// How often the list is written while a file comes (it is written at every
/// change of state too, and a restart measures what came anyway).
const SAVE_EVERY: Duration = Duration::from_secs(15);
/// A server that sends nothing for this long has dropped the download.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the server has to start answering (a share asleep can hold it).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);
/// Downloads waiting for their server try again after this, then twice as
/// long each time up to [`RETRY_MAX`].
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// Bytes gathered before each write to the disk.
const WRITE_BUFFER: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Waiting its turn.
    Queued,
    Downloading,
    /// The server can't be reached: tried again in a while.
    Waiting,
    Failed,
    Done,
}

/// A file downloaded, or on its way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Download {
    pub id: u64,
    /// The server's origin (`http://192.168.1.20:7539`).
    pub server: String,
    pub file_id: i64,
    /// What it is called (`Alien (1979).mkv`).
    pub name: String,
    /// Where it is, once whole (it comes into `<path>.part`).
    pub path: PathBuf,
    /// Its size, once the server said.
    pub size: Option<u64>,
    /// What is on the disk so far.
    pub received: u64,
    pub state: State,
    /// Why it failed, to show.
    pub error: Option<String>,
    /// When it was asked for (Unix ms).
    pub added: i64,
}

/// A download as the server's page hears of it: no paths, nothing of
/// other servers.
#[derive(Debug, Serialize)]
struct PageDownload<'a> {
    id: u64,
    name: &'a str,
    size: Option<u64>,
    received: u64,
    state: State,
    error: Option<&'a str>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    next_id: u64,
    downloads: Vec<Download>,
}

/// The app's downloads: the list (kept in `downloads.json`) and the folder
/// the files go to.
pub struct Downloads {
    index: Mutex<Index>,
    /// `downloads.json`, in the app's data folder.
    file: PathBuf,
    /// Where the files go.
    dir: PathBuf,
    /// Wakes the downloader: something was asked for.
    wake: Notify,
}

impl Downloads {
    /// The list as `file` keeps it, with what came of unfinished downloads
    /// measured again, and the files deleted meanwhile forgotten.
    pub fn load(file: PathBuf, dir: PathBuf) -> Self {
        let mut index: Index =
            std::fs::read(&file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        index.downloads.retain_mut(|d| match d.state {
            State::Done => d.path.is_file(),
            State::Failed => true,
            State::Queued | State::Downloading | State::Waiting => {
                d.state = State::Queued;
                d.received = std::fs::metadata(part_of(&d.path)).map(|m| m.len()).unwrap_or(0);
                true
            }
        });
        Self { index: Mutex::new(index), file, dir, wake: Notify::new() }
    }

    /// Every download, newest first.
    pub fn list(&self) -> Vec<Download> {
        let index = self.index.lock().unwrap();
        index.downloads.iter().rev().cloned().collect()
    }

    pub fn get(&self, id: u64) -> Option<Download> {
        self.index.lock().unwrap().downloads.iter().find(|d| d.id == id).cloned()
    }

    /// Asks for file `file_id` of `server` as `name`. Asked again, a file
    /// already kept stays as it is; a failed or waiting one goes again.
    pub fn request(&self, server: &str, file_id: i64, name: &str, now: i64) -> Download {
        let mut index = self.index.lock().unwrap();
        if let Some(d) = index.downloads.iter_mut().find(|d| d.server == server && d.file_id == file_id) {
            let kept = d.state == State::Done && d.path.is_file();
            if !kept && !matches!(d.state, State::Queued | State::Downloading) {
                d.state = State::Queued;
                d.error = None;
            }
            let d = d.clone();
            self.save(&index);
            self.wake.notify_one();
            return d;
        }
        let taken: Vec<&Path> = index.downloads.iter().map(|d| d.path.as_path()).collect();
        let path = free_path(&self.dir, name, &taken);
        index.next_id += 1;
        let d = Download {
            id: index.next_id,
            server: server.to_owned(),
            file_id,
            name: path.file_name().map_or_else(|| name.to_owned(), |n| n.to_string_lossy().into_owned()),
            path,
            size: None,
            received: 0,
            state: State::Queued,
            error: None,
            added: now,
        };
        index.downloads.push(d.clone());
        self.save(&index);
        self.wake.notify_one();
        d
    }

    /// Changes download `id` (and writes the list): `None` once removed.
    fn update(&self, id: u64, change: impl FnOnce(&mut Download)) -> Option<Download> {
        let mut index = self.index.lock().unwrap();
        let d = index.downloads.iter_mut().find(|d| d.id == id)?;
        change(d);
        let d = d.clone();
        self.save(&index);
        Some(d)
    }

    /// Notes what came of download `id`, as it comes: whether it is still wanted.
    fn progress(&self, id: u64, received: u64, size: Option<u64>) -> bool {
        let mut index = self.index.lock().unwrap();
        let Some(d) = index.downloads.iter_mut().find(|d| d.id == id) else { return false };
        d.received = received;
        d.size = size.or(d.size);
        true
    }

    /// Takes download `id` off the list and deletes what it kept.
    pub fn remove(&self, id: u64) -> Option<Download> {
        let mut index = self.index.lock().unwrap();
        let at = index.downloads.iter().position(|d| d.id == id)?;
        let d = index.downloads.remove(at);
        self.save(&index);
        drop(index);
        let _ = std::fs::remove_file(part_of(&d.path));
        if d.state == State::Done {
            let _ = std::fs::remove_file(&d.path);
        }
        Some(d)
    }

    /// Download `id` again, after it failed.
    pub fn retry(&self, id: u64) -> Option<Download> {
        let d = self.update(id, |d| {
            if d.state == State::Failed {
                d.state = State::Queued;
                d.error = None;
            }
        });
        self.wake.notify_one();
        d
    }

    /// The next download to make.
    fn next(&self) -> Option<Download> {
        self.index.lock().unwrap().downloads.iter().find(|d| d.state == State::Queued).cloned()
    }

    /// Downloads that waited for their server go again.
    fn requeue_waiting(&self) {
        let mut index = self.index.lock().unwrap();
        for d in index.downloads.iter_mut().filter(|d| d.state == State::Waiting) {
            d.state = State::Queued;
        }
    }

    fn waiting(&self) -> bool {
        self.index.lock().unwrap().downloads.iter().any(|d| d.state == State::Waiting)
    }

    /// Writes the list whole, so that a crash leaves the old one or the new.
    fn save(&self, index: &Index) {
        let Ok(json) = serde_json::to_vec_pretty(index) else { return };
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let temp = self.file.with_extension("json.tmp");
        if std::fs::write(&temp, json).is_ok() && std::fs::rename(&temp, &self.file).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }

    fn save_now(&self) {
        let index = self.index.lock().unwrap();
        self.save(&index);
    }
}

/// Where a download's file comes while it isn't whole.
pub fn part_of(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

/// Where downloads go: Videos/pley (Downloads/pley without a videos folder).
pub fn folder<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let paths = app.path();
    paths
        .video_dir()
        .or_else(|_| paths.download_dir())
        .map(|dir| dir.join("pley"))
        .or_else(|_| paths.app_data_dir().map(|dir| dir.join("downloads")))
        .unwrap_or_else(|_| PathBuf::from("pley downloads"))
}

/// The file of `url` when it is the original of one of the server's files
/// (`/api/files/{id}/stream`).
pub fn file_of(url: &Url) -> Option<i64> {
    let id = url.path().strip_prefix("/api/files/")?.strip_suffix("/stream")?;
    id.parse().ok().filter(|id| *id > 0)
}

/// A name the page chose made safe to keep: no folders or characters that
/// file systems refuse, no names Windows keeps for devices, and an audio or
/// video file's extension (`None` for anything else).
pub fn safe_name(name: &str) -> Option<String> {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { ' ' } else { c }).collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let (stem, ext) = cleaned.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    // No dot first either: such files are hidden on macOS and Linux.
    let stem = stem.trim_matches(['.', ' ']);
    if stem.is_empty() || !MEDIA.contains(&ext.as_str()) {
        return None;
    }
    // At most 150 characters before the extension: paths stay short.
    let mut stem: String = stem.chars().take(150).collect();
    let device = stem.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((device.starts_with("COM") || device.starts_with("LPT"))
            && device.len() == 4
            && device.as_bytes()[3].is_ascii_digit());
    if reserved {
        stem.insert(0, '_');
    }
    Some(format!("{}.{ext}", stem.trim_end_matches(['.', ' '])))
}

/// `dir/name`, or `dir/name (2)` and on when that is taken by a file, a
/// file on its way, or another download.
fn free_path(dir: &Path, name: &str, taken: &[&Path]) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    (1..)
        .map(|n| match (n, ext) {
            (1, _) => dir.join(name),
            (_, "") => dir.join(format!("{stem} ({n})")),
            _ => dir.join(format!("{stem} ({n}).{ext}")),
        })
        .find(|path| !path.exists() && !part_of(path).exists() && !taken.contains(&path.as_path()))
        .unwrap_or_else(|| dir.join(name))
}

/// What the window does with a download the page asks for: a file of the
/// server shown is downloaded by the app (`false`: not by the web view);
/// anything else (a text file the page made) goes where the system keeps
/// downloads.
pub fn requested<R: Runtime>(app: &AppHandle<R>, server: Option<&Url>, url: &Url, destination: &Path) -> bool {
    let Some(server) = server.filter(|server| server.origin() == url.origin()) else {
        return true;
    };
    let Some(file_id) = file_of(url) else {
        return true;
    };
    let origin = server.origin().ascii_serialization();
    let suggested = destination.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(name) = safe_name(&suggested) else {
        // Not a kind of file to keep.
        tell_page(
            app,
            &origin,
            &PageDownload {
                id: 0,
                name: &suggested,
                size: None,
                received: 0,
                state: State::Failed,
                error: Some("pley keeps audio and video files only."),
            },
        );
        return false;
    };
    let downloads = app.state::<Downloads>();
    let d = downloads.request(&origin, file_id, &name, now_ms());
    announce(app, &d);
    false
}

/// A download the web view made itself (a text file the page made) is
/// over: the page says so, as the web view shows nothing of it once the
/// app handles downloads.
pub fn finished<R: Runtime>(app: &AppHandle<R>, server: Option<&Url>, url: &Url, path: Option<&Path>, success: bool) {
    let (Some(server), Some(name)) = (server, path.and_then(Path::file_name)) else { return };
    if file_of(url).is_some() {
        return;
    }
    let name = name.to_string_lossy();
    tell_page(
        app,
        &server.origin().ascii_serialization(),
        &PageDownload {
            id: 0,
            name: &name,
            size: None,
            received: 0,
            state: if success { State::Done } else { State::Failed },
            error: (!success).then_some("The file couldn't be saved."),
        },
    );
}

/// Tells the page of `d`'s server how it goes (its own downloads only).
fn announce<R: Runtime>(app: &AppHandle<R>, d: &Download) {
    tell_page(
        app,
        &d.server,
        &PageDownload {
            id: d.id,
            name: &d.name,
            size: d.size,
            received: d.received,
            state: d.state,
            error: d.error.as_deref(),
        },
    );
}

/// Sends `download` to the window's page if it is one of `origin`. The
/// check runs in the page, so that a page that has just replaced it (the
/// window went to another server) hears nothing.
fn tell_page<R: Runtime>(app: &AppHandle<R>, origin: &str, download: &PageDownload) {
    let Some(window) = app.get_webview_window("main") else { return };
    let (Ok(origin), Ok(detail)) = (serde_json::to_string(origin), serde_json::to_string(download)) else { return };
    let _ = window.eval(format!(
        "if (window.location.origin === {origin}) window.dispatchEvent(new CustomEvent({PAGE_EVENT:?}, {{ detail: {detail} }}));"
    ));
}

/// The cookies the window has for `url`: the server's session.
fn cookies<R: Runtime>(app: &AppHandle<R>, url: &Url) -> Option<String> {
    let window = app.get_webview_window("main")?;
    // Not on the main thread: on Windows, reading cookies there deadlocks.
    let cookies = window.cookies_for_url(url.clone()).ok()?;
    let header = cookies.iter().map(|c| format!("{}={}", c.name(), c.value())).collect::<Vec<_>>().join("; ");
    (!header.is_empty()).then_some(header)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

/// Makes the downloads asked for, one at a time, for as long as the app runs.
pub async fn run<R: Runtime>(app: AppHandle<R>) {
    let client = match reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).build() {
        Ok(client) => client,
        Err(err) => {
            eprintln!("downloads are off: {err}");
            return;
        }
    };
    let downloads = app.state::<Downloads>();
    let mut retry = RETRY_FIRST;
    loop {
        let Some(next) = downloads.next() else {
            if downloads.waiting() {
                tokio::select! {
                    () = downloads.wake.notified() => {}
                    () = tokio::time::sleep(retry) => {
                        downloads.requeue_waiting();
                        retry = (retry * 2).min(RETRY_MAX);
                    }
                }
            } else {
                downloads.wake.notified().await;
            }
            continue;
        };
        if download(&app, &downloads, &client, &next).await {
            retry = RETRY_FIRST;
        }
    }
}

/// Makes download `d`: whether the server was there.
async fn download<R: Runtime>(
    app: &AppHandle<R>,
    downloads: &Downloads,
    client: &reqwest::Client,
    d: &Download,
) -> bool {
    let Some(d) = downloads.update(d.id, |d| {
        d.state = State::Downloading;
        d.error = None;
    }) else {
        return true;
    };
    announce(app, &d);
    let Ok(url) = Url::parse(&d.server).and_then(|server| server.join(&format!("/api/files/{}/stream", d.file_id)))
    else {
        fail(app, downloads, d.id, "This server's address isn't valid.");
        return true;
    };
    let cookie = cookies(app, &url);
    let part = part_of(&d.path);
    let mut told = Instant::now();
    let mut saved = Instant::now();
    let fetched = fetch(client, &url, cookie.as_deref(), &part, d.size, |received, size| {
        if !downloads.progress(d.id, received, size) {
            return false;
        }
        if told.elapsed() >= PROGRESS_EVERY {
            told = Instant::now();
            if let Some(d) = downloads.get(d.id) {
                announce(app, &d);
            }
        }
        if saved.elapsed() >= SAVE_EVERY {
            saved = Instant::now();
            downloads.save_now();
        }
        true
    })
    .await;
    match fetched {
        Ok(Fetched::Whole) => {
            finish(app, downloads, &d, &part);
            true
        }
        // Removed while it came.
        Ok(Fetched::Stopped) => {
            let _ = tokio::fs::remove_file(&part).await;
            true
        }
        Err(Problem::Gone) => {
            fail(app, downloads, d.id, "This file is no longer on the server.");
            true
        }
        Err(Problem::SignedOut) => {
            fail(app, downloads, d.id, "Sign in to the server again, then download it again.");
            true
        }
        Err(Problem::Disk(err)) => {
            let message = if err.kind() == std::io::ErrorKind::StorageFull {
                "The disk is full.".to_owned()
            } else {
                format!("The file couldn't be written: {err}")
            };
            fail(app, downloads, d.id, &message);
            true
        }
        Err(Problem::Network(why)) => {
            if let Some(d) = downloads.update(d.id, |d| {
                d.state = State::Waiting;
                d.error = Some(why);
            }) {
                announce(app, &d);
            }
            false
        }
    }
}

/// The part file of `d` is whole: it becomes the file.
fn finish<R: Runtime>(app: &AppHandle<R>, downloads: &Downloads, d: &Download, part: &Path) {
    // A file of that name may have come meanwhile: it isn't replaced.
    let path = if d.path.exists() {
        let dir = d.path.parent().unwrap_or(Path::new("."));
        free_path(dir, &d.name, &[])
    } else {
        d.path.clone()
    };
    if let Err(err) = std::fs::rename(part, &path) {
        fail(app, downloads, d.id, &format!("The file couldn't be written: {err}"));
        return;
    }
    let done = downloads.update(d.id, |d| {
        d.state = State::Done;
        d.size = Some(d.received);
        d.error = None;
        if path != d.path {
            d.name = path.file_name().map_or_else(|| d.name.clone(), |n| n.to_string_lossy().into_owned());
            d.path = path.clone();
        }
    });
    match done {
        Some(d) => announce(app, &d),
        // Removed as it finished.
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn fail<R: Runtime>(app: &AppHandle<R>, downloads: &Downloads, id: u64, why: &str) {
    if let Some(d) = downloads.update(id, |d| {
        d.state = State::Failed;
        d.error = Some(why.to_owned());
    }) {
        announce(app, &d);
    }
}

#[derive(Debug, PartialEq)]
enum Fetched {
    Whole,
    /// No longer wanted.
    Stopped,
}

#[derive(Debug)]
enum Problem {
    /// The server has no such file (any more).
    Gone,
    /// The session is over.
    SignedOut,
    /// The server can't be reached or broke off: try again later.
    Network(String),
    Disk(std::io::Error),
}

fn network(err: reqwest::Error) -> Problem {
    Problem::Network(if err.is_timeout() || err.is_connect() {
        "The server can't be reached.".to_owned()
    } else {
        "The connection to the server dropped.".to_owned()
    })
}

/// `Content-Range: bytes 100-299/300`: where the part starts and the
/// whole file's size (`*/300` for a range past the end: no start).
fn content_range(headers: &reqwest::header::HeaderMap) -> Option<(Option<u64>, Option<u64>)> {
    let value = headers.get(reqwest::header::CONTENT_RANGE)?.to_str().ok()?;
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let start = range.split_once('-').and_then(|(start, _)| start.trim().parse().ok());
    Some((start, total.trim().parse().ok()))
}

/// Fetches `url` into `part`, going on from what `part` has when the file
/// is still the one of `expected` bytes begun. `progress` hears what is on
/// the disk and the whole size as it comes, and stops it by answering
/// `false`.
async fn fetch(
    client: &reqwest::Client,
    url: &Url,
    cookie: Option<&str>,
    part: &Path,
    expected: Option<u64>,
    mut progress: impl FnMut(u64, Option<u64>) -> bool,
) -> Result<Fetched, Problem> {
    // A server that won't go on from the part (it changed) is asked once more for all of it.
    for _ in 0..2 {
        let have = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
        let mut request = client.get(url.clone());
        if let Some(cookie) = cookie {
            request = request.header(reqwest::header::COOKIE, cookie);
        }
        if have > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let mut response = tokio::time::timeout(ANSWER_TIMEOUT, request.send())
            .await
            .map_err(|_| Problem::Network("The server didn't answer.".to_owned()))?
            .map_err(network)?;
        let (append, size) = match response.status().as_u16() {
            206 => match content_range(response.headers()) {
                // The rest of the same file: one of another size changed on the server since.
                Some((Some(start), total))
                    if start == have && (expected.is_none() || total.is_none() || total == expected) =>
                {
                    (true, total)
                }
                _ => {
                    let _ = tokio::fs::remove_file(part).await;
                    continue;
                }
            },
            200 => (false, response.content_length()),
            416 => {
                // Asked from the end: the part may be the whole file already.
                if content_range(response.headers()).and_then(|(_, total)| total) == Some(have) {
                    return Ok(if progress(have, Some(have)) { Fetched::Whole } else { Fetched::Stopped });
                }
                let _ = tokio::fs::remove_file(part).await;
                continue;
            }
            401 | 403 => return Err(Problem::SignedOut),
            404 | 410 => return Err(Problem::Gone),
            _ => return Err(Problem::Network("The server couldn't send the file.".to_owned())),
        };
        if let Some(dir) = part.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(Problem::Disk)?;
        }
        let mut options = tokio::fs::OpenOptions::new();
        if append {
            options.append(true).create(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        let file = options.open(part).await.map_err(Problem::Disk)?;
        let mut file = tokio::io::BufWriter::with_capacity(WRITE_BUFFER, file);
        let mut received = if append { have } else { 0 };
        if !progress(received, size) {
            return Ok(Fetched::Stopped);
        }
        loop {
            let chunk = tokio::time::timeout(READ_TIMEOUT, response.chunk())
                .await
                .map_err(|_| Problem::Network("The server stopped sending.".to_owned()))?;
            // What came before the connection dropped stays, to go on from.
            let chunk = match chunk {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(err) => {
                    file.flush().await.map_err(Problem::Disk)?;
                    return Err(network(err));
                }
            };
            file.write_all(&chunk).await.map_err(Problem::Disk)?;
            received += chunk.len() as u64;
            if !progress(received, size) {
                return Ok(Fetched::Stopped);
            }
        }
        file.flush().await.map_err(Problem::Disk)?;
        file.get_ref().sync_all().await.map_err(Problem::Disk)?;
        if size.is_some_and(|size| received < size) {
            return Err(Problem::Network("The connection to the server dropped.".to_owned()));
        }
        return Ok(Fetched::Whole);
    }
    Err(Problem::Network("The server couldn't send the file.".to_owned()))
}

/// A download as the picker lists it.
#[derive(Debug, Serialize)]
pub struct Listed {
    pub id: u64,
    pub server: String,
    pub name: String,
    pub size: Option<u64>,
    pub received: u64,
    pub state: State,
    pub error: Option<String>,
}

#[tauri::command]
pub fn downloads(app: AppHandle) -> Vec<Listed> {
    app.state::<Downloads>()
        .list()
        .into_iter()
        // A file deleted outside the app is gone from the list.
        .filter(|d| d.state != State::Done || d.path.is_file())
        .map(|d| Listed {
            id: d.id,
            server: d.server,
            name: d.name,
            size: d.size,
            received: d.received,
            state: d.state,
            error: d.error,
        })
        .collect()
}

/// A download that is whole, as kept: an audio or video file in its folder.
fn kept(app: &AppHandle, id: u64) -> Result<PathBuf, String> {
    let d = app.state::<Downloads>().get(id).ok_or("This download is gone.")?;
    let name = d.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if d.state != State::Done || !d.path.is_file() || safe_name(&name).as_deref() != Some(name.as_str()) {
        return Err("This download isn't whole.".into());
    }
    Ok(d.path)
}

/// Plays a download in the system's player for its kind of file.
#[tauri::command]
pub fn play_download(app: AppHandle, id: u64) -> Result<(), String> {
    let path = kept(&app, id)?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

/// Shows a download in its folder.
#[tauri::command]
pub fn show_download(app: AppHandle, id: u64) -> Result<(), String> {
    let path = kept(&app, id)?;
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_download(app: AppHandle, id: u64) {
    app.state::<Downloads>().remove(id);
}

#[tauri::command]
pub fn retry_download(app: AppHandle, id: u64) {
    if let Some(d) = app.state::<Downloads>().retry(id) {
        announce(&app, &d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    #[test]
    fn only_audio_and_video_files_are_kept_under_safe_names() {
        assert_eq!(safe_name("Alien (1979).mkv").as_deref(), Some("Alien (1979).mkv"));
        assert_eq!(
            safe_name("The Expanse - S01E01 - Dulcinea.MP4").as_deref(),
            Some("The Expanse - S01E01 - Dulcinea.mp4")
        );
        assert_eq!(safe_name(r"..\..\Startup\x.mkv").as_deref(), Some("Startup x.mkv"), "no folders");
        assert_eq!(safe_name(".hidden.mkv").as_deref(), Some("hidden.mkv"));
        assert_eq!(safe_name("What? Why: Now.flac").as_deref(), Some("What Why Now.flac"));
        assert_eq!(safe_name("CON.mkv").as_deref(), Some("_CON.mkv"));
        assert_eq!(safe_name("com1.mp3").as_deref(), Some("_com1.mp3"));
        assert_eq!(safe_name("Command.mkv").as_deref(), Some("Command.mkv"));
        for refused in ["setup.exe", "film.mkv.lnk", "notes.txt", "x.scr", "mkv", ".mkv", "film."] {
            assert_eq!(safe_name(refused), None, "{refused}");
        }
        let long = format!("{}.mkv", "a".repeat(400));
        assert_eq!(safe_name(&long).map(|n| n.len()), Some(154));
    }

    #[test]
    fn only_the_original_of_a_file_is_one_to_keep() {
        let file = |u: &str| file_of(&Url::parse(u).unwrap());
        assert_eq!(file("http://nas:7539/api/files/42/stream"), Some(42));
        assert_eq!(file("http://nas:7539/api/files/42/stream?download=1"), Some(42));
        assert_eq!(file("http://nas:7539/api/files/42/subtitles/3.vtt"), None);
        assert_eq!(file("http://nas:7539/api/files/x/stream"), None);
        assert_eq!(file("http://nas:7539/api/files/0/stream"), None);
    }

    #[test]
    fn a_name_taken_gets_a_number() {
        let dir = tempdir("names");
        std::fs::write(dir.join("Alien (1979).mkv"), b"x").unwrap();
        std::fs::write(dir.join("Alien (1979) (2).mkv.part"), b"x").unwrap();
        let reserved = dir.join("Alien (1979) (3).mkv");
        assert_eq!(free_path(&dir, "Alien (1979).mkv", &[&reserved]), dir.join("Alien (1979) (4).mkv"));
        assert_eq!(free_path(&dir, "Heat (1995).mkv", &[]), dir.join("Heat (1995).mkv"));
    }

    #[test]
    fn the_list_comes_back_with_unfinished_downloads_measured_again() {
        let dir = tempdir("index");
        let downloads = Downloads::load(dir.join("downloads.json"), dir.clone());
        let a = downloads.request("http://nas:7539", 7, "Alien (1979).mkv", 1);
        let b = downloads.request("http://nas:7539", 8, "Heat (1995).mkv", 2);
        std::fs::write(part_of(&a.path), vec![0; 1234]).unwrap();
        downloads.update(a.id, |d| d.state = State::Downloading);
        downloads.update(b.id, |d| d.state = State::Done);

        let again = Downloads::load(dir.join("downloads.json"), dir.clone());
        let list = again.list();
        assert_eq!(list.len(), 1, "a file deleted meanwhile is forgotten");
        assert_eq!((list[0].state, list[0].received), (State::Queued, 1234));
        // Asked again, it is the same download, under the same name.
        let c = again.request("http://nas:7539", 7, "Alien (1979).mkv", 3);
        assert_eq!((c.id, c.path), (a.id, a.path));
        assert_eq!(again.request("http://nas:7539", 9, "Alien (1979).mkv", 4).name, "Alien (1979) (2).mkv");
    }

    #[test]
    fn content_ranges_are_read() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::CONTENT_RANGE, "bytes 100-299/300".parse().unwrap());
        assert_eq!(content_range(&headers), Some((Some(100), Some(300))));
        headers.insert(reqwest::header::CONTENT_RANGE, "bytes */300".parse().unwrap());
        assert_eq!(content_range(&headers), Some((None, Some(300))));
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pley-desktop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A server that answers each connection with the next of `answers`
    /// and tells what each request asked.
    async fn serve(answers: Vec<Vec<u8>>) -> (Url, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/api/files/7/stream", listener.local_addr().unwrap())).unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for answer in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                let _ = tx.send(String::from_utf8_lossy(&request).into_owned());
                // A client that stopped reading closes the connection: no matter.
                let _ = socket.write_all(&answer).await;
                let _ = socket.shutdown().await;
            }
        });
        (url, rx)
    }

    fn answer(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
        let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
        for header in headers {
            out.push_str(header);
            out.push_str("\r\n");
        }
        out.push_str("\r\n");
        let mut out = out.into_bytes();
        out.extend_from_slice(body);
        out
    }

    fn body() -> Vec<u8> {
        (0..300_000u32).map(|i| (i % 251) as u8).collect()
    }

    #[tokio::test]
    async fn a_file_comes_whole_with_the_session() {
        let body = body();
        let (url, mut asked) = serve(vec![answer("200 OK", &[], &body)]).await;
        let part = tempdir("whole").join("Alien (1979).mkv.part");
        let mut seen = 0;
        let fetched = fetch(&reqwest::Client::new(), &url, Some("pley_token=abc"), &part, None, |received, size| {
            assert_eq!(size, Some(300_000));
            seen = received;
            true
        })
        .await
        .unwrap();
        assert_eq!(fetched, Fetched::Whole);
        assert_eq!(seen, 300_000);
        assert_eq!(std::fs::read(&part).unwrap(), body);
        let request = asked.recv().await.unwrap().to_ascii_lowercase();
        assert!(request.contains("cookie: pley_token=abc"), "{request}");
        assert!(!request.contains("range:"), "{request}");
    }

    #[tokio::test]
    async fn a_file_cut_short_goes_on_from_where_it_stopped() {
        let body = body();
        let (url, mut asked) = serve(vec![answer(
            "206 Partial Content",
            &["Content-Range: bytes 100000-299999/300000"],
            &body[100_000..],
        )])
        .await;
        let part = tempdir("resume").join("Alien (1979).mkv.part");
        std::fs::write(&part, &body[..100_000]).unwrap();
        let fetched = fetch(&reqwest::Client::new(), &url, None, &part, None, |_, _| true).await.unwrap();
        assert_eq!(fetched, Fetched::Whole);
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert!(asked.recv().await.unwrap().to_ascii_lowercase().contains("range: bytes=100000-"));
    }

    #[tokio::test]
    async fn a_file_changed_on_the_server_since_it_began_comes_whole_again() {
        let body = body();
        // Replaced by a longer one: what is down belongs to the file before.
        let (url, mut asked) = serve(vec![
            answer("206 Partial Content", &["Content-Range: bytes 100000-399999/400000"], &[0; 300_000]),
            answer("200 OK", &[], &body),
        ])
        .await;
        let part = tempdir("changed").join("Alien (1979).mkv.part");
        std::fs::write(&part, vec![1; 100_000]).unwrap();
        let fetched = fetch(&reqwest::Client::new(), &url, None, &part, Some(300_000), |_, _| true).await.unwrap();
        assert_eq!(fetched, Fetched::Whole);
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert!(asked.recv().await.unwrap().to_ascii_lowercase().contains("range: bytes=100000-"));
        assert!(!asked.recv().await.unwrap().to_ascii_lowercase().contains("range:"));
    }

    #[tokio::test]
    async fn a_server_that_ignores_the_range_sends_it_all_again() {
        let body = body();
        let (url, _) = serve(vec![answer("200 OK", &[], &body)]).await;
        let part = tempdir("ignored").join("Alien (1979).mkv.part");
        std::fs::write(&part, vec![9; 50_000]).unwrap();
        fetch(&reqwest::Client::new(), &url, None, &part, None, |_, _| true).await.unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
    }

    #[tokio::test]
    async fn a_part_that_is_whole_already_is_done() {
        let (url, _) = serve(vec![answer("416 Range Not Satisfiable", &["Content-Range: bytes */5"], b"")]).await;
        let part = tempdir("whole-part").join("a.mkv.part");
        std::fs::write(&part, b"12345").unwrap();
        assert_eq!(fetch(&reqwest::Client::new(), &url, None, &part, None, |_, _| true).await.unwrap(), Fetched::Whole);
    }

    #[tokio::test]
    async fn failures_say_what_to_do() {
        let (url, _) = serve(vec![
            answer("404 Not Found", &[], b""),
            answer("401 Unauthorized", &[], b""),
            answer("503 Service Unavailable", &[], b""),
        ])
        .await;
        let part = tempdir("failures").join("a.mkv.part");
        let client = reqwest::Client::new();
        assert!(matches!(fetch(&client, &url, None, &part, None, |_, _| true).await, Err(Problem::Gone)));
        assert!(matches!(fetch(&client, &url, None, &part, None, |_, _| true).await, Err(Problem::SignedOut)));
        assert!(matches!(fetch(&client, &url, None, &part, None, |_, _| true).await, Err(Problem::Network(_))));
    }

    #[tokio::test]
    async fn a_download_removed_while_it_comes_stops() {
        let (url, _) = serve(vec![answer("200 OK", &[], &body())]).await;
        let part = tempdir("stopped").join("a.mkv.part");
        let fetched =
            fetch(&reqwest::Client::new(), &url, None, &part, None, |received, _| received < 1000).await.unwrap();
        assert_eq!(fetched, Fetched::Stopped);
    }
}
