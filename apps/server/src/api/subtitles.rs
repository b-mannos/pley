//! Subtitles added to a title (see [`crate::subtitles`]): uploaded, or
//! deleted again.
//!
//! - `POST /api/items/{id}/subtitles`: a file (`multipart/form-data`)
//! - `DELETE /api/items/{id}/subtitles/{index}`

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::api::items::item_or_404;
use crate::auth::AuthUser;
use crate::catalog::{self, FileRow, ItemRow};
use crate::error::{ApiResult, AppError};
use crate::playback::{SubtitleTrack, subtitle_tracks};
use crate::state::AppState;
use crate::subtitles::{self, AddedSub, NewSub};

/// A title that plays as video, and the file of it asked for (its first).
async fn video_file(state: &AppState, user: &AuthUser, id: i64, file_id: Option<i64>) -> ApiResult<(ItemRow, FileRow)> {
    let row = item_or_404(state, user.id, id).await?;
    if !matches!(row.kind.as_str(), "movie" | "episode" | "extra") {
        return Err(AppError::bad_request("only movies, episodes and extras have subtitles"));
    }
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let file = match file_id {
        Some(id) => files.into_iter().find(|f| f.id == id).ok_or_else(|| AppError::not_found("file not found"))?,
        None => files.into_iter().next().ok_or_else(|| AppError::not_found("this item has no media file"))?,
    };
    Ok((row, file))
}

/// `POST /api/items/{id}/subtitles`: adds a subtitle to the title's file
/// (its first, or `file_id`): a file uploaded as `multipart/form-data`
/// (`file`, `language`, and `hearing_impaired`, `forced`, `file_id` if need
/// be). `201` with the new track.
pub async fn add(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Response> {
    let content_type = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let (file, sub) = if content_type.to_ascii_lowercase().starts_with("multipart/form-data") {
        let form = Form::parse(content_type, &body)?;
        let file_id =
            form.text("file_id").map(|v| v.parse().map_err(|_| AppError::bad_request("bad file_id"))).transpose()?;
        let (_, file) = video_file(&state, &user, id, file_id).await?;
        let (name, data) = form.file("file").ok_or_else(|| AppError::bad_request("a subtitle file is needed"))?;
        let (format, text) = subtitles::read(data, name)?;
        let language =
            form.text("language").ok_or_else(|| AppError::bad_request("the subtitles' language is needed"))?;
        let language = subtitles::language(language)
            .ok_or_else(|| AppError::bad_request(format!("{language} isn't a language code")))?;
        let sub = NewSub {
            format,
            text,
            language,
            title: subtitles::title_from(name),
            forced: form.flag("forced")?,
            sdh: form.flag("hearing_impaired")?,
            source: "upload".into(),
            added_by: user.id,
        };
        (file, sub)
    } else {
        return Err(AppError::bad_request("subtitles are added as a file, in a multipart/form-data form"));
    };
    let added = subtitles::add(&state.db, &state.config.added_subtitles_dir(), &file, sub).await?;
    tracing::info!("{} added subtitles {} to {}", user.username, added.index, file.path);
    Ok((StatusCode::CREATED, Json(track(&file, &added, &user))).into_response())
}

/// An added track as players list it.
fn track(file: &FileRow, added: &AddedSub, user: &AuthUser) -> SubtitleTrack {
    let mut track = subtitle_tracks(file.id, &[added.to_stream()]).remove(0);
    track.deletable = added.may_delete(user);
    track
}

/// `DELETE /api/items/{id}/subtitles/{index}`: an added track of the title,
/// by whoever added it or an administrator. `204`.
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, index)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    let row = item_or_404(&state, user.id, id).await?;
    let files = catalog::files_for_item(&state.db, row.id).await?;
    let (file, sub) = subtitles::find(&files, index)
        .ok_or_else(|| AppError::not_found("this title has no added subtitles with that index"))?;
    if !sub.may_delete(&user) {
        return Err(AppError::forbidden("only an administrator or whoever added these subtitles can delete them"));
    }
    subtitles::delete(&state.db, &state.config.added_subtitles_dir(), file.id, &sub).await?;
    tracing::info!("{} deleted subtitles {index} of {}", user.username, file.path);
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ forms

/// The parts of a `multipart/form-data` body (RFC 7578): a few small ones.
struct Form<'a> {
    parts: Vec<Part<'a>>,
}

struct Part<'a> {
    name: String,
    filename: Option<String>,
    data: &'a [u8],
}

impl<'a> Form<'a> {
    fn parse(content_type: &str, body: &'a [u8]) -> ApiResult<Self> {
        let bad = || AppError::bad_request("a form that can't be read");
        let boundary = content_type
            .split(';')
            .filter_map(|p| p.trim().split_once('='))
            .find(|(k, _)| k.trim().eq_ignore_ascii_case("boundary"))
            .map(|(_, v)| v.trim().trim_matches('"').to_owned())
            .filter(|b| !b.is_empty() && b.len() <= 70)
            .ok_or_else(bad)?;
        let delimiter = format!("--{boundary}").into_bytes();
        let mut parts = Vec::new();
        let mut rest = &body[find(body, &delimiter).ok_or_else(bad)? + delimiter.len()..];
        loop {
            if rest.starts_with(b"--") {
                break;
            }
            rest = rest.strip_prefix(b"\r\n").ok_or_else(bad)?;
            let head_end = find(rest, b"\r\n\r\n").ok_or_else(bad)?;
            let head = std::str::from_utf8(&rest[..head_end]).map_err(|_| bad())?;
            let content = &rest[head_end + 4..];
            let mut end_marker = b"\r\n".to_vec();
            end_marker.extend_from_slice(&delimiter);
            let end = find(content, &end_marker).ok_or_else(bad)?;
            let (mut name, mut filename) = (None, None);
            for line in head.split("\r\n") {
                let Some((key, value)) = line.split_once(':') else { continue };
                if !key.trim().eq_ignore_ascii_case("content-disposition") {
                    continue;
                }
                for param in value.split(';').skip(1) {
                    let Some((k, v)) = param.trim().split_once('=') else { continue };
                    let v = v.trim().trim_matches('"').to_owned();
                    match k.trim().to_ascii_lowercase().as_str() {
                        "name" => name = Some(v),
                        "filename" => filename = Some(v),
                        _ => {}
                    }
                }
            }
            if let Some(name) = name {
                parts.push(Part { name, filename, data: &content[..end] });
            }
            if parts.len() > 16 {
                return Err(bad());
            }
            rest = &content[end + end_marker.len()..];
        }
        Ok(Self { parts })
    }

    fn part(&self, name: &str) -> Option<&Part<'a>> {
        self.parts.iter().find(|p| p.name == name)
    }

    /// A field's text, when it has some.
    fn text(&self, name: &str) -> Option<&str> {
        self.part(name).and_then(|p| std::str::from_utf8(p.data).ok()).map(str::trim).filter(|t| !t.is_empty())
    }

    /// A file's name (its path's last part) and contents.
    fn file(&self, name: &str) -> Option<(&str, &'a [u8])> {
        let part = self.part(name)?;
        let filename = part.filename.as_deref()?;
        Some((filename.rsplit(['/', '\\']).next().unwrap_or(filename), part.data))
    }

    /// A yes-or-no field: `true`, `1`, `on` or `yes`; `false` when left out.
    fn flag(&self, name: &str) -> ApiResult<bool> {
        match self.text(name).map(str::to_ascii_lowercase).as_deref() {
            None | Some("false" | "0" | "off" | "no") => Ok(false),
            Some("true" | "1" | "on" | "yes") => Ok(true),
            Some(other) => Err(AppError::bad_request(format!("{name} can't be {other}"))),
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(boundary: &str, parts: &[(&str, Option<&str>, &[u8])]) -> Vec<u8> {
        let mut body = b"preamble\r\n".to_vec();
        for (name, filename, data) in parts {
            body.extend(format!("--{boundary}\r\n").bytes());
            match filename {
                Some(f) => body.extend(
                    format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/x-subrip\r\n\r\n")
                        .bytes(),
                ),
                None => body.extend(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").bytes()),
            }
            body.extend_from_slice(data);
            body.extend(b"\r\n");
        }
        body.extend(format!("--{boundary}--\r\n").bytes());
        body
    }

    #[test]
    fn forms_are_read() {
        let srt = b"1\r\n00:00:01,000 --> 00:00:02,000\r\nHi\r\n\r\n--not the boundary\r\n";
        let body = form(
            "----x7",
            &[
                ("file", Some("C:\\Users\\me\\Movie.en.srt"), srt),
                ("language", None, b" fr "),
                ("forced", None, b"true"),
            ],
        );
        let read = Form::parse("multipart/form-data; boundary=\"----x7\"", &body).unwrap();
        assert_eq!(read.file("file"), Some(("Movie.en.srt", srt.as_slice())));
        assert_eq!(read.text("language"), Some("fr"));
        assert!(read.flag("forced").unwrap());
        assert!(!read.flag("hearing_impaired").unwrap());
        assert_eq!(read.file("language"), None, "not a file");
        assert!(Form::parse("multipart/form-data", &body).is_err(), "no boundary");
        assert!(Form::parse("multipart/form-data; boundary=other", &body).is_err());
        assert!(Form::parse("multipart/form-data; boundary=----x7", b"--").is_err());
        let truncated = &body[..body.len() - 20];
        assert!(Form::parse("multipart/form-data; boundary=----x7", truncated).is_err());
        let maybe = form("b", &[("forced", None, b"maybe")]);
        let yes_no = Form::parse("multipart/form-data; boundary=b", &maybe).unwrap();
        assert!(yes_no.flag("forced").is_err());
    }
}
