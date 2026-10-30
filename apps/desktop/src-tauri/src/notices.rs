//! The system's notifications for requests, while the window isn't in
//! front: a movie or show someone asked for is ready to watch, or was
//! approved or declined; for administrators, someone asked for one, or a
//! request couldn't be sent. The app hears of them on the event stream of
//! the server the window shows (`/api/events`), with the window's session,
//! and decides itself what to show: the server's pages get nothing from it.
//! While the window is in front, the page tells of them itself. After a
//! dropped stream (the computer slept, the network went), what changed for
//! the account meanwhile is told of in one notice.

use std::collections::HashSet;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;
use url::Url;

/// How long the server has to start answering.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);
/// The server sends something every 15 s, a comment at least: a stream
/// silent for this long has dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(45);
/// After a dropped stream, the app tries again after this, then twice as
/// long each time up to [`RETRY_MAX`].
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// An event longer than this isn't one the app knows: skipped.
const MAX_EVENT: usize = 64 * 1024;

/// The server the window shows, which the notices follow.
pub struct Notices(watch::Sender<Option<Url>>);

impl Default for Notices {
    fn default() -> Self {
        Self(watch::channel(None).0)
    }
}

impl Notices {
    /// The window shows `server` now (`None`: the picker).
    pub fn follow(&self, server: Option<Url>) {
        self.0.send_if_modified(|shown| {
            let changed = *shown != server;
            *shown = server;
            changed
        });
    }
}

/// Who asked, as requests name people.
#[derive(Debug, PartialEq, Deserialize)]
struct Person {
    name: String,
}

/// The events notices are made of (apps/server/src/events.rs); the others
/// are of no use here.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Heard {
    /// Something this account asked for, or follows, is ready to watch:
    /// all of it, or (`complete` false) the seasons of a show in `seasons`.
    RequestAvailable {
        request_id: i64,
        title: String,
        #[serde(default)]
        seasons: Option<Vec<i64>>,
        #[serde(default = "whole")]
        complete: bool,
    },
    /// An administrator approved or declined it (no reason for followers).
    RequestDecided {
        request_id: i64,
        title: String,
        status: String,
        #[serde(default)]
        reason: Option<String>,
    },
    /// Administrators: someone asked for a title.
    RequestCreated {
        title: String,
        #[serde(default)]
        seasons: Option<Vec<i64>>,
        requested_by: Person,
        /// "pending", or "approved" as it was made (nothing to decide then).
        #[serde(default)]
        status: Option<String>,
    },
    /// Administrators: a request couldn't be sent to Radarr or Sonarr.
    RequestFailed {
        title: String,
        kind: String,
        #[serde(default)]
        reason: Option<String>,
        /// When the server tries again by itself: no notice until it gives up.
        #[serde(default)]
        retry_at: Option<i64>,
    },
    #[serde(other)]
    Other,
}

fn whole() -> bool {
    true
}

fn heard(data: &str) -> Heard {
    serde_json::from_str(data).unwrap_or(Heard::Other)
}

/// `reason` as notices quote it: none when it's blank.
fn quoted(reason: Option<&str>) -> Option<&str> {
    reason.map(str::trim).filter(|r| !r.is_empty())
}

/// "“Heat” is ready to watch.", or for a show's first seasons, "Season 2 of
/// “The Expanse” is ready to watch.".
fn ready_words(title: &str, seasons: Option<&[i64]>) -> String {
    match seasons_label(seasons) {
        Some(label) if seasons.is_some_and(|s| s.len() > 1) => format!("{label} of “{title}” are ready to watch."),
        Some(label) => format!("{label} of “{title}” is ready to watch."),
        None => format!("“{title}” is ready to watch."),
    }
}

fn declined_words(title: &str, reason: Option<&str>) -> String {
    match quoted(reason) {
        Some(reason) => format!("“{title}” was declined: “{reason}”."),
        None => format!("“{title}” was declined."),
    }
}

/// What the notice for `heard` says, if there is one (the words every
/// client uses, docs/API.md "Requests").
fn notice(heard: &Heard) -> Option<String> {
    Some(match heard {
        Heard::RequestAvailable { title, seasons, complete, .. } => {
            ready_words(title, if *complete { None } else { seasons.as_deref() })
        }
        Heard::RequestDecided { title, status, reason, .. } => match status.as_str() {
            "approved" => format!("“{title}” was approved."),
            "declined" => declined_words(title, reason.as_deref()),
            _ => return None,
        },
        Heard::RequestCreated { title, seasons, requested_by, status } => {
            if status.as_deref().is_some_and(|s| s != "pending") {
                return None;
            }
            let who = &requested_by.name;
            match seasons_label(seasons.as_deref()) {
                Some(seasons) => format!("{who} asked for “{title}” ({seasons})."),
                None => format!("{who} asked for “{title}”."),
            }
        }
        Heard::RequestFailed { title, kind, reason, retry_at } => {
            if retry_at.is_some() {
                return None;
            }
            let service = if kind == "show" { "Sonarr" } else { "Radarr" };
            match quoted(reason.as_deref()) {
                Some(reason) => format!("“{title}” couldn't be sent to {service}: {reason}"),
                None => format!("“{title}” couldn't be sent to {service}."),
            }
        }
        Heard::Other => return None,
    })
}

/// A show's seasons as the web says them (`seasonsLabel` in
/// apps/web/src/pages/requests/shared.tsx): "Season 2", "Seasons 1–3, 5",
/// "Seasons 1–2 and specials", "Specials"; `None` for none (a movie).
fn seasons_label(seasons: Option<&[i64]>) -> Option<String> {
    let seasons = seasons.filter(|s| !s.is_empty())?;
    let mut numbers: Vec<i64> = seasons.iter().copied().filter(|&n| n > 0).collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let start = numbers[i];
        while i + 1 < numbers.len() && numbers[i + 1] == numbers[i] + 1 {
            i += 1;
        }
        let end = numbers[i];
        ranges.push(if start == end { start.to_string() } else { format!("{start}–{end}") });
        i += 1;
    }
    if ranges.is_empty() {
        return Some("Specials".to_owned());
    }
    let label = format!("{} {}", if numbers.len() == 1 { "Season" } else { "Seasons" }, ranges.join(", "));
    Some(if seasons.contains(&0) { format!("{label} and specials") } else { label })
}

/// Splits a `text/event-stream` into the data of its events.
#[derive(Debug, Default)]
struct EventStream {
    line: Vec<u8>,
    data: Option<String>,
    /// The event being read is too long: dropped at its end.
    too_long: bool,
}

impl EventStream {
    /// Reads `chunk`: the data of each event it ends.
    fn read(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut events = Vec::new();
        for &byte in chunk {
            if byte != b'\n' {
                if !self.too_long {
                    self.line.push(byte);
                    self.too_long = self.line.len() + self.data.as_ref().map_or(0, String::len) > MAX_EVENT;
                }
                continue;
            }
            let mut line = std::mem::take(&mut self.line);
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                // A blank line ends an event.
                let data = self.data.take();
                if !std::mem::take(&mut self.too_long) {
                    events.extend(data);
                }
                continue;
            }
            if self.too_long {
                continue;
            }
            // Only the data matters here: comments (":") keep the stream
            // alive, and events' names, ids and retry times aren't used.
            let line = String::from_utf8_lossy(&line);
            if let Some(value) = line.strip_prefix("data:") {
                let value = value.strip_prefix(' ').unwrap_or(value);
                match &mut self.data {
                    Some(data) => {
                        data.push('\n');
                        data.push_str(value);
                    }
                    None => self.data = Some(value.to_owned()),
                }
            }
        }
        events
    }
}

/// A request with news for the account (ready to watch, partly, or
/// declined since it last looked), as the server lists it.
#[derive(Debug, Deserialize)]
struct Listed {
    id: i64,
    title: String,
    stage: String,
    #[serde(default)]
    available_seasons: Vec<i64>,
    #[serde(default)]
    reason: Option<String>,
    updated_at: i64,
    /// Older servers have no such thing: then nothing is news.
    #[serde(default)]
    unseen: bool,
}

#[derive(Deserialize)]
struct Requests {
    requests: Vec<Listed>,
}

/// What the notices of one server have told of.
#[derive(Debug, Default)]
struct Told {
    /// The latest change among the requests with news, as listed when the
    /// stream opened; `None` until it first opens, which tells of nothing
    /// old (the page does, as it opens).
    newest: Option<i64>,
    /// The requests heard of as they changed: not told of twice.
    heard: HashSet<i64>,
}

impl Told {
    /// What `listed` (the account's requests with news, as listed when a
    /// stream opens) has that wasn't heard of: nothing the first time.
    fn catch_up<'a>(&mut self, listed: &'a [Listed]) -> Vec<&'a Listed> {
        let listed: Vec<&Listed> = listed.iter().filter(|r| r.unseen).collect();
        let latest = listed.iter().map(|r| r.updated_at).max();
        let Some(newest) = self.newest else {
            self.newest = Some(latest.unwrap_or(0));
            return Vec::new();
        };
        self.newest = Some(latest.map_or(newest, |latest| latest.max(newest)));
        let news: Vec<&Listed> =
            listed.into_iter().filter(|r| r.updated_at > newest && !self.heard.contains(&r.id)).collect();
        self.heard.extend(news.iter().map(|r| r.id));
        news
    }
}

/// The notice for what came while the stream was down: the request's own
/// words when there is one, else how many have news.
fn caught_up(news: &[&Listed]) -> Option<String> {
    match news {
        [] => None,
        [one] => match one.stage.as_str() {
            "available" => Some(ready_words(&one.title, None)),
            "partial" => Some(ready_words(&one.title, Some(&one.available_seasons))),
            "declined" => Some(declined_words(&one.title, one.reason.as_deref())),
            _ => None,
        },
        several => Some(format!("{} of your requests have news.", several.len())),
    }
}

/// What the notices need of the app: kept apart so that the tests run
/// without a window.
trait Shell: Send + Sync {
    /// Whether the window is in front: its page tells of what happens then.
    fn in_front(&self) -> bool;
    fn show(&self, title: &str, body: &str);
    /// The window's cookies for `server`: its session there.
    fn session(&self, server: &Url) -> Option<String>;
}

struct App<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Shell for App<R> {
    fn in_front(&self) -> bool {
        let Some(window) = self.0.get_webview_window("main") else { return false };
        window.is_visible().unwrap_or(false)
            && !window.is_minimized().unwrap_or(false)
            && window.is_focused().unwrap_or(false)
    }

    fn show(&self, title: &str, body: &str) {
        if let Err(err) = self.0.notification().builder().title(title).body(body).show() {
            eprintln!("cannot show a notification: {err}");
        }
    }

    fn session(&self, server: &Url) -> Option<String> {
        crate::downloads::cookies(&self.0, server)
    }
}

/// How a stream ended.
#[derive(Debug, PartialEq)]
enum Ended {
    /// The server can't be reached, or dropped the stream.
    Dropped,
    /// The window isn't signed in there (or the server has no events):
    /// tried again in a while, with the window's session then.
    Refused,
}

fn get(client: &reqwest::Client, url: Url, session: Option<&str>) -> reqwest::RequestBuilder {
    let request = client.get(url);
    match session {
        Some(cookie) => request.header(reqwest::header::COOKIE, cookie),
        None => request,
    }
}

/// The account's requests with news, latest first (`None`: the server
/// doesn't list them for it, requests being off for instance).
async fn listed_news(client: &reqwest::Client, server: &Url, session: Option<&str>) -> Option<Vec<Listed>> {
    let url = server.join("/api/requests?unseen=1&limit=20").ok()?;
    let response = tokio::time::timeout(ANSWER_TIMEOUT, get(client, url, session).send()).await.ok()?.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let list: Requests = tokio::time::timeout(ANSWER_TIMEOUT, response.json()).await.ok()?.ok()?;
    Some(list.requests)
}

/// Listens to `server`'s events once, telling of them, until the stream ends.
async fn listen(client: &reqwest::Client, server: &Url, shell: &dyn Shell, name: &str, told: &mut Told) -> Ended {
    let Ok(url) = server.join("/api/events") else { return Ended::Refused };
    let session = shell.session(server);
    let request = get(client, url, session.as_deref()).header(reqwest::header::ACCEPT, "text/event-stream");
    let mut response = match tokio::time::timeout(ANSWER_TIMEOUT, request.send()).await {
        Ok(Ok(response)) => response,
        _ => return Ended::Dropped,
    };
    match response.status().as_u16() {
        200 => {}
        401 | 403 | 404 => return Ended::Refused,
        _ => return Ended::Dropped,
    }
    // What came while the stream was down (nothing, the first time).
    let news = match listed_news(client, server, session.as_deref()).await {
        Some(listed) => caught_up(&told.catch_up(&listed)),
        None => None,
    };
    if let Some(body) = news.filter(|_| !shell.in_front()) {
        shell.show(name, &body);
    }
    let mut stream = EventStream::default();
    loop {
        let chunk = match tokio::time::timeout(READ_TIMEOUT, response.chunk()).await {
            Ok(Ok(Some(chunk))) => chunk,
            _ => return Ended::Dropped,
        };
        for data in stream.read(&chunk) {
            let heard = heard(&data);
            if let Heard::RequestAvailable { request_id, .. } | Heard::RequestDecided { request_id, .. } = &heard {
                told.heard.insert(*request_id);
            }
            match notice(&heard) {
                Some(body) if !shell.in_front() => shell.show(name, &body),
                _ => {}
            }
        }
    }
}

/// Follows `server`'s events for as long as it is shown (the caller drops
/// this when the window moves on), and tries again after a dropped stream.
async fn follow(client: &reqwest::Client, server: &Url, shell: &dyn Shell, name: &str) {
    let mut told = Told::default();
    let mut retry = RETRY_FIRST;
    loop {
        let opened = std::time::Instant::now();
        let ended = listen(client, server, shell, name, &mut told).await;
        // A stream that stayed open a while starts the pace again.
        if ended == Ended::Dropped && opened.elapsed() > RETRY_MAX {
            retry = RETRY_FIRST;
        }
        tokio::time::sleep(retry).await;
        retry = (retry * 2).min(RETRY_MAX);
    }
}

/// What notices from `server` are titled: the name it was saved with, else
/// its address.
fn title_for(saved: &[crate::servers::SavedServer], server: &Url) -> String {
    let address = server.as_str().trim_end_matches('/');
    saved.iter().find(|s| s.url == address).map(|s| s.name.clone()).unwrap_or_else(|| match server.port() {
        Some(port) => format!("{}:{port}", server.host_str().unwrap_or(address)),
        None => server.host_str().unwrap_or(address).to_owned(),
    })
}

/// Shows the notices of the server the window shows, for as long as the app runs.
pub async fn run(app: AppHandle) {
    let client = match reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).build() {
        Ok(client) => client,
        Err(err) => {
            eprintln!("notifications are off: {err}");
            return;
        }
    };
    let mut shown = app.state::<Notices>().0.subscribe();
    let shell = App(app.clone());
    loop {
        let server = shown.borrow_and_update().clone();
        if let Some(server) = server {
            let name = title_for(&crate::servers::load(&app), &server);
            tokio::select! {
                changed = shown.changed() => if changed.is_err() { return },
                () = follow(&client, &server, &shell, &name) => {}
            }
        } else if shown.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn events_are_read_across_chunks_and_line_endings() {
        let mut stream = EventStream::default();
        assert!(stream.read(b": keep-alive\n\nda").is_empty());
        assert!(stream.read(b"ta: {\"type\":\"request_av").is_empty());
        assert_eq!(
            stream.read(b"ailable\"}\r\n\r\ndata: one\ndata: two\n\n"),
            [r#"{"type":"request_available"}"#, "one\ntwo"]
        );
        assert_eq!(stream.read(b"event: x\nid: 4\ndata:no space\n\n\n\n"), ["no space"]);
        // An event too long is dropped whole; the next is read.
        let mut long = b"data: ".to_vec();
        long.extend(std::iter::repeat_n(b'x', MAX_EVENT + 10));
        long.extend_from_slice(b"\ndata: more\n\ndata: next\n\n");
        assert_eq!(stream.read(&long), ["next"]);
    }

    #[test]
    fn the_events_notices_come_from_are_read_as_the_server_sends_them() {
        assert_eq!(
            heard(
                r#"{"type":"request_available","request_id":2,"title":"Heat","kind":"movie","tmdb_id":949,"item_id":9,"poster_url":null,"seasons":null,"complete":true}"#
            ),
            Heard::RequestAvailable { request_id: 2, title: "Heat".into(), seasons: None, complete: true }
        );
        assert_eq!(
            heard(r#"{"type":"request_available","request_id":2,"title":"Heat","item_id":9}"#),
            Heard::RequestAvailable { request_id: 2, title: "Heat".into(), seasons: None, complete: true },
            "a server that sends less"
        );
        assert_eq!(
            heard(
                r##"{"type":"request_created","request_id":3,"title":"The Expanse","kind":"show","tmdb_id":63639,"seasons":[1,2,3],"requested_by":{"id":4,"name":"Sam","avatar_url":null,"avatar_color":"#5b8def"},"status":"pending"}"##
            ),
            Heard::RequestCreated {
                title: "The Expanse".into(),
                seasons: Some(vec![1, 2, 3]),
                requested_by: Person { name: "Sam".into() },
                status: Some("pending".into())
            }
        );
        assert_eq!(heard(r#"{"type":"requests_changed","request_id":null}"#), Heard::Other);
        assert_eq!(heard(r#"{"type":"library_scan","library_id":1}"#), Heard::Other);
        assert_eq!(heard("not json"), Heard::Other);
        assert_eq!(heard(r#"{"type":"request_available"}"#), Heard::Other, "missing fields");
    }

    #[test]
    fn notices_use_the_words_every_client_uses() {
        let say = |json: &str| notice(&heard(json));
        assert_eq!(
            say(r#"{"type":"request_available","request_id":2,"title":"Heat","item_id":null}"#).as_deref(),
            Some("“Heat” is ready to watch.")
        );
        assert_eq!(
            say(r#"{"type":"request_available","request_id":2,"title":"The Expanse","seasons":[2],"complete":false}"#)
                .as_deref(),
            Some("Season 2 of “The Expanse” is ready to watch.")
        );
        assert_eq!(
            say(
                r#"{"type":"request_available","request_id":2,"title":"The Expanse","seasons":[1,2],"complete":false}"#
            )
            .as_deref(),
            Some("Seasons 1–2 of “The Expanse” are ready to watch.")
        );
        assert_eq!(
            say(
                r#"{"type":"request_available","request_id":2,"title":"The Expanse","seasons":[1,2,3],"complete":true}"#
            )
            .as_deref(),
            Some("“The Expanse” is ready to watch."),
            "complete: the whole request"
        );
        assert_eq!(
            say(r#"{"type":"request_decided","request_id":2,"title":"Heat","status":"approved","reason":null}"#)
                .as_deref(),
            Some("“Heat” was approved.")
        );
        assert_eq!(
            say(r#"{"type":"request_decided","request_id":2,"title":"Heat","status":"declined","reason":" Not in English "}"#)
                .as_deref(),
            Some("“Heat” was declined: “Not in English”.")
        );
        assert_eq!(
            say(r#"{"type":"request_decided","request_id":2,"title":"Heat","status":"declined","reason":""}"#)
                .as_deref(),
            Some("“Heat” was declined.")
        );
        assert_eq!(say(r#"{"type":"request_decided","request_id":2,"title":"Heat","status":"weird"}"#), None);
        let sam = r#""requested_by":{"id":4,"name":"Sam","avatar_url":null,"avatar_color":null}"#;
        assert_eq!(
            say(&format!(
                r#"{{"type":"request_created","request_id":3,"title":"Heat","seasons":null,{sam},"status":"pending"}}"#
            ))
            .as_deref(),
            Some("Sam asked for “Heat”.")
        );
        assert_eq!(
            say(&format!(
                r#"{{"type":"request_created","request_id":3,"title":"The Expanse","seasons":[5,1,2,3,0],{sam}}}"#
            ))
            .as_deref(),
            Some("Sam asked for “The Expanse” (Seasons 1–3, 5 and specials).")
        );
        assert_eq!(
            say(&format!(
                r#"{{"type":"request_created","request_id":3,"title":"Heat","seasons":null,{sam},"status":"approved"}}"#
            )),
            None,
            "approved as made: nothing to decide"
        );
        assert_eq!(
            say(r#"{"type":"request_failed","request_id":3,"title":"Heat","kind":"movie","reason":"HTTP 500","retry_at":null}"#)
                .as_deref(),
            Some("“Heat” couldn't be sent to Radarr: HTTP 500")
        );
        assert_eq!(
            say(r#"{"type":"request_failed","request_id":3,"title":"Ran","kind":"show","reason":null}"#).as_deref(),
            Some("“Ran” couldn't be sent to Sonarr.")
        );
        assert_eq!(
            say(
                r#"{"type":"request_failed","request_id":3,"title":"Heat","kind":"movie","reason":"HTTP 503","retry_at":1798000000}"#
            ),
            None,
            "the server tries again by itself"
        );
        assert_eq!(say(r#"{"type":"requests_changed","request_id":3,"kind":"movie","tmdb_id":949}"#), None);
    }

    #[test]
    fn seasons_read_as_on_the_web() {
        let label = |s: &[i64]| seasons_label(Some(s));
        assert_eq!(label(&[2]).as_deref(), Some("Season 2"));
        assert_eq!(label(&[3, 1, 2, 5]).as_deref(), Some("Seasons 1–3, 5"));
        assert_eq!(label(&[0, 1, 2]).as_deref(), Some("Seasons 1–2 and specials"));
        assert_eq!(label(&[0]).as_deref(), Some("Specials"));
        assert_eq!(label(&[4, 4]).as_deref(), Some("Season 4"));
        assert_eq!(label(&[]), None);
        assert_eq!(seasons_label(None), None);
    }

    fn news(id: i64, title: &str, stage: &str, updated_at: i64) -> Listed {
        Listed {
            id,
            title: title.into(),
            stage: stage.into(),
            available_seasons: Vec::new(),
            reason: None,
            updated_at,
            unseen: true,
        }
    }

    fn titles(news: &[&Listed]) -> Vec<String> {
        news.iter().map(|r| r.title.clone()).collect()
    }

    #[test]
    fn catching_up_tells_only_of_what_came_since() {
        let mut told = Told::default();
        // The first time, nothing old is told of: the page does, as it opens.
        assert!(told.catch_up(&[news(1, "Heat", "available", 100), news(2, "Ran", "declined", 50)]).is_empty());
        assert!(told.catch_up(&[news(1, "Heat", "available", 100)]).is_empty());
        // One heard of as it changed isn't told of again; one seen isn't news.
        told.heard.insert(4);
        let seen = Listed { unseen: false, ..news(6, "Big", "available", 180) };
        let listed = [
            news(5, "Up", "available", 170),
            news(4, "Ronin", "available", 160),
            news(3, "Alien", "declined", 150),
            seen,
            news(1, "Heat", "available", 100),
        ];
        assert_eq!(titles(&told.catch_up(&listed)), ["Up", "Alien"]);
        assert!(told.catch_up(&listed).is_empty());
        let mut empty = Told::default();
        assert!(empty.catch_up(&[]).is_empty());
        assert_eq!(titles(&empty.catch_up(&[news(9, "Big", "partial", 1)])), ["Big"]);
    }

    #[test]
    fn what_came_meanwhile_is_one_notice() {
        let heat = news(1, "Heat", "available", 100);
        assert_eq!(caught_up(&[&heat]).as_deref(), Some("“Heat” is ready to watch."));
        let expanse = Listed { available_seasons: vec![1, 2], ..news(2, "The Expanse", "partial", 100) };
        assert_eq!(caught_up(&[&expanse]).as_deref(), Some("Seasons 1–2 of “The Expanse” are ready to watch."));
        let ran = Listed { reason: Some("Not in English".into()), ..news(3, "Ran", "declined", 100) };
        assert_eq!(caught_up(&[&ran]).as_deref(), Some("“Ran” was declined: “Not in English”."));
        assert_eq!(caught_up(&[&heat, &expanse, &ran]).as_deref(), Some("3 of your requests have news."));
        assert_eq!(caught_up(&[]), None);
        assert_eq!(caught_up(&[&news(4, "Up", "searching", 100)]), None);
    }

    #[test]
    fn notices_are_titled_with_the_servers_name() {
        let saved = [crate::servers::SavedServer {
            url: "http://192.168.1.20:7539".into(),
            name: "Den".into(),
            id: None,
            last_used: 0,
        }];
        let at = |s: &str| title_for(&saved, &Url::parse(s).unwrap());
        assert_eq!(at("http://192.168.1.20:7539/"), "Den");
        assert_eq!(at("http://192.168.1.21:7539/"), "192.168.1.21:7539");
        assert_eq!(at("https://pley.example.com/"), "pley.example.com");
    }

    /// The window, as the tests see it.
    #[derive(Default)]
    struct Window {
        in_front: Mutex<bool>,
        shown: Mutex<Vec<(String, String)>>,
    }

    impl Shell for Window {
        fn in_front(&self) -> bool {
            *self.in_front.lock().unwrap()
        }

        fn show(&self, title: &str, body: &str) {
            self.shown.lock().unwrap().push((title.to_owned(), body.to_owned()));
        }

        fn session(&self, _server: &Url) -> Option<String> {
            Some("pley_token=abc".to_owned())
        }
    }

    /// A server that answers each connection with the next of `answers`
    /// (written in pieces, a stream's events one by one), and tells what
    /// each request asked.
    async fn serve(answers: Vec<Vec<Vec<u8>>>) -> (Url, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for pieces in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                let _ = tx.send(String::from_utf8_lossy(&request).into_owned());
                for piece in pieces {
                    let _ = socket.write_all(&piece).await;
                    let _ = socket.flush().await;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                let _ = socket.shutdown().await;
            }
        });
        (url, rx)
    }

    fn whole(status: &str, body: &str) -> Vec<Vec<u8>> {
        vec![
            format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .into_bytes(),
        ]
    }

    fn events(events: &[&str]) -> Vec<Vec<u8>> {
        let mut pieces =
            vec![b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n".to_vec()];
        pieces.extend(events.iter().map(|e| e.as_bytes().to_vec()));
        pieces
    }

    #[tokio::test]
    async fn the_stream_is_heard_with_the_windows_session_and_heard_again_after_it_drops() {
        let list =
            |items: &str| whole("200 OK", &format!(r#"{{"total":0,"counts":{{}},"requests":[{items}],"people":[]}}"#));
        let ran = r#"{"id":1,"title":"Ran","stage":"available","available_seasons":[],"updated_at":100,"unseen":true}"#;
        let heat =
            r#"{"id":2,"title":"Heat","stage":"available","available_seasons":[],"updated_at":140,"unseen":true}"#;
        let alien =
            r#"{"id":3,"title":"Alien","stage":"available","available_seasons":[],"updated_at":150,"unseen":true}"#;
        let (server, mut asked) = serve(vec![
            events(&[
                ": hello\n\n",
                "data: {\"type\":\"request_available\",\"request_id\":2,\"title\":\"Heat\",\"item_id\":9}\n\n",
            ]),
            list(ran),
            // Dropped: the second stream catches up.
            events(&["data: {\"type\":\"request_created\",\"request_id\":5,\"title\":\"Up\",\"seasons\":null,\
                 \"requested_by\":{\"id\":4,\"name\":\"Sam\"},\"status\":\"pending\"}\n\n"]),
            list(&[alien, heat, ran].join(",")),
        ])
        .await;
        let window = Window::default();
        let client = reqwest::Client::new();
        let mut told = Told::default();
        assert_eq!(listen(&client, &server, &window, "Den", &mut told).await, Ended::Dropped);
        assert_eq!(listen(&client, &server, &window, "Den", &mut told).await, Ended::Dropped);
        let first = asked.recv().await.unwrap().to_ascii_lowercase();
        assert!(first.starts_with("get /api/events "), "{first}");
        assert!(first.contains("cookie: pley_token=abc\r\n"), "{first}");
        assert!(first.contains("accept: text/event-stream\r\n"), "{first}");
        let listing = asked.recv().await.unwrap().to_ascii_lowercase();
        assert!(listing.starts_with("get /api/requests?unseen=1&limit=20 "), "{listing}");
        assert!(listing.contains("cookie: pley_token=abc\r\n"), "{listing}");
        let shown: Vec<String> =
            window.shown.lock().unwrap().iter().map(|(title, body)| format!("{title}: {body}")).collect();
        assert_eq!(
            shown,
            [
                "Den: “Heat” is ready to watch.",
                // The second stream first catches up: Heat was heard of as it
                // came, so only Alien is news.
                "Den: “Alien” is ready to watch.",
                "Den: Sam asked for “Up”.",
            ]
        );
    }

    #[tokio::test]
    async fn nothing_is_shown_while_the_window_is_in_front_and_a_refusal_waits() {
        let (server, _asked) = serve(vec![
            events(&["data: {\"type\":\"request_available\",\"request_id\":2,\"title\":\"Heat\",\"item_id\":9}\n\n"]),
            whole("503 Service Unavailable", r#"{"code":"unavailable","message":"requests are off"}"#),
            whole("401 Unauthorized", r#"{"code":"unauthorized","message":"sign in"}"#),
        ])
        .await;
        let window = Window::default();
        *window.in_front.lock().unwrap() = true;
        let client = reqwest::Client::new();
        let mut told = Told::default();
        assert_eq!(listen(&client, &server, &window, "Den", &mut told).await, Ended::Dropped);
        assert!(window.shown.lock().unwrap().is_empty());
        assert_eq!(listen(&client, &server, &window, "Den", &mut told).await, Ended::Refused);
    }
}
