//! Libraries: folders of movies, shows or music that are scanned into items.

pub mod parse;
pub mod scanner;
pub mod sidecar;
pub mod walk;
pub mod watcher;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryKind {
    Movies,
    Shows,
    Music,
}

impl LibraryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LibraryKind::Movies => "movies",
            LibraryKind::Shows => "shows",
            LibraryKind::Music => "music",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "movies" => Some(LibraryKind::Movies),
            "shows" => Some(LibraryKind::Shows),
            "music" => Some(LibraryKind::Music),
            _ => None,
        }
    }

    /// Item kind listed at the top level of a library of this kind.
    pub fn top_level_kind(self) -> &'static str {
        match self {
            LibraryKind::Movies => "movie",
            LibraryKind::Shows => "show",
            LibraryKind::Music => "artist",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LibraryRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub paths: String,
    pub created_at: i64,
    pub scanned_at: Option<i64>,
}

impl LibraryRow {
    pub fn kind(&self) -> LibraryKind {
        LibraryKind::parse(&self.kind).unwrap_or(LibraryKind::Movies)
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        serde_json::from_str::<Vec<String>>(&self.paths).unwrap_or_default().into_iter().map(PathBuf::from).collect()
    }
}

pub async fn get_library(db: &Db, id: i64) -> sqlx::Result<Option<LibraryRow>> {
    sqlx::query_as("SELECT id, name, kind, paths, created_at, scanned_at FROM libraries WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.read)
        .await
}

pub async fn list_libraries(db: &Db) -> sqlx::Result<Vec<LibraryRow>> {
    sqlx::query_as("SELECT id, name, kind, paths, created_at, scanned_at FROM libraries ORDER BY name COLLATE NOCASE")
        .fetch_all(&db.read)
        .await
}
