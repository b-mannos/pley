//! Libraries and items through pley's API (docs/API.md: Libraries, Items,
//! Watch history, System): what administrators may change and how it is
//! checked, folders that go offline, item endpoints that refuse what makes
//! no sense, metadata edits, hubs and history filters.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{TestApp, titles};

#[tokio::test]
async fn libraries_are_checked_when_created_and_changed() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    std::fs::create_dir_all(app.media.path().join("films")).unwrap();
    app.media_file("not-a-folder.txt", 10);
    let films = app.media_path("films");

    let create = |body: Value| app.post("/api/libraries", &admin, body);
    for body in [
        json!({"name": " ", "kind": "movies", "paths": [films]}),
        json!({"name": "x".repeat(101), "kind": "movies", "paths": [films]}),
        json!({"name": "Films", "kind": "movies", "paths": []}),
        json!({"name": "Films", "kind": "movies", "paths": ["  "]}),
        json!({"name": "Films", "kind": "movies", "paths": ["films"]}),
        json!({"name": "Films", "kind": "movies", "paths": [app.media_path("missing")]}),
        json!({"name": "Films", "kind": "movies", "paths": [app.media_path("not-a-folder.txt")]}),
    ] {
        let reply = create(body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}: {:?}", reply.body);
    }
    let unknown_kind = create(json!({"name": "Films", "kind": "photos", "paths": [films]})).await;
    assert!(unknown_kind.status.is_client_error());

    // The same folder twice (once with a trailing separator) is kept once.
    let twice = format!("{films}{}", std::path::MAIN_SEPARATOR);
    let created = create(json!({"name": "  Films  ", "kind": "movies", "paths": [films, twice]})).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!((created.body["name"].clone(), created.body["paths"].clone()), (json!("Films"), json!([films])));
    let id = created.body["id"].as_i64().unwrap();
    app.wait_for_scans().await;

    let rename = app.patch(&format!("/api/libraries/{id}"), &admin, json!({"name": ""})).await;
    assert_eq!(rename.status, StatusCode::BAD_REQUEST);
    let rename = app.patch(&format!("/api/libraries/{id}"), &admin, json!({"name": "Cinema"})).await;
    assert_eq!(rename.body["name"], json!("Cinema"));
    assert_eq!(app.patch("/api/libraries/999", &admin, json!({"name": "x"})).await.status, StatusCode::NOT_FOUND);
    let sorted = app.get(&format!("/api/libraries/{id}/items?sort=random"), &admin).await;
    assert_eq!(sorted.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get("/api/libraries/999", &admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.post("/api/libraries/999/scan", &admin, json!({})).await.status, StatusCode::NOT_FOUND);

    // Only administrators manage libraries.
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    assert_eq!(
        app.post("/api/libraries", &bob, json!({"name": "Mine", "kind": "movies", "paths": [films]})).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.patch(&format!("/api/libraries/{id}"), &bob, json!({"name": "Mine"})).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.post(&format!("/api/libraries/{id}/scan"), &bob, json!({})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post("/api/libraries/scan", &bob, json!({})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.delete(&format!("/api/libraries/{id}"), &bob).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get(&format!("/api/libraries/{id}"), &bob).await.status, StatusCode::OK);
}

#[tokio::test]
async fn moving_and_deleting_a_library() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("old/Alien (1979).mkv", 1024);
    app.media_file("new/Brazil (1985).mkv", 1024);
    let lib = app.library(&admin, "Movies", "movies", "old").await;
    let alien = app.ids(lib, &admin).await["Alien"];

    // New folders: their films come in, the old folder's go.
    let moved = app.patch(&format!("/api/libraries/{lib}"), &admin, json!({"paths": [app.media_path("new")]})).await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.body);
    app.wait_for_scans().await;
    assert_eq!(titles(&app.get(&format!("/api/libraries/{lib}/items"), &admin).await.body["items"]), ["Brazil"]);
    assert_eq!(app.get(&format!("/api/items/{alien}"), &admin).await.status, StatusCode::NOT_FOUND);

    let brazil = app.ids(lib, &admin).await["Brazil"];
    assert_eq!(app.delete(&format!("/api/libraries/{lib}"), &admin).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&format!("/api/libraries/{lib}"), &admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&format!("/api/items/{brazil}"), &admin).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get("/api/libraries", &admin).await.body, json!([]));
    assert_eq!(app.delete(&format!("/api/libraries/{lib}"), &admin).await.status, StatusCode::NOT_FOUND);
    // The file on disk is never touched.
    assert!(app.media.path().join("new/Brazil (1985).mkv").is_file());
}

#[tokio::test]
async fn folders_that_go_offline_keep_their_items() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("nas/Alien (1979).mkv", 1024);
    app.media_file("usb/Brazil (1985).mkv", 1024);
    std::fs::create_dir_all(app.media.path().join("local")).unwrap();
    app.media_file("local/Casablanca (1942).mkv", 1024);
    let paths = [app.media_path("nas"), app.media_path("usb"), app.media_path("local")];
    let created = app.post("/api/libraries", &admin, json!({"name": "Movies", "kind": "movies", "paths": paths})).await;
    let lib = created.body["id"].as_i64().unwrap();
    app.wait_for_scans().await;
    let rescan = || async {
        let reply = app.post(&format!("/api/libraries/{lib}/scan"), &admin, json!({})).await;
        assert_eq!(reply.status, StatusCode::ACCEPTED);
        app.wait_for_scans().await;
        titles(&app.get(&format!("/api/libraries/{lib}/items"), &admin).await.body["items"])
    };
    assert_eq!(rescan().await, ["Alien", "Brazil", "Casablanca"]);

    // A share that is gone, a drive whose mount point is left empty: their
    // films are kept. A file deleted from a folder that is there is gone.
    std::fs::remove_dir_all(app.media.path().join("nas")).unwrap();
    std::fs::remove_file(app.media.path().join("usb/Brazil (1985).mkv")).unwrap();
    assert_eq!(rescan().await, ["Alien", "Brazil", "Casablanca"]);
    app.media_file("local/Dune (2021).mkv", 1024);
    std::fs::remove_file(app.media.path().join("local/Casablanca (1942).mkv")).unwrap();
    assert_eq!(rescan().await, ["Alien", "Brazil", "Dune"]);
}

#[tokio::test]
async fn item_endpoints_refuse_what_makes_no_sense() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Alien (1979).mkv", 1024);
    app.media_file("movies/Brazil (1985).mkv", 1024);
    app.media_file("tv/Show (2015)/Season 01/Show - S01E01 - Pilot.mkv", 1024);
    app.media_file("music/Band/Record (2001)/01 - Song.mp3", 512);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let tv = app.library(&admin, "TV", "shows", "tv").await;
    let music = app.library(&admin, "Music", "music", "music").await;
    let m = app.ids(movies, &admin).await;
    let (alien, brazil) = (m["Alien"], m["Brazil"]);
    let show = app.ids(tv, &admin).await["Show"];
    let season = app.children(show, &admin).await["Season 1"];
    let pilot = app.children(season, &admin).await["Pilot"];
    let band = app.ids(music, &admin).await["Band"];
    let record = app.children(band, &admin).await["Record"];

    // Progress is for things that play, in known states.
    let progress = |id: i64, body: Value| {
        let (app, admin) = (&app, admin.clone());
        async move { app.post(&format!("/api/items/{id}/progress"), &admin, body).await }
    };
    assert_eq!(progress(show, json!({"position_ms": 1_000})).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        progress(alien, json!({"position_ms": 1_000, "state": "rewinding"})).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        progress(alien, json!({"position_ms": 1_000, "state": "buffering"})).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(progress(9_999, json!({"position_ms": 1_000})).await.status, StatusCode::NOT_FOUND);

    // Track choices name a file of the item.
    let other_file = app.file_of(brazil).await;
    let streams = app
        .put(&format!("/api/items/{alien}/streams"), &admin, json!({"file_id": other_file, "subtitle_index": null}))
        .await;
    assert_eq!(streams.status, StatusCode::NOT_FOUND);
    let streams = app.put(&format!("/api/items/{show}/streams"), &admin, json!({"subtitle_index": null})).await;
    assert_eq!(streams.status, StatusCode::NOT_FOUND, "a show has no file of its own");

    // Ratings are 0-10.
    for rating in [json!(-1), json!(10.5), json!("good")] {
        let reply = app.put(&format!("/api/items/{alien}/rating"), &admin, json!({"rating": rating})).await;
        assert!(reply.status.is_client_error(), "{rating}");
    }

    // Playback only plays movies, episodes and tracks.
    let caps = json!({"containers": ["mkv"], "video_codecs": [], "audio_codecs": []});
    let play = app.post("/api/playback", &admin, json!({"item_id": show, "caps": caps})).await;
    assert_eq!(play.status, StatusCode::BAD_REQUEST);
    let play = app.post("/api/playback", &admin, json!({"item_id": alien, "file_id": other_file, "caps": caps})).await;
    assert_eq!(play.status, StatusCode::NOT_FOUND);

    // "More like this" for movies and shows only, within limits.
    assert_eq!(app.get(&format!("/api/items/{pilot}/related"), &admin).await.body["items"], json!([]));
    let related = app.get(&format!("/api/items/{alien}/related?limit=0"), &admin).await;
    assert_eq!(related.status, StatusCode::OK);
    assert!(related.body["items"].as_array().unwrap().len() <= 1);

    // On deck and next only make sense in shows (and albums for next).
    assert_eq!(app.get(&format!("/api/items/{alien}/on-deck"), &admin).await.body["item"], Value::Null);
    assert_eq!(app.get(&format!("/api/items/{season}/on-deck"), &admin).await.body["item"]["title"], json!("Pilot"));
    assert_eq!(app.get(&format!("/api/items/{alien}/next"), &admin).await.body["item"], Value::Null);
    assert_eq!(
        app.get(&format!("/api/items/{pilot}/next"), &admin).await.body["item"],
        Value::Null,
        "the last episode"
    );

    // Refreshing needs a metadata source; analysis needs video.
    for (id, source) in [(alien, "TMDB"), (show, "TV source"), (record, "MusicBrainz")] {
        let reply = app.post(&format!("/api/items/{id}/refresh"), &admin, json!({})).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert!(reply.body["message"].as_str().unwrap().contains(source), "{}", reply.body);
    }
    assert_eq!(
        app.post(&format!("/api/items/{record}/analyze"), &admin, json!({})).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(app.post(&format!("/api/items/{season}/analyze"), &admin, json!({})).await.status, StatusCode::ACCEPTED);

    // Pictures: known kinds only; no transcoder, no frame grabs or covers.
    for (id, kind) in [(alien, "logo"), (alien, "thumb"), (record, "poster"), (band, "poster")] {
        let reply = app.get(&format!("/api/items/{id}/images/{kind}"), &admin).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{kind} of {id}");
    }
}

#[tokio::test]
async fn artwork_falls_back_to_the_show_and_is_revalidated() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let mut png = Vec::new();
    image::RgbImage::from_pixel(20, 30, image::Rgb([1, 2, 3]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    app.media_file("tv/Show (2015)/Season 01/Show - S01E01 - Pilot.mkv", 1024);
    std::fs::write(app.media.path().join("tv/Show (2015)/poster.png"), &png).unwrap();
    std::fs::write(app.media.path().join("tv/Show (2015)/fanart.png"), &png).unwrap();
    let tv = app.library(&admin, "TV", "shows", "tv").await;
    let show = app.ids(tv, &admin).await["Show"];
    let season = app.children(show, &admin).await["Season 1"];
    let pilot = app.children(season, &admin).await["Pilot"];

    for (id, kind) in [(season, "poster"), (season, "backdrop"), (pilot, "backdrop")] {
        let reply = app.get(&format!("/api/items/{id}/images/{kind}"), &admin).await;
        assert_eq!(reply.status, StatusCode::OK, "{kind} of {id}");
        assert_eq!(reply.headers["content-type"], "image/png");
        assert_eq!(reply.headers["cache-control"], "private, max-age=300", "unversioned URLs are revalidated");
    }
    let summary = &app.get(&format!("/api/items/{season}/children"), &admin).await.body["items"][0];
    assert!(summary["poster_url"].as_str().unwrap().starts_with(&format!("/api/items/{show}/images/poster?v=")));

    // ETags make a second fetch free.
    let first = app.get(&format!("/api/items/{show}/images/poster?w=10"), &admin).await;
    let etag = first.headers["etag"].to_str().unwrap().to_owned();
    let again = app
        .call(
            Method::GET,
            &format!("/api/items/{show}/images/poster?w=10"),
            Some(&admin),
            None,
            &[("if-none-match", &etag)],
        )
        .await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn metadata_edits_are_checked_and_cleaned() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Alien (1979).mkv", 1024);
    app.media_file("movies/Brazil (1985).mkv", 1024);
    let lib = app.library(&admin, "Movies", "movies", "movies").await;
    let alien = app.ids(lib, &admin).await["Alien"];
    let item_uri = format!("/api/items/{alien}");
    let edit = |body: Value| app.patch(&item_uri, &admin, body);

    for body in [
        json!({"year": 1799}),
        json!({"year": "soon"}),
        json!({"title": "  "}),
        json!({"release_date": "1979/05/25"}),
        json!({"genres": "Horror"}),
        json!({"summary": true}),
        json!({"tagline": {"text": "x"}}),
        json!({"poster": "/etc/passwd"}),
        json!({"title": "Fine", "year": 3000}),
    ] {
        let reply = edit(body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
    }
    let unchanged = app.get(&format!("/api/items/{alien}"), &admin).await;
    assert_eq!((unchanged.body["title"].clone(), unchanged.body["locked_fields"].clone()), (json!("Alien"), json!([])));

    // Values are trimmed, genre lists cleaned, the sort title follows the title.
    let edited = edit(json!({"title": "  The Alien  ", "year": 1979, "genres": [" Horror ", "", 5, "Sci-Fi"]})).await;
    assert_eq!(edited.status, StatusCode::OK, "{:?}", edited.body);
    assert_eq!((edited.body["title"].clone(), edited.body["sort_title"].clone()), (json!("The Alien"), json!("alien")));
    assert_eq!(edited.body["genres"], json!(["Horror", "Sci-Fi"]));
    let page = app.get(&format!("/api/libraries/{lib}/items?genre=Sci-Fi"), &admin).await;
    assert_eq!(titles(&page.body["items"]), ["The Alien"]);
    // An explicit sort title wins over the title's.
    let sorted = edit(json!({"sort_title": "zzz"})).await;
    assert_eq!(sorted.body["sort_title"], json!("zzz"));
    assert_eq!(
        titles(&app.get(&format!("/api/libraries/{lib}/items"), &admin).await.body["items"]),
        ["Brazil", "The Alien"]
    );

    // A content rating set by hand applies to age limits right away.
    let (kid, _) = app.user(&admin, "kid", json!({"max_age": 12})).await;
    edit(json!({"content_rating": "PG"})).await;
    assert_eq!(app.get(&format!("/api/items/{alien}"), &kid).await.status, StatusCode::OK);
    edit(json!({"content_rating": "R"})).await;
    assert_eq!(app.get(&format!("/api/items/{alien}"), &kid).await.status, StatusCode::NOT_FOUND);

    // Pictures go through the artwork endpoints, which check what they get.
    let upload = |kind: &str, bytes: Vec<u8>| {
        let uri = format!("/api/items/{alien}/artwork/{kind}/upload");
        let app = &app;
        let admin = admin.clone();
        async move {
            let req = axum::http::Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header("authorization", format!("Bearer {admin}"))
                .body(axum::body::Body::from(bytes))
                .unwrap();
            tower::ServiceExt::oneshot(app.api.clone(), req).await.unwrap().status()
        }
    };
    assert_eq!(upload("poster", b"not a picture".to_vec()).await, StatusCode::BAD_REQUEST);
    assert_eq!(upload("poster", Vec::new()).await, StatusCode::BAD_REQUEST);
    assert_eq!(upload("logo", b"x".to_vec()).await, StatusCode::NOT_FOUND);
    let mut png = Vec::new();
    image::RgbImage::from_pixel(10, 15, image::Rgb([9, 9, 9]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    assert_eq!(upload("thumb", png).await, StatusCode::OK);
    let locked = app.get(&format!("/api/items/{alien}"), &admin).await.body["locked_fields"].clone();
    assert!(locked.as_array().unwrap().contains(&json!("thumb")), "{locked}");
    let thumb = app.get(&format!("/api/items/{alien}/images/thumb"), &admin).await;
    assert_eq!((thumb.status, thumb.headers["content-type"].clone()), (StatusCode::OK, "image/jpeg".parse().unwrap()));
    let reset = app.delete(&format!("/api/items/{alien}/artwork/thumb"), &admin).await;
    assert_eq!(reset.status, StatusCode::NO_CONTENT);
    let locked = app.get(&format!("/api/items/{alien}"), &admin).await.body["locked_fields"].clone();
    assert!(!locked.as_array().unwrap().contains(&json!("thumb")));

    // TMDB-backed features say what they need; matching is for movies and shows.
    let options = app.get(&format!("/api/items/{alien}/artwork/poster"), &admin).await;
    assert_eq!(options.status, StatusCode::BAD_REQUEST);
    assert!(options.body["message"].as_str().unwrap().contains("TMDB"));
    let tv_path = app.media.path().join("tv");
    std::fs::create_dir_all(tv_path.join("Show (2015)/Season 01")).unwrap();
    app.media_file("tv/Show (2015)/Season 01/Show - S01E01.mkv", 1024);
    let tv = app.library(&admin, "TV", "shows", "tv").await;
    let season = app.children(app.ids(tv, &admin).await["Show"], &admin).await["Season 1"];
    let rematch = app.post(&format!("/api/items/{season}/match"), &admin, json!({"tmdb_id": 1})).await;
    assert_eq!(rematch.status, StatusCode::BAD_REQUEST);

    // Not for regular users.
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    assert_eq!(
        app.patch(&format!("/api/items/{alien}"), &bob, json!({"title": "x"})).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.delete(&format!("/api/items/{alien}/artwork/poster"), &bob).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post(&format!("/api/items/{alien}/refresh"), &bob, json!({})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post(&format!("/api/items/{alien}/analyze"), &bob, json!({})).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn history_filters() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    app.media_file("movies/Alien (1979).mkv", 1024);
    for (s, e) in [(1, 1), (1, 2), (2, 1)] {
        app.media_file(&format!("tv/Show (2015)/Season {s:02}/Show - S{s:02}E{e:02}.mkv"), 1024);
    }
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let tv = app.library(&admin, "TV", "shows", "tv").await;
    let alien = app.ids(movies, &admin).await["Alien"];
    let show = app.ids(tv, &admin).await["Show"];
    let seasons = app.children(show, &admin).await;
    let mut episodes: Vec<i64> = Vec::new();
    for season in ["Season 1", "Season 2"] {
        let mut ids: Vec<i64> = app.children(seasons[season], &admin).await.into_values().collect();
        ids.sort_unstable();
        episodes.extend(ids);
    }
    for id in episodes.iter().chain([&alien]) {
        let body = json!({"position_ms": 95_000, "duration_ms": 100_000});
        assert_eq!(app.post(&format!("/api/items/{id}/progress"), &admin, body).await.status, StatusCode::NO_CONTENT);
    }
    let total = |query: String| {
        let app = &app;
        let admin = admin.clone();
        async move {
            let reply = app.get(&format!("/api/history{query}"), &admin).await;
            assert_eq!(reply.status, StatusCode::OK, "{query}");
            (reply.body["total"].as_i64().unwrap(), reply.body["entries"].as_array().unwrap().len())
        }
    };
    assert_eq!(total(String::new()).await, (4, 4));
    assert_eq!(total(format!("?item_id={show}")).await.0, 3, "a show covers its episodes");
    assert_eq!(total(format!("?item_id={}", seasons["Season 2"])).await.0, 1);
    assert_eq!(total(format!("?item_id={alien}")).await.0, 1);
    assert_eq!(total(format!("?library_id={movies}")).await.0, 1);
    assert_eq!(total(format!("?library_id={tv}&item_id={}", seasons["Season 1"])).await.0, 2);
    assert_eq!(total("?since=4102444800".into()).await.0, 0);
    assert_eq!(total("?limit=0".into()).await, (4, 1), "at least one entry per page");
    assert_eq!(total("?limit=2&offset=3".into()).await, (4, 1));

    // Others can't look at the administrator's plays.
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    let admin_id = app.get("/api/auth/me", &admin).await.body["id"].as_i64().unwrap();
    let peek = app.get(&format!("/api/history?user_id={admin_id}"), &bob).await;
    assert_eq!(peek.body["total"], json!(0));
    let entry = app.get("/api/history?limit=1", &admin).await.body["entries"][0]["id"].as_i64().unwrap();
    assert_eq!(app.delete(&format!("/api/history/{entry}"), &bob).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.delete("/api/history/999999", &admin).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn library_hubs_show_what_has_content() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    for name in ["Alien (1979)", "Brazil (1985)", "Casablanca (1942)"] {
        app.media_file(&format!("movies/{name}.mkv"), 1024);
    }
    app.media_file("music/Band/Record (2001)/01 - Song.mp3", 512);
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    let music = app.library(&admin, "Music", "music", "music").await;
    let keys = |hubs: &Value| -> Vec<String> {
        hubs["hubs"].as_array().unwrap().iter().map(|h| h["key"].as_str().unwrap().to_owned()).collect()
    };
    let hubs = app.get(&format!("/api/libraries/{movies}/hubs"), &admin).await;
    assert_eq!(keys(&hubs.body), [format!("recently_added:{movies}")], "empty hubs are left out");

    let m = app.ids(movies, &admin).await;
    for (title, rating, released) in [("Alien", 8.5, "1979-05-25"), ("Brazil", 7.9, "1985-12-18")] {
        sqlx::query("UPDATE items SET rating = ?, release_date = ? WHERE id = ?")
            .bind(rating)
            .bind(released)
            .bind(m[title])
            .execute(&app.state.db.write)
            .await
            .unwrap();
    }
    let body = json!({"position_ms": 30_000, "duration_ms": 100_000});
    app.post(&format!("/api/items/{}/progress", m["Casablanca"]), &admin, body).await;
    let hubs = app.get(&format!("/api/libraries/{movies}/hubs"), &admin).await.body;
    assert_eq!(
        keys(&hubs),
        [
            format!("continue_watching:{movies}"),
            format!("recently_added:{movies}"),
            format!("recently_released:{movies}"),
            format!("top_rated:{movies}")
        ]
    );
    let hub = |key: &str| hubs["hubs"].as_array().unwrap().iter().find(|h| h["key"] == key).unwrap().clone();
    assert_eq!(titles(&hub(&format!("recently_released:{movies}"))["items"]), ["Brazil", "Alien"]);
    assert_eq!(titles(&hub(&format!("top_rated:{movies}"))["items"]), ["Alien", "Brazil"]);
    assert_eq!(titles(&hub(&format!("continue_watching:{movies}"))["items"]), ["Casablanca"]);
    let filters = app.get(&format!("/api/libraries/{movies}/filters"), &admin).await;
    assert_eq!(filters.body["years"], json!([1985, 1979, 1942]));

    // Music has albums, never "continue watching".
    let hubs = app.get(&format!("/api/libraries/{music}/hubs"), &admin).await.body;
    assert_eq!(keys(&hubs), [format!("recently_added:{music}")]);
    assert_eq!(titles(&hubs["hubs"][0]["items"]), ["Record"]);
}

#[tokio::test]
async fn administration_endpoints() {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    let (bob, _) = app.user(&admin, "bob", json!({})).await;
    for (method, uri) in [
        (Method::GET, "/api/system"),
        (Method::GET, "/api/system/storage"),
        (Method::POST, "/api/system/storage/clean"),
        (Method::GET, "/api/system/backups"),
        (Method::POST, "/api/system/backups"),
        (Method::GET, "/api/transcoder"),
        (Method::GET, "/api/settings"),
        (Method::PUT, "/api/settings"),
        (Method::GET, "/api/fs"),
    ] {
        let reply = app.call(method.clone(), uri, Some(&bob), Some(json!({})), &[]).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method} {uri}");
    }

    let storage = app.get("/api/system/storage", &admin).await;
    assert_eq!(storage.status, StatusCode::OK);
    for field in ["blobs", "blob_bytes", "media_folders", "media_bytes", "cache_bytes"] {
        assert!(storage.body[field].is_u64(), "{field}: {}", storage.body);
    }
    let cleaned = app.post("/api/system/storage/clean", &admin, json!({})).await;
    assert_eq!(cleaned.body["blobs_removed"], json!(0));

    let backup = app.post("/api/system/backups", &admin, json!({})).await;
    assert_eq!(backup.status, StatusCode::OK, "{:?}", backup.body);
    let name = backup.body["name"].as_str().unwrap();
    assert!(name.starts_with("pley-") && name.ends_with(".db"), "{name}");
    let backups = app.get("/api/system/backups", &admin).await;
    assert_eq!(backups.body["backups"].as_array().unwrap().len(), 1);
    assert_eq!(backups.body["health"]["ok"], Value::Null, "checked at start-up, which tests skip");

    // The folder browser lists folders, without hidden ones.
    let root = app.media.path();
    for dir in ["Films", ".hidden", "$RECYCLE.BIN", "series"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(root.join("notes.txt"), b"x").unwrap();
    let path = root.to_string_lossy().into_owned();
    let listing = app.get(&format!("/api/fs?path={}", common::encode(&path)), &admin).await;
    let names: Vec<&str> =
        listing.body["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Films", "series"]);
    assert_eq!(listing.body["parent"], json!(root.parent().unwrap().to_string_lossy()));
    let missing = app.get(&format!("/api/fs?path={}", common::encode(&format!("{path}/nope"))), &admin).await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get("/api/fs?path=relative", &admin).await.status, StatusCode::BAD_REQUEST);
}
