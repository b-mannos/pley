//! Accounts and sessions: Argon2id password hashes, random session tokens stored
//! hashed (SHA-256) in the database, a short-lived in-memory cache so that
//! segment/image requests don't hit SQLite, and axum extractors.

use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use base64::Engine;
use dashmap::DashMap;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now;

pub const COOKIE_NAME: &str = "pley_token";
pub const SESSION_TTL_SECS: i64 = 90 * 24 * 3600;
const CACHE_TTL: Duration = Duration::from_secs(60);
const TOUCH_INTERVAL_SECS: i64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    /// Every library, or only the ones in `user_libraries` (see [`crate::access`]).
    pub all_libraries: bool,
    /// Age limit for movies and shows; `None` = unrestricted.
    pub max_age: Option<i64>,
}

impl AuthUser {
    /// A user acting with all rights the account has, for server-side work.
    pub async fn load(db: &Db, id: i64) -> sqlx::Result<Option<AuthUser>> {
        let row: Option<(i64, String, bool, bool, Option<i64>)> =
            sqlx::query_as("SELECT id, username, is_admin, all_libraries, max_age FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&db.read)
                .await?;
        Ok(row.map(|(id, username, is_admin, all_libraries, max_age)| AuthUser {
            id,
            username,
            is_admin,
            all_libraries,
            max_age: if is_admin { None } else { max_age },
        }))
    }
}

/// Extractor that additionally requires administrator rights.
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

pub async fn hash_password(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| anyhow::anyhow!("hashing password: {e}"))
    })
    .await?
}

pub async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    })
    .await
    .unwrap_or(false)
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn session_cookie(token: &str) -> String {
    format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECS}")
}

pub fn clear_cookie() -> String {
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

pub fn validate_username(name: &str) -> Result<(), AppError> {
    let ok = (1..=64).contains(&name.chars().count())
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'));
    if ok { Ok(()) } else { Err(AppError::bad_request("username must be 1-64 letters, digits or _ - . @")) }
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.chars().count() < 4 {
        return Err(AppError::bad_request("password must be at least 4 characters"));
    }
    Ok(())
}

struct CachedSession {
    user: AuthUser,
    expires_at: i64,
    cached_at: Instant,
    touched_at: i64,
}

pub struct AuthService {
    db: Db,
    cache: DashMap<String, CachedSession>,
    /// Failed password and PIN attempts.
    pub throttle: crate::accounts::Throttle,
}

impl AuthService {
    pub fn new(db: Db) -> Self {
        Self { db, cache: DashMap::new(), throttle: Default::default() }
    }

    pub async fn create_session(
        &self,
        user_id: i64,
        user_agent: Option<&str>,
        ip: Option<&str>,
    ) -> Result<String, AppError> {
        let token = new_token();
        let now = now();
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, created_at, last_seen_at, expires_at, user_agent, last_ip)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(hash_token(&token))
        .bind(user_id)
        .bind(now)
        .bind(now)
        .bind(now + SESSION_TTL_SECS)
        .bind(user_agent.map(|ua| ua.chars().take(200).collect::<String>()))
        .bind(ip)
        .execute(&self.db.write)
        .await?;
        Ok(token)
    }

    pub async fn validate(&self, token: &str) -> Option<AuthUser> {
        let key = hash_token(token);
        let now = now();
        if let Some(entry) = self.cache.get(&key) {
            if entry.cached_at.elapsed() < CACHE_TTL && entry.expires_at > now {
                return Some(entry.user.clone());
            }
        }
        #[allow(clippy::type_complexity)]
        let row: Option<(i64, String, bool, bool, Option<i64>, bool, i64, i64)> = sqlx::query_as(
            "SELECT u.id, u.username, u.is_admin, u.all_libraries, u.max_age, u.disabled, s.expires_at, s.last_seen_at
             FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?",
        )
        .bind(&key)
        .fetch_optional(&self.db.read)
        .await
        .ok()?;
        let Some((id, username, is_admin, all_libraries, max_age, disabled, expires_at, last_seen_at)) = row else {
            self.cache.remove(&key);
            return None;
        };
        if expires_at <= now || disabled {
            self.cache.remove(&key);
            return None;
        }
        let user = AuthUser { id, username, is_admin, all_libraries, max_age: if is_admin { None } else { max_age } };
        let mut touched_at = last_seen_at;
        if now - last_seen_at > TOUCH_INTERVAL_SECS {
            // Sliding expiration, written at most every few minutes.
            let _ = sqlx::query("UPDATE sessions SET last_seen_at = ?, expires_at = ? WHERE token_hash = ?")
                .bind(now)
                .bind(now + SESSION_TTL_SECS)
                .bind(&key)
                .execute(&self.db.write)
                .await;
            touched_at = now;
        }
        self.cache.insert(key, CachedSession { user: user.clone(), expires_at, cached_at: Instant::now(), touched_at });
        Some(user)
    }

    pub async fn revoke(&self, token: &str) {
        let key = hash_token(token);
        self.cache.remove(&key);
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(key).execute(&self.db.write).await;
    }

    /// Drop cached sessions of a user (password change, deletion, role change).
    pub fn forget_user(&self, user_id: i64) {
        self.cache.retain(|_, s| s.user.id != user_id);
    }

    pub async fn purge_expired(&self) {
        let _ = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?").bind(now()).execute(&self.db.write).await;
        let now = now();
        self.cache.retain(|_, s| s.expires_at > now && s.touched_at > 0);
    }
}

/// Token from `Authorization: Bearer`, `X-Pley-Token`, the session cookie, or `?token=`.
pub fn token_from_parts(parts: &Parts) -> Option<String> {
    let headers = &parts.headers;
    if let Some(v) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = v.strip_prefix("Bearer ") {
            return Some(token.trim().to_owned());
        }
    }
    if let Some(v) = headers.get("x-pley-token").and_then(|v| v.to_str().ok()) {
        return Some(v.trim().to_owned());
    }
    for cookie_header in headers.get_all(header::COOKIE) {
        let Ok(cookies) = cookie_header.to_str() else { continue };
        for pair in cookies.split(';') {
            if let Some((name, value)) = pair.trim().split_once('=') {
                if name == COOKIE_NAME && !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    parts.uri.query().and_then(|q| {
        q.split('&').find_map(|kv| kv.strip_prefix("token=").map(|t| t.to_owned())).filter(|t| !t.is_empty())
    })
}

/// The session token of the request, if any (to mark "this device").
pub struct CurrentToken(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for CurrentToken {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(CurrentToken(token_from_parts(parts)))
    }
}

/// The client's address as seen by the server (for the devices list).
pub struct ClientIp(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let forwarded = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        let peer = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.ip().to_string());
        Ok(ClientIp(forwarded.or(peer)))
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        if let Some(user) = parts.extensions.get::<AuthUser>() {
            return Ok(user.clone());
        }
        let token = token_from_parts(parts).ok_or(AppError::Unauthorized)?;
        let user = state.auth.validate(&token).await.ok_or(AppError::Unauthorized)?;
        parts.extensions.insert(user.clone());
        Ok(user)
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(AppError::forbidden("administrator rights required"));
        }
        Ok(AdminUser(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn password_round_trip() {
        let hash = hash_password("hunter22".into()).await.unwrap();
        assert!(verify_password("hunter22".into(), hash.clone()).await);
        assert!(!verify_password("wrong".into(), hash).await);
    }

    #[test]
    fn tokens_are_random_and_hashable() {
        let a = new_token();
        let b = new_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(hash_token(&a).len(), 64);
    }

    #[test]
    fn extracts_tokens() {
        let req = axum::http::Request::builder()
            .uri("/api/items?x=1&token=abc")
            .header("cookie", "foo=bar; pley_token=cookie-token")
            .body(())
            .unwrap();
        let (parts, _) = req.into_parts();
        assert_eq!(token_from_parts(&parts).as_deref(), Some("cookie-token"));

        let req = axum::http::Request::builder().uri("/api/items?token=abc").body(()).unwrap();
        let (parts, _) = req.into_parts();
        assert_eq!(token_from_parts(&parts).as_deref(), Some("abc"));

        let req = axum::http::Request::builder().uri("/").header("authorization", "Bearer xyz").body(()).unwrap();
        let (parts, _) = req.into_parts();
        assert_eq!(token_from_parts(&parts).as_deref(), Some("xyz"));
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("bob.smith@home").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("bad name").is_err());
    }
}
