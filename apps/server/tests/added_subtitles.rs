//! Subtitles people add to a title, uploaded from the web app: kept in the
//! data folder and listed as the file's own external tracks, to every
//! client.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::TestApp;

const SRT: &str = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n";

struct World {
    app: TestApp,
    admin: String,
    joker: i64,
    file: i64,
}

/// pley with a movie library with "Joker".
async fn world() -> World {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Joker (2019)/Joker (2019).mkv", 200_000);
    let library = app.library(&admin, "Movies", "movies", "movies").await;
    let joker = app.ids(library, &admin).await["Joker"];
    let file = app.file_of(joker).await;
    World { app, admin, joker, file }
}

/// A `multipart/form-data` body of these parts (name, file name, contents).
fn form(parts: &[(&str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "----pley-test-boundary";
    let mut body = Vec::new();
    for (name, filename, data) in parts {
        body.extend(format!("--{boundary}\r\n").bytes());
        match filename {
            Some(f) => body
                .extend(format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\n\r\n").bytes()),
            None => body.extend(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").bytes()),
        }
        body.extend_from_slice(data);
        body.extend(b"\r\n");
    }
    body.extend(format!("--{boundary}--\r\n").bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn upload(app: &TestApp, token: &str, item: i64, parts: &[(&str, Option<&str>, &[u8])]) -> common::Reply {
    let (content_type, body) = form(parts);
    app.send_body(Method::POST, &format!("/api/items/{item}/subtitles"), token, &content_type, body).await
}

/// The subtitle streams of a title's first file, as its page lists them.
async fn listed(app: &TestApp, token: &str, item: i64) -> Vec<Value> {
    let detail = app.get(&format!("/api/items/{item}"), token).await;
    let streams = detail.body["files"][0]["streams"].as_array().cloned().unwrap_or_default();
    streams.into_iter().filter(|s| s["kind"] == json!("subtitle")).collect()
}

#[tokio::test]
async fn uploaded_subtitles_are_kept_in_the_data_folder_and_listed_as_the_files_own() {
    let w = world().await;
    let (app, admin, joker, file) = (&w.app, w.admin.as_str(), w.joker, w.file);
    // UTF-16 with its byte order mark, named from a Windows path.
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("1\r\n00:00:01,000 --> 00:00:02,000\r\nCafé\r\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let added = upload(
        app,
        admin,
        joker,
        &[("file", Some("C:\\Subs\\Joker.Commentary.srt"), &utf16), ("language", None, b"fr")],
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.text());
    assert_eq!(
        added.body,
        json!({
            "index": 9000,
            "codec": "subrip",
            "language": "fre",
            "title": "Joker.Commentary",
            "default": false,
            "forced": false,
            "external": true,
            "text_based": true,
            "url": format!("/api/files/{file}/subtitles/9000.vtt"),
            "added": true,
            "deletable": true,
        })
    );
    // In the data folder, as UTF-8; nothing next to the film.
    let data = app.state.config.added_subtitles_dir().join(file.to_string());
    let kept: Vec<_> = std::fs::read_dir(&data).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(kept.len(), 1);
    assert!(std::fs::read_to_string(&kept[0]).unwrap().contains("Café"));
    let folder = app.media.path().join("movies/Joker (2019)");
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1, "only the film");

    // WebVTT named .srt, forced, for SDH: the next index.
    let vtt = b"WEBVTT\n\n00:01.000 --> 00:02.000\nHi\n";
    let second = upload(
        app,
        admin,
        joker,
        &[
            ("file", Some("signs.srt"), vtt),
            ("language", None, b"English"),
            ("forced", None, b"true"),
            ("hearing_impaired", None, b"1"),
        ],
    )
    .await;
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.text());
    assert_eq!(
        (&second.body["index"], &second.body["codec"], &second.body["forced"]),
        (&json!(9001), &json!("webvtt"), &json!(true))
    );
    let streams = listed(app, admin, joker).await;
    let indexes: Vec<&Value> = streams.iter().map(|s| &s["index"]).collect();
    assert_eq!(indexes, [&json!(9000), &json!(9001)]);
    assert!(streams.iter().all(|s| s["added"] == json!(true) && s["external"] == json!(true)));

    // Files that aren't subtitles pley takes, or that are too big, aren't.
    let bad = [
        (&[("file", Some("x.srt"), b"hello".as_slice()), ("language", None, b"en".as_slice())][..], "not subtitles"),
        (&[("file", Some("x.sub"), SRT.as_bytes()), ("language", None, b"en")][..], "not a format taken"),
        (&[("file", Some("x.srt"), SRT.as_bytes())][..], "no language"),
        (&[("file", Some("x.srt"), SRT.as_bytes()), ("language", None, b"not a language")][..], "a wrong language"),
        (&[("language", None, b"en".as_slice())][..], "no file"),
    ];
    for (parts, why) in bad {
        assert_eq!(upload(app, admin, joker, parts).await.status, StatusCode::BAD_REQUEST, "{why}");
    }
    let big = SRT.repeat(2 * 1024 * 1024 / SRT.len() + 1);
    let too_big =
        upload(app, admin, joker, &[("file", Some("big.srt"), big.as_bytes()), ("language", None, b"en")]).await;
    assert!(matches!(too_big.status, StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE), "{}", too_big.status);
    assert_eq!(listed(app, admin, joker).await.len(), 2);

    // The Plex-compatible API lists them and serves them as they are.
    let stream = app.plex(Method::GET, &format!("/library/streams/{}", file * 10_000 + 9000), admin, &[]).await;
    assert_eq!(stream.status, StatusCode::OK);
    assert!(stream.text().contains("Café"), "{}", stream.text());
    let meta = app.plex_json(&format!("/library/metadata/{joker}"), admin).await;
    let parts = meta.to_string();
    assert!(parts.contains(&format!("/library/streams/{}", file * 10_000 + 9001)), "{parts}");
}

#[tokio::test]
async fn whoever_added_subtitles_or_an_administrator_deletes_them() {
    let w = world().await;
    let (app, admin, joker) = (&w.app, w.admin.as_str(), w.joker);
    let (ana, _) = app.user(admin, "ana", json!({})).await;
    let (bo, _) = app.user(admin, "bo", json!({})).await;
    let theirs = |token: &str| {
        let token = token.to_owned();
        async move {
            upload(app, &token, joker, &[("file", Some("a.srt"), SRT.as_bytes()), ("language", None, b"en")]).await
        }
    };
    let by_ana = theirs(&ana).await;
    assert_eq!(by_ana.status, StatusCode::CREATED, "anyone who sees the title adds");
    let by_admin = theirs(admin).await;
    let (ana_index, admin_index) = (by_ana.body["index"].as_i64().unwrap(), by_admin.body["index"].as_i64().unwrap());

    // Each sees which they may delete when playing.
    let delete = |token: &str, index: i64| {
        let (uri, token) = (format!("/api/items/{joker}/subtitles/{index}"), token.to_owned());
        async move { app.delete(&uri, &token).await }
    };
    assert_eq!(delete(&bo, ana_index).await.status, StatusCode::FORBIDDEN);
    assert_eq!(delete(&ana, admin_index).await.status, StatusCode::FORBIDDEN);
    assert_eq!(delete(&ana, ana_index).await.status, StatusCode::NO_CONTENT);
    assert_eq!(delete(&ana, ana_index).await.status, StatusCode::NOT_FOUND, "gone");
    let by_bo = theirs(&bo).await;
    assert_eq!(by_bo.body["index"], json!(admin_index + 1), "indexes aren't given twice while the highest stays");
    assert_eq!(delete(admin, by_bo.body["index"].as_i64().unwrap()).await.status, StatusCode::NO_CONTENT);
    // Only added tracks: not the file's own, nor sidecars.
    assert_eq!(delete(admin, 2).await.status, StatusCode::NOT_FOUND);
    assert_eq!(delete(admin, 1000).await.status, StatusCode::NOT_FOUND);
    assert_eq!(listed(app, admin, joker).await.len(), 1);
    // Their files go with them.
    let data = app.state.config.added_subtitles_dir().join(w.file.to_string());
    assert_eq!(std::fs::read_dir(&data).unwrap().count(), 1);

    // A title someone can't see can't be given subtitles by them.
    let (kid, _) = app.user(admin, "kid", json!({"all_libraries": false, "library_ids": []})).await;
    let hidden = upload(app, &kid, joker, &[("file", Some("a.srt"), SRT.as_bytes()), ("language", None, b"en")]).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
}
