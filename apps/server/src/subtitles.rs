//! Subtitles added to a media file, uploaded from the web app. They are
//! kept in the data folder (`subtitles/<file_id>/<id>.<ext>`, never next to the
//! media, which pley doesn't change) as UTF-8 text, and every client sees
//! them as ordinary subtitle files: external text tracks, flagged `added`,
//! whose WebVTT, burn-in and downloads go the way a sidecar's do.
//!
//! Track indexes: embedded tracks are below 1000, subtitle files next to the
//! video from 1000 ([`crate::library::sidecar::EXTERNAL_INDEX_BASE`]) to
//! 8999, added ones from [`ADDED_INDEX_BASE`] to 9999 (the Plex-compatible
//! API's stream ids hold four digits of index). An added track's index is
//! given once among the files of its title, so a title's `DELETE` needs no
//! file.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pley_transcoder_api::{MediaStream, StreamKind};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::catalog::FileRow;
use crate::db::Db;
use crate::error::{ApiResult, AppError};

/// Added tracks' indexes start here...
pub const ADDED_INDEX_BASE: i64 = 9000;
/// ...and stay below this.
pub const ADDED_INDEX_END: i64 = 10_000;
/// The largest subtitle file taken: a film's SRT takes 100 KB, an elaborate
/// ASS a megabyte.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// An added subtitle track, as [`FileRow::added_subs`] lists them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddedSub {
    /// Its row: the file is `<id>.<ext>`.
    pub id: i64,
    pub index: i64,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub sdh: bool,
    /// Who added it (`None` once their account is gone).
    pub added_by: Option<i64>,
}

impl AddedSub {
    pub fn to_stream(&self) -> MediaStream {
        MediaStream {
            index: self.index,
            kind: StreamKind::Subtitle,
            codec: self.codec.clone(),
            language: self.language.clone(),
            title: self.title.clone(),
            forced: self.forced,
            text_based: Some(true),
            external: true,
            added: true,
            ..Default::default()
        }
    }

    /// Where its file is, in `dir` ([`crate::config::Config::added_subtitles_dir`]).
    pub fn path(&self, dir: &Path, file_id: i64) -> PathBuf {
        let ext = Format::from_codec(&self.codec).map_or("srt", Format::extension);
        dir.join(file_id.to_string()).join(format!("{}.{ext}", self.id))
    }

    /// Whether `user` may delete it: an administrator, or whoever added it.
    pub fn may_delete(&self, user: &AuthUser) -> bool {
        user.is_admin || self.added_by == Some(user.id)
    }
}

/// The subtitle formats that can be added: text that every player gets as
/// WebVTT, and that libass draws when burned in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Srt,
    Vtt,
    Ass,
    Ssa,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "srt" => Format::Srt,
            "vtt" | "webvtt" => Format::Vtt,
            "ass" => Format::Ass,
            "ssa" => Format::Ssa,
            _ => return None,
        })
    }

    pub fn from_codec(codec: &str) -> Option<Self> {
        Some(match codec {
            "subrip" => Format::Srt,
            "webvtt" => Format::Vtt,
            "ass" => Format::Ass,
            "ssa" => Format::Ssa,
            _ => return None,
        })
    }

    /// The codec name tracks carry (as ffprobe names it).
    pub fn codec(self) -> &'static str {
        match self {
            Format::Srt => "subrip",
            Format::Vtt => "webvtt",
            Format::Ass => "ass",
            Format::Ssa => "ssa",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::Vtt => "vtt",
            Format::Ass => "ass",
            Format::Ssa => "ssa",
        }
    }

    /// The format `text` is in, by what it holds: a file named `.srt` may
    /// well be WebVTT. `named` (the file's extension) settles ASS and SSA,
    /// which read alike. `None` when it is none of them.
    pub fn of(text: &str, named: Option<Format>) -> Option<Format> {
        let head = text.trim_start();
        if head.starts_with("WEBVTT") {
            return has_timing(text, VTT_TIMING).then_some(Format::Vtt);
        }
        let lower = text.to_ascii_lowercase();
        if lower.contains("[script info]") && lower.contains("[events]") {
            let dialogue = text.lines().any(|line| line.trim_start().starts_with("Dialogue:"));
            if !dialogue {
                return None;
            }
            let ssa = named == Some(Format::Ssa) || (named != Some(Format::Ass) && !lower.contains("v4.00+"));
            return Some(if ssa { Format::Ssa } else { Format::Ass });
        }
        has_timing(text, SRT_TIMING).then_some(Format::Srt)
    }
}

const SRT_TIMING: &str = r"(?m)^\s*\d{1,3}:\d{2}:\d{2}[,.]\d{1,3}\s*-->\s*\d{1,3}:\d{2}:\d{2}[,.]\d{1,3}";
const VTT_TIMING: &str = r"(?m)^\s*(\d{1,3}:)?\d{2}:\d{2}\.\d{3}\s+-->\s+(\d{1,3}:)?\d{2}:\d{2}\.\d{3}";

fn has_timing(text: &str, pattern: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|re| re.is_match(text))
}

/// Subtitle text as UTF-8, whatever it came in: without a byte order mark,
/// UTF-16 by its mark, UTF-8 when it reads as such, else Windows-1252 (as
/// the transcoder reads subtitle files).
pub fn to_utf8(raw: &[u8]) -> String {
    let text = if let Some(rest) = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if let Some(rest) = raw.strip_prefix(&[0xFF, 0xFE]) {
        utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = raw.strip_prefix(&[0xFE, 0xFF]) {
        utf16(rest, u16::from_be_bytes)
    } else {
        match std::str::from_utf8(raw) {
            Ok(text) => text.to_owned(),
            Err(_) => raw.iter().map(|&b| windows_1252(b)).collect(),
        }
    };
    text.replace('\0', "")
}

fn utf16(bytes: &[u8], word: fn([u8; 2]) -> u16) -> String {
    let words: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| word(*pair)).collect();
    String::from_utf16_lossy(&words)
}

fn windows_1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    match byte {
        0x80..=0x9F => HIGH[(byte - 0x80) as usize],
        _ => byte as char,
    }
}

/// A subtitle file as it is kept: UTF-8 text, checked to be the format it
/// says it is. `name` is the file's name (its extension is checked against
/// the formats that can be added).
pub fn read(raw: &[u8], name: &str) -> ApiResult<(Format, String)> {
    if raw.len() > MAX_BYTES {
        return Err(AppError::bad_request("subtitle files can be 2 MiB at most"));
    }
    let named = name.rsplit_once('.').and_then(|(_, ext)| Format::from_extension(ext));
    if named.is_none() {
        return Err(AppError::bad_request("subtitle files must be .srt, .vtt, .ass or .ssa"));
    }
    let text = to_utf8(raw);
    let format = Format::of(&text, named)
        .ok_or_else(|| AppError::bad_request("this isn't a subtitle file pley can read (SRT, WebVTT, ASS or SSA)"))?;
    Ok((format, text))
}

/// A track's language as given (`en`, `eng`, `English`, `pt-BR`): ISO 639-2
/// as the other tracks have it when known, else the code as given, in lower
/// case. `None` when it isn't a language code.
pub fn language(given: &str) -> Option<String> {
    let given = given.trim().to_lowercase();
    if let Some(code) = crate::library::sidecar::normalize_language(&given) {
        return Some(code);
    }
    let primary = given.split(['-', '_']).next().unwrap_or_default();
    if let Some(code) = crate::library::sidecar::normalize_language(primary) {
        return Some(code);
    }
    let code = regex::Regex::new(r"^[a-z]{2,3}([-_][a-z0-9]{2,8})?$").ok()?;
    code.is_match(&given).then_some(given)
}

/// A subtitle track being added.
#[derive(Debug, Clone)]
pub struct NewSub {
    pub format: Format,
    pub text: String,
    /// ISO 639-2 when known ([`language`]).
    pub language: String,
    pub title: Option<String>,
    pub forced: bool,
    pub sdh: bool,
    /// `upload`, or `opensubtitles:<file id>`.
    pub source: String,
    pub added_by: i64,
}

/// A track's title from a file's name: without its extension, at most 100
/// characters.
pub fn title_from(name: &str) -> Option<String> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem).trim();
    let stem: String = stem.chars().take(100).collect();
    (!stem.is_empty()).then_some(stem)
}

/// Adds `sub` to `file`: its text is written first, under a name of its own,
/// then the track is listed with the next free index of the file's title,
/// and the file takes its final name.
pub async fn add(db: &Db, dir: &Path, file: &FileRow, sub: NewSub) -> ApiResult<AddedSub> {
    let folder = dir.join(file.id.to_string());
    tokio::fs::create_dir_all(&folder).await?;
    let part = folder.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    tokio::fs::write(&part, sub.text.as_bytes()).await?;
    let listed = list(db, file, &sub).await;
    let added = match listed {
        Ok(added) => added,
        Err(err) => {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(err);
        }
    };
    if let Err(err) = tokio::fs::rename(&part, added.path(dir, file.id)).await {
        let _ = tokio::fs::remove_file(&part).await;
        let _ = sqlx::query("DELETE FROM added_subtitles WHERE id = ?").bind(added.id).execute(&db.write).await;
        return Err(err.into());
    }
    Ok(added)
}

async fn list(db: &Db, file: &FileRow, sub: &NewSub) -> ApiResult<AddedSub> {
    let mut tx = db.write.begin().await?;
    let used: Vec<i64> = sqlx::query_scalar(
        "SELECT a.idx FROM added_subtitles a JOIN media_files f ON f.id = a.file_id WHERE f.item_id = ?",
    )
    .bind(file.item_id)
    .fetch_all(&mut *tx)
    .await?;
    let index =
        next_index(&used).ok_or_else(|| AppError::conflict("this title has as many added subtitles as it can"))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO added_subtitles (file_id, idx, codec, language, title, forced, sdh, source, added_by, added_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(file.id)
    .bind(index)
    .bind(sub.format.codec())
    .bind(&sub.language)
    .bind(&sub.title)
    .bind(sub.forced)
    .bind(sub.sdh)
    .bind(&sub.source)
    .bind(sub.added_by)
    .bind(crate::util::now())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(AddedSub {
        id,
        index,
        codec: sub.format.codec().to_owned(),
        language: Some(sub.language.clone()),
        title: sub.title.clone(),
        forced: sub.forced,
        sdh: sub.sdh,
        added_by: Some(sub.added_by),
    })
}

/// The index an added track gets next to those `used`: after the highest,
/// or once that reaches the end, the lowest free one.
fn next_index(used: &[i64]) -> Option<i64> {
    let next = used.iter().copied().max().map_or(ADDED_INDEX_BASE, |max| (max + 1).max(ADDED_INDEX_BASE));
    if next < ADDED_INDEX_END {
        return Some(next);
    }
    (ADDED_INDEX_BASE..ADDED_INDEX_END).find(|i| !used.contains(i))
}

/// The added track `index` of a title, and the file it belongs to.
pub fn find(files: &[FileRow], index: i64) -> Option<(&FileRow, AddedSub)> {
    files.iter().find_map(|file| file.added_subs().into_iter().find(|s| s.index == index).map(|sub| (file, sub)))
}

/// Removes an added track, its file too.
pub async fn delete(db: &Db, dir: &Path, file_id: i64, sub: &AddedSub) -> ApiResult<()> {
    sqlx::query("DELETE FROM added_subtitles WHERE id = ?").bind(sub.id).execute(&db.write).await?;
    let path = sub.path(dir, file_id);
    if let Err(err) = tokio::fs::remove_file(&path).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("could not remove {}: {err}", path.display());
    }
    Ok(())
}

/// Removes the files of added tracks that went with their media file (a
/// file removed from a library takes its tracks with it) and are older than
/// `grace`: `(files, bytes)`. Blocking.
pub fn sweep(dir: &Path, listed: &std::collections::HashSet<i64>, grace: Duration) -> (u64, u64) {
    let (mut count, mut bytes) = (0, 0);
    let now = SystemTime::now();
    let Ok(folders) = std::fs::read_dir(dir) else { return (0, 0) };
    for folder in folders.flatten() {
        let Ok(files) = std::fs::read_dir(folder.path()) else { continue };
        let mut left = 0;
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let id = name.split('.').next().and_then(|id| id.parse::<i64>().ok());
            let meta = file.metadata().ok();
            let old = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| now.duration_since(t).unwrap_or_default() >= grace);
            let unlisted = id.is_none_or(|id| !listed.contains(&id));
            if old && unlisted && std::fs::remove_file(file.path()).is_ok() {
                count += 1;
                bytes += meta.map_or(0, |m| m.len());
            } else {
                left += 1;
            }
        }
        if left == 0 {
            let _ = std::fs::remove_dir(folder.path());
        }
    }
    (count, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str =
        "1\r\n00:00:01,000 --> 00:00:02,500\r\nHello\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nThere\r\n";
    const VTT: &str = "WEBVTT\n\n00:01.000 --> 00:02.500\nHello\n";
    const ASS: &str = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name\n\n[Events]\nFormat: Layer, Start, End, Style, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,Hello\n";

    #[test]
    fn formats_are_read_from_what_files_hold() {
        assert_eq!(Format::of(SRT, Some(Format::Srt)), Some(Format::Srt));
        assert_eq!(Format::of(VTT, Some(Format::Srt)), Some(Format::Vtt), "WebVTT named .srt");
        assert_eq!(Format::of(ASS, Some(Format::Ass)), Some(Format::Ass));
        assert_eq!(Format::of(ASS, Some(Format::Ssa)), Some(Format::Ssa));
        assert_eq!(Format::of(&ASS.replace("v4.00+", "v4.00"), None), Some(Format::Ssa));
        assert_eq!(Format::of("WEBVTT\n\nno cues", None), None);
        assert_eq!(Format::of("[Script Info]\n[Events]\n", None), None, "no dialogue");
        assert_eq!(Format::of("just some text", Some(Format::Srt)), None);
        assert_eq!(Format::of("\u{1}\u{2}binary", Some(Format::Srt)), None);
    }

    #[test]
    fn files_are_checked_and_made_utf8() {
        let (format, text) = read(SRT.as_bytes(), "Movie.en.srt").unwrap();
        assert_eq!((format, text.as_str()), (Format::Srt, SRT));
        assert!(matches!(read(SRT.as_bytes(), "movie.sub"), Err(AppError::BadRequest(_))), "not a format taken");
        assert!(matches!(read(b"hello", "movie.srt"), Err(AppError::BadRequest(_))));
        let big = vec![b'a'; MAX_BYTES + 1];
        assert!(matches!(read(&big, "movie.srt"), Err(AppError::BadRequest(_))));

        // A byte order mark goes; UTF-16 and Windows-1252 become UTF-8.
        let bom = [&[0xEF, 0xBB, 0xBF][..], SRT.as_bytes()].concat();
        assert_eq!(to_utf8(&bom), SRT);
        let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain("Café".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(to_utf8(&utf16), "Café");
        let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("Ünï".encode_utf16().flat_map(u16::to_be_bytes)).collect();
        assert_eq!(to_utf8(&be), "Ünï");
        assert_eq!(to_utf8(b"Caf\xe9 \x93quoted\x94 \x80"), "Café “quoted” €");
        let latin = [b"1\n00:00:01,000 --> 00:00:02,000\n".as_slice(), b"Gar\xe7on\n"].concat();
        let (_, text) = read(&latin, "x.srt").unwrap();
        assert!(text.ends_with("Garçon\n"), "{text}");
    }

    #[test]
    fn languages_are_named_as_tracks_name_them() {
        assert_eq!(language("en").as_deref(), Some("eng"));
        assert_eq!(language(" English ").as_deref(), Some("eng"));
        assert_eq!(language("pt-BR").as_deref(), Some("por"));
        assert_eq!(language("zh-CN").as_deref(), Some("chi"));
        assert_eq!(language("fil").as_deref(), Some("fil"), "unknown codes as given");
        assert_eq!(language("x").as_deref(), None);
        assert_eq!(language("not a language").as_deref(), None);
    }

    #[test]
    fn indexes_count_on_then_fill_gaps() {
        assert_eq!(next_index(&[]), Some(9000));
        assert_eq!(next_index(&[9000, 9003]), Some(9004));
        assert_eq!(next_index(&[9999]), Some(9000));
        assert_eq!(next_index(&[9000, 9999]), Some(9001));
        let all: Vec<i64> = (ADDED_INDEX_BASE..ADDED_INDEX_END).collect();
        assert_eq!(next_index(&all), None);
    }

    #[test]
    fn titles_come_from_file_names() {
        assert_eq!(
            title_from("Movie.2019.1080p.BluRay.x264-GRP.srt").as_deref(),
            Some("Movie.2019.1080p.BluRay.x264-GRP")
        );
        assert_eq!(title_from(".srt"), None);
        assert_eq!(title_from(&format!("{}.srt", "x".repeat(300))).unwrap().len(), 100);
    }

    #[test]
    fn sweeps_take_only_files_nothing_lists() {
        let dir = tempfile::tempdir().unwrap();
        let (kept, gone, other) = (dir.path().join("7"), dir.path().join("8"), dir.path().join("9"));
        for folder in [&kept, &gone, &other] {
            std::fs::create_dir(folder).unwrap();
        }
        std::fs::write(kept.join("1.srt"), "a").unwrap();
        std::fs::write(kept.join("2.srt"), "bb").unwrap();
        std::fs::write(gone.join("3.ass"), "ccc").unwrap();
        std::fs::write(other.join(".x.tmp"), "d").unwrap();
        let listed = [1].into_iter().collect();
        // Young files stay, listed or not.
        assert_eq!(sweep(dir.path(), &listed, Duration::from_secs(3600)), (0, 0));
        assert_eq!(sweep(dir.path(), &listed, Duration::ZERO), (3, 6));
        assert!(kept.join("1.srt").exists() && !kept.join("2.srt").exists());
        assert!(!gone.exists() && !other.exists(), "empty folders go");
    }
}
