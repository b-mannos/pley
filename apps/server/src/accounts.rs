//! Accounts: profile data, the users list as the API returns it, PIN and
//! password attempt throttling, avatars, device names and invitation codes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use image::codecs::jpeg::JpegEncoder;
use rand_core::{OsRng, RngCore};
use serde::Serialize;

use crate::db::Db;
use crate::error::AppError;

/// Stored instead of a password hash for managed profiles: never verifies.
pub const NO_PASSWORD: &str = "!managed";
const AVATAR_SIZE: u32 = 256;

#[derive(Debug, Clone, sqlx::FromRow)]
struct UserRow {
    id: i64,
    username: String,
    display_name: Option<String>,
    is_admin: bool,
    managed: bool,
    home: bool,
    has_pin: bool,
    avatar: Option<String>,
    avatar_color: Option<String>,
    all_libraries: bool,
    max_age: Option<i64>,
    disabled: bool,
    created_at: i64,
}

/// A user as the API shows it.
#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    pub id: i64,
    pub username: String,
    /// What to show: the display name, or the username.
    pub name: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    /// A profile without a password, reached by switching from a home member.
    pub managed: bool,
    /// Listed in the profile switcher of the household.
    pub home: bool,
    pub has_pin: bool,
    pub avatar_url: Option<String>,
    pub avatar_color: Option<String>,
    pub all_libraries: bool,
    pub library_ids: Vec<i64>,
    pub max_age: Option<i64>,
    pub disabled: bool,
    pub created_at: i64,
}

const USER_COLUMNS: &str = "id, username, display_name, is_admin, managed, home, pin_hash IS NOT NULL AS has_pin,
    avatar, avatar_color, all_libraries, max_age, disabled, created_at";

impl UserRow {
    fn into_dto(self, library_ids: Vec<i64>) -> UserDto {
        UserDto {
            name: self.display_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.username.clone()),
            avatar_url: self.avatar.as_ref().map(|a| format!("/api/users/{}/avatar?v={}", self.id, version_of(a))),
            id: self.id,
            username: self.username,
            display_name: self.display_name,
            is_admin: self.is_admin,
            managed: self.managed,
            home: self.home,
            has_pin: self.has_pin,
            avatar_color: self.avatar_color,
            all_libraries: self.all_libraries || self.is_admin,
            library_ids,
            max_age: if self.is_admin { None } else { self.max_age },
            disabled: self.disabled,
            created_at: self.created_at,
        }
    }
}

/// `"7-1734567890.jpg"` → `"1734567890"`, for cache-busting avatar URLs.
fn version_of(file: &str) -> &str {
    file.rsplit_once('-').map(|(_, v)| v.trim_end_matches(".jpg")).unwrap_or(file)
}

pub async fn user(db: &Db, id: i64) -> sqlx::Result<Option<UserDto>> {
    let row: Option<UserRow> = sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?"))
        .bind(id)
        .fetch_optional(&db.read)
        .await?;
    let Some(row) = row else { return Ok(None) };
    let ids = sqlx::query_scalar("SELECT library_id FROM user_libraries WHERE user_id = ? ORDER BY library_id")
        .bind(id)
        .fetch_all(&db.read)
        .await?;
    Ok(Some(row.into_dto(ids)))
}

pub async fn users(db: &Db) -> sqlx::Result<Vec<UserDto>> {
    let rows: Vec<UserRow> = sqlx::query_as(&format!(
        "SELECT {USER_COLUMNS} FROM users ORDER BY managed, COALESCE(display_name, username) COLLATE NOCASE"
    ))
    .fetch_all(&db.read)
    .await?;
    let pairs: Vec<(i64, i64)> = sqlx::query_as("SELECT user_id, library_id FROM user_libraries ORDER BY library_id")
        .fetch_all(&db.read)
        .await?;
    let mut by_user: HashMap<i64, Vec<i64>> = HashMap::new();
    for (user, library) in pairs {
        by_user.entry(user).or_default().push(library);
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let libraries = by_user.remove(&r.id).unwrap_or_default();
            r.into_dto(libraries)
        })
        .collect())
}

/// Replace a user's library list.
pub async fn set_libraries(db: &Db, user_id: i64, library_ids: &[i64]) -> sqlx::Result<()> {
    let mut tx = db.write.begin().await?;
    sqlx::query("DELETE FROM user_libraries WHERE user_id = ?").bind(user_id).execute(&mut *tx).await?;
    for id in library_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO user_libraries (user_id, library_id) SELECT ?, id FROM libraries WHERE id = ?",
        )
        .bind(user_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub fn validate_pin(pin: &str) -> Result<(), AppError> {
    if (4..=8).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(AppError::bad_request("the PIN must be 4 to 8 digits"))
    }
}

pub fn validate_color(color: &str) -> Result<(), AppError> {
    let hex = color.strip_prefix('#').unwrap_or_default();
    if matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(AppError::bad_request("the color must look like #38bdf8"))
    }
}

// ------------------------------------------------------------------ throttling

/// Failed password / PIN attempts per key (username, profile id). After
/// `MAX_FAILURES` failures within the window, attempts are refused until it
/// ends, which makes guessing a 4-digit PIN impractical.
#[derive(Default)]
pub struct Throttle {
    failures: DashMap<String, (u32, Instant)>,
}

const MAX_FAILURES: u32 = 5;
const WINDOW: Duration = Duration::from_secs(15 * 60);

impl Throttle {
    /// `Err` with the remaining wait while `key` is locked out.
    pub fn check(&self, key: &str) -> Result<(), AppError> {
        if let Some(entry) = self.failures.get(key) {
            let (count, since) = *entry;
            let elapsed = since.elapsed();
            if count >= MAX_FAILURES && elapsed < WINDOW {
                let minutes = (WINDOW - elapsed).as_secs().div_ceil(60);
                return Err(AppError::TooManyRequests(format!("too many attempts, try again in {minutes} min")));
            }
        }
        Ok(())
    }

    pub fn failed(&self, key: &str) {
        let mut entry = self.failures.entry(key.to_owned()).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }

    pub fn succeeded(&self, key: &str) {
        self.failures.remove(key);
    }
}

// ------------------------------------------------------------------ devices

/// "Chrome on Windows", "Safari on iPhone", or the raw agent for other clients.
pub fn device_name(user_agent: Option<&str>) -> String {
    let Some(ua) = user_agent.filter(|u| !u.is_empty()) else { return "Unknown device".into() };
    let browser = if ua.contains("Edg/") {
        Some("Edge")
    } else if ua.contains("OPR/") || ua.contains("Opera") {
        Some("Opera")
    } else if ua.contains("Firefox/") {
        Some("Firefox")
    } else if ua.contains("Chrome/") || ua.contains("CriOS/") {
        Some("Chrome")
    } else if ua.contains("Safari/") {
        Some("Safari")
    } else {
        None
    };
    let os = if ua.contains("iPhone") {
        Some("iPhone")
    } else if ua.contains("iPad") {
        Some("iPad")
    } else if ua.contains("Android") {
        Some("Android")
    } else if ua.contains("Windows") {
        Some("Windows")
    } else if ua.contains("Mac OS X") || ua.contains("Macintosh") {
        Some("macOS")
    } else if ua.contains("CrOS") {
        Some("ChromeOS")
    } else if ua.contains("Linux") {
        Some("Linux")
    } else {
        None
    };
    match (browser, os) {
        (Some(b), Some(o)) => format!("{b} on {o}"),
        (Some(b), None) => b.to_owned(),
        _ => ua.split_whitespace().next().unwrap_or(ua).chars().take(60).collect(),
    }
}

// ------------------------------------------------------------------ avatars

pub fn avatars_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("avatars")
}

/// Center-crop and scale an uploaded picture to a square JPEG.
pub fn process_avatar(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    let img =
        image::load_from_memory(bytes).map_err(|_| AppError::bad_request("that file is not a supported picture"))?;
    let side = img.width().min(img.height());
    let (x, y) = ((img.width() - side) / 2, (img.height() - side) / 2);
    let square = img.crop_imm(x, y, side, side);
    let resized = square.resize_exact(AVATAR_SIZE, AVATAR_SIZE, image::imageops::FilterType::Lanczos3).to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&resized)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("encoding avatar: {e}")))?;
    Ok(out)
}

// ------------------------------------------------------------------ invites

/// Short, unambiguous invitation code (no 0/O, 1/l/I).
pub fn invite_code() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut bytes = [0u8; 12];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names() {
        let chrome_win = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";
        assert_eq!(device_name(Some(chrome_win)), "Chrome on Windows");
        let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36 Edg/140.0";
        assert_eq!(device_name(Some(edge)), "Edge on Windows");
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1";
        assert_eq!(device_name(Some(iphone)), "Safari on iPhone");
        assert_eq!(device_name(Some("python-requests/2.32")), "python-requests/2.32");
        assert_eq!(device_name(None), "Unknown device");
    }

    #[test]
    fn throttle_locks_after_repeated_failures() {
        let t = Throttle::default();
        for _ in 0..MAX_FAILURES {
            assert!(t.check("pin:7").is_ok());
            t.failed("pin:7");
        }
        assert!(t.check("pin:7").is_err());
        assert!(t.check("pin:8").is_ok());
        t.succeeded("pin:7");
        assert!(t.check("pin:7").is_ok());
    }

    #[test]
    fn pins_colors_and_codes() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("12a4").is_err());
        assert!(validate_pin("123").is_err());
        assert!(validate_color("#38bdf8").is_ok());
        assert!(validate_color("red").is_err());
        let code = invite_code();
        assert_eq!(code.len(), 12);
        assert!(!code.contains(['0', 'o', '1', 'l', 'i']));
    }

    #[test]
    fn avatars_are_square_jpegs() {
        let img = image::RgbImage::from_pixel(400, 200, image::Rgb([56, 189, 248]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let jpeg = process_avatar(&png).unwrap();
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (AVATAR_SIZE, AVATAR_SIZE));
        assert!(process_avatar(b"not an image").is_err());
    }
}
