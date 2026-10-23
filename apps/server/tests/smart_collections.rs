//! Smart collections: filled by a library filter (Plex's syntax), through
//! pley's API and the Plex-compatible one, as each user may see them.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use common::{TestApp, plex_titles, titles};

struct World {
    app: TestApp,
    admin: String,
    movies: i64,
}

/// Four films with genres and years; Alien is rated R, the others PG.
async fn world() -> World {
    let app = TestApp::new().await;
    let admin = app.setup_admin().await;
    for name in ["Alien (1979)", "Brazil (1985)", "2001 A Space Odyssey (1968)", "Airplane (1980)"] {
        app.media_file(&format!("movies/{name}.mkv"), 1024);
    }
    let movies = app.library(&admin, "Movies", "movies", "movies").await;
    for (title, genres) in [
        ("Alien", r#"["Horror","Science Fiction"]"#),
        ("Brazil", r#"["Comedy","Science Fiction"]"#),
        ("2001 A Space Odyssey", r#"["Science Fiction"]"#),
        ("Airplane", r#"["Comedy"]"#),
    ] {
        sqlx::query("UPDATE items SET genres = ? WHERE title = ?")
            .bind(genres)
            .bind(title)
            .execute(&app.state.db.write)
            .await
            .unwrap();
    }
    app.set_rating("Alien", Some("R")).await;
    for title in ["Brazil", "2001 A Space Odyssey", "Airplane"] {
        app.set_rating(title, Some("PG")).await;
    }
    World { app, admin, movies }
}

async fn items_of(w: &World, token: &str, id: i64) -> Value {
    let got = w.app.get(&format!("/api/collections/{id}"), token).await;
    assert_eq!(got.status, StatusCode::OK, "{:?}", got.body);
    got.body["items"].clone()
}

#[tokio::test]
async fn smart_collections_fill_themselves() {
    let w = world().await;
    let filter = "genre=Science%20Fiction&sort=year";
    let created = w
        .app
        .post(
            &format!("/api/libraries/{}/collections", w.movies),
            &w.admin,
            json!({"title": "Sci-fi", "smart_filter": filter}),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(created.body["smart_filter"], json!(filter));
    let id = created.body["id"].as_i64().unwrap();
    assert_eq!(
        titles(&items_of(&w, &w.admin, id).await),
        ["2001 A Space Odyssey", "Alien", "Brazil"],
        "in the filter's order"
    );

    // A film tagged later joins by itself.
    sqlx::query("UPDATE items SET genres = ? WHERE title = 'Airplane'")
        .bind(r#"["Comedy","Science Fiction"]"#)
        .execute(&w.app.state.db.write)
        .await
        .unwrap();
    assert_eq!(titles(&items_of(&w, &w.admin, id).await).len(), 4);
    let listed = w.app.get(&format!("/api/libraries/{}/collections", w.movies), &w.admin).await;
    assert_eq!(listed.body["collections"][0]["item_count"], json!(4));

    // Each viewer sees what they may.
    let (kid, _) = w.app.user(&w.admin, "kid", json!({"max_age": 12})).await;
    assert_eq!(titles(&items_of(&w, &kid, id).await), ["2001 A Space Odyssey", "Airplane", "Brazil"]);

    // Its items come from the filter only; the filter can change.
    let added = w.app.post(&format!("/api/collections/{id}/items"), &w.admin, json!({"item_ids": [1]})).await;
    assert_eq!(added.status, StatusCode::BAD_REQUEST);
    let changed =
        w.app.patch(&format!("/api/collections/{id}"), &w.admin, json!({"smart_filter": "genre=Comedy"})).await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    assert_eq!(titles(&items_of(&w, &w.admin, id).await), ["Airplane", "Brazil"]);
    let bad = w.app.patch(&format!("/api/collections/{id}"), &w.admin, json!({"smart_filter": "type=8"})).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST, "artists aren't in a movie library");
}

#[tokio::test]
async fn smart_collections_through_the_plex_api() {
    let w = world().await;
    let (app, admin, movies) = (&w.app, w.admin.as_str(), w.movies);
    // As python-plexapi builds it: the section listing as a server:// uri.
    let uri = format!("server://m/com.plexapp.plugins.library/library/sections/{movies}/all?type=1&genre=Comedy");
    let create =
        format!("/library/collections?type=1&title=Comedies&smart=1&sectionId={movies}&uri={}", common::encode(&uri));
    let created = app.plex(Method::POST, &create, admin, &[]).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    let collection = &created.body["MediaContainer"]["Metadata"][0];
    assert_eq!((&collection["smart"], &collection["title"]), (&json!(true), &json!("Comedies")));
    assert_eq!(collection["content"], json!(format!("/library/sections/{movies}/all?type=1&genre=Comedy")));
    let id = collection["ratingKey"].as_str().unwrap().to_owned();
    let children = app.plex_json(&format!("/library/collections/{id}/children"), admin).await;
    assert_eq!(plex_titles(&children), ["Airplane", "Brazil"]);

    // A new filter (python-plexapi's updateFilters).
    let horror = format!("server://m/com.plexapp.plugins.library/library/sections/{movies}/all?type=1&genre=Horror");
    let update = format!("/library/collections/{id}/items?uri={}", common::encode(&horror));
    assert_eq!(app.plex(Method::PUT, &update, admin, &[]).await.status, StatusCode::OK);
    let children = app.plex_json(&format!("/library/collections/{id}/children"), admin).await;
    assert_eq!(plex_titles(&children), ["Alien"]);

    // A plain collection from items, then the smart one deleted.
    let ids = app.ids(movies, admin).await;
    let items = format!("server://m/com.plexapp.plugins.library/library/metadata/{},{}", ids["Alien"], ids["Brazil"]);
    let plain =
        format!("/library/collections?type=1&title=Pair&smart=0&sectionId={movies}&uri={}", common::encode(&items));
    let created = app.plex(Method::POST, &plain, admin, &[]).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    assert_eq!(created.body["MediaContainer"]["Metadata"][0]["childCount"], json!(2));
    assert_eq!(
        app.plex(Method::DELETE, &format!("/library/collections/{id}"), admin, &[]).await.status,
        StatusCode::OK
    );
    let left = app.plex_json(&format!("/library/sections/{movies}/collections"), admin).await;
    let names: Vec<&str> = left["Metadata"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(names, ["Pair"]);

    // Only administrators make collections.
    let (kid, _) = app.user(admin, "kid", json!({})).await;
    assert_eq!(app.plex(Method::POST, &create, &kid, &[]).await.status, StatusCode::FORBIDDEN);
}

fn entry_titles(playlist: &Value) -> Vec<String> {
    playlist["entries"].as_array().unwrap().iter().map(|e| e["item"]["title"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn smart_playlists_play_what_their_filter_finds() {
    let w = world().await;
    let (app, admin) = (&w.app, w.admin.as_str());
    let created = app
        .post(
            "/api/playlists",
            admin,
            json!({"title": "Comedies", "smart_filter": "genre=Comedy&sort=titleSort", "library_id": w.movies}),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    assert_eq!(
        (&created.body["kind"], &created.body["smart_filter"]),
        (&json!("video"), &json!("genre=Comedy&sort=titleSort"))
    );
    let id = created.body["id"].as_i64().unwrap();
    assert_eq!(entry_titles(&created.body), ["Airplane", "Brazil"]);
    let added = app.post(&format!("/api/playlists/{id}/items"), admin, json!({"item_ids": [1]})).await;
    assert_eq!(added.status, StatusCode::BAD_REQUEST, "filled by its filter");
    let changed = app.patch(&format!("/api/playlists/{id}"), admin, json!({"smart_filter": "genre=Horror"})).await;
    assert_eq!(entry_titles(&changed.body), ["Alien"]);
    // Someone else's playlist stays theirs.
    let (bob, _) = app.user(admin, "bob", json!({})).await;
    assert_eq!(app.get(&format!("/api/playlists/{id}"), &bob).await.status, StatusCode::NOT_FOUND);

    // Shows play as their episodes.
    for e in [1, 2] {
        app.media_file(&format!("tv/Taxi (1978)/Season 01/Taxi - S01E0{e}.mkv"), 1024);
    }
    let tv = app.library(admin, "TV", "shows", "tv").await;
    let shows = app
        .post("/api/playlists", admin, json!({"title": "Taxi", "smart_filter": "type=2&title=taxi", "library_id": tv}))
        .await;
    assert_eq!(shows.status, StatusCode::CREATED, "{:?}", shows.body);
    let kinds: Vec<&str> =
        shows.body["entries"].as_array().unwrap().iter().map(|e| e["item"]["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["episode", "episode"]);
}

#[tokio::test]
async fn smart_playlists_through_the_plex_api() {
    let w = world().await;
    let (app, admin, movies) = (&w.app, w.admin.as_str(), w.movies);
    let uri = format!("server://m/com.plexapp.plugins.library/library/sections/{movies}/all?type=1&genre=Horror");
    let create = format!("/playlists?type=video&title=Scary&smart=1&uri={}", common::encode(&uri));
    let created = app.plex(Method::POST, &create, admin, &[]).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    let playlist = &created.body["MediaContainer"]["Metadata"][0];
    assert_eq!(
        (&playlist["smart"], &playlist["content"]),
        (&json!(true), &json!(format!("/library/sections/{movies}/all?type=1&genre=Horror")))
    );
    let id = playlist["ratingKey"].as_str().unwrap().to_owned();
    assert_eq!(plex_titles(&app.plex_json(&format!("/playlists/{id}/items"), admin).await), ["Alien"]);

    let comedy = format!("server://m/com.plexapp.plugins.library/library/sections/{movies}/all?type=1&genre=Comedy");
    let update = format!("/playlists/{id}/items?uri={}", common::encode(&comedy));
    assert_eq!(app.plex(Method::PUT, &update, admin, &[]).await.status, StatusCode::OK);
    assert_eq!(plex_titles(&app.plex_json(&format!("/playlists/{id}/items"), admin).await).len(), 2);
    let smart = app.plex_json("/playlists?smart=1", admin).await;
    assert_eq!(smart["size"], json!(1));
    assert_eq!(app.plex_json("/playlists?smart=0", admin).await["size"], json!(0));
}
