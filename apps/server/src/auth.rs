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

/// Password hashes made or checked at once. Each takes 19 MiB and a core
/// for a moment (Argon2id), so a flood of sign-ins queues instead of
/// exhausting memory.
const HASHING_AT_ONCE: usize = 4;
/// How long a password waits its turn before pley says it is busy.
const HASHING_WAIT: Duration = Duration::from_secs(10);

/// Runs Argon2 `work` when its turn comes.
async fn hashing<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, AppError> {
    static TURNS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(HASHING_AT_ONCE);
    let _turn = tokio::time::timeout(HASHING_WAIT, TURNS.acquire())
        .await
        .map_err(|_| AppError::Unavailable("pley is busy checking passwords: try again in a moment".into()))?
        .map_err(|e| AppError::Internal(e.into()))?;
    tokio::task::spawn_blocking(work).await.map_err(|e| AppError::Internal(e.into()))
}

pub async fn hash_password(password: String) -> Result<String, AppError> {
    hashing(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AppError::Internal(anyhow::anyhow!("hashing password: {e}")))
    })
    .await?
}

/// A real Argon2 hash of a random password, made once: checking a login for
/// an unknown user against it costs the same as checking a real one.
pub async fn dummy_password_hash() -> String {
    static HASH: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    HASH.get_or_init(|| async {
        let mut secret = [0u8; 16];
        OsRng.fill_bytes(&mut secret);
        hash_password(hex::encode(secret)).await.unwrap_or_default()
    })
    .await
    .clone()
}

/// Whether `password` matches `hash` (never for the markers of accounts
/// without a password); an error when pley is too busy to check.
pub async fn verify_password(password: String, hash: String) -> Result<bool, AppError> {
    hashing(move || {
        PasswordHash::new(&hash)
            .is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    })
    .await
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// The session cookie. `secure` (the site is reached over HTTPS, e.g. through
/// a TLS proxy) keeps browsers from ever sending it over plain HTTP.
pub fn session_cookie(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECS}{secure}")
}

/// Whether the proxy in front of pley received the request over HTTPS.
pub fn forwarded_https(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"))
}

pub fn clear_cookie() -> String {
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

pub fn validate_username(name: &str) -> Result<(), AppError> {
    let ok = (1..=64).contains(&name.chars().count())
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'));
    if ok { Ok(()) } else { Err(AppError::bad_request("username must be 1-64 letters, digits or _ - . @")) }
}

/// Shortest password accepted (NIST SP 800-63B's minimum).
pub const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_CHARS: usize = 256;

/// Whether `password` may become the password of `username`: long enough,
/// not the username, not one character over and over, and not one of the
/// passwords tried first by anyone guessing.
pub fn validate_password(password: &str, username: &str) -> Result<(), AppError> {
    let chars = password.chars().count();
    if chars < MIN_PASSWORD_CHARS {
        return Err(AppError::bad_request(format!("use at least {MIN_PASSWORD_CHARS} characters for the password")));
    }
    if chars > MAX_PASSWORD_CHARS {
        return Err(AppError::bad_request(format!("use at most {MAX_PASSWORD_CHARS} characters for the password")));
    }
    let lower = password.to_lowercase();
    if lower == username.trim().to_lowercase() {
        return Err(AppError::bad_request("the password can't be the username"));
    }
    let mut letters = lower.chars();
    let first = letters.next();
    if letters.all(|c| Some(c) == first) || COMMON_PASSWORDS.contains(&lower.as_str()) {
        return Err(AppError::bad_request(
            "that password is too easy to guess: try a few words that don't go together",
        ));
    }
    Ok(())
}

/// Very common passwords of 8 characters or more (shorter ones are refused
/// anyway), lowercase.
const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password1",
    "password12",
    "password123",
    "password1234",
    "password!",
    "passw0rd",
    "p@ssw0rd",
    "p@ssword",
    "mypassword",
    "12345678",
    "123456789",
    "1234567890",
    "12345678910",
    "87654321",
    "987654321",
    "0987654321",
    "12341234",
    "11223344",
    "12121212",
    "123123123",
    "123321123",
    "1234qwer",
    "qwer1234",
    "1q2w3e4r",
    "1q2w3e4r5t",
    "1q2w3e4r5t6y",
    "q1w2e3r4",
    "q1w2e3r4t5",
    "1qaz2wsx",
    "1qazxsw2",
    "zaq12wsx",
    "qazwsxedc",
    "123qweasd",
    "qwertyui",
    "qwertyuiop",
    "qwerty12",
    "qwerty123",
    "qwerty1234",
    "asdfghjk",
    "asdfghjkl",
    "asdf1234",
    "asdfasdf",
    "zxcvbnm1",
    "zxcvbnm123",
    "abcd1234",
    "1234abcd",
    "abc12345",
    "abcdefgh",
    "abcdefg1",
    "aa123456",
    "a1234567",
    "a12345678",
    "123456789a",
    "12345678a",
    "a123456789",
    "iloveyou",
    "iloveyou1",
    "iloveyou2",
    "loveyou1",
    "sunshine",
    "sunshine1",
    "princess",
    "princess1",
    "football",
    "football1",
    "baseball",
    "basketball",
    "superman",
    "batman123",
    "trustno1",
    "welcome1",
    "welcome123",
    "letmein1",
    "letmein123",
    "starwars",
    "whatever",
    "dragon123",
    "master123",
    "computer",
    "michelle",
    "jennifer",
    "charlie1",
    "jordan23",
    "shadow12",
    "monkey123",
    "mustang1",
    "liverpool",
    "chelsea1",
    "arsenal1",
    "manchester",
    "internet",
    "samsung1",
    "google123",
    "freedom1",
    "pokemon1",
    "minecraft",
    "fortnite",
    "butterfly",
    "chocolate",
    "babygirl1",
    "admin123",
    "admin1234",
    "administrator",
    "changeme",
    "changeme1",
    "default1",
    "secret123",
    "pley1234",
    "pleypley",
    "plexpass",
    "plex1234",
    "jellyfin",
    "mediaserver",
];

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
    /// The code that sets up a server without accounts from elsewhere.
    pub setup: crate::setup::SetupCode,
}

impl AuthService {
    pub fn new(db: Db) -> Self {
        Self { db, cache: DashMap::new(), throttle: Default::default(), setup: Default::default() }
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
    header_token(parts).or_else(|| cookie_token(parts)).or_else(|| query_token(parts))
}

/// A token the client sent on purpose (header or `?token=`), never the
/// browser's cookie: what listeners without CSRF protection accept.
pub fn token_without_cookie(parts: &Parts) -> Option<String> {
    header_token(parts).or_else(|| query_token(parts))
}

fn header_token(parts: &Parts) -> Option<String> {
    let headers = &parts.headers;
    if let Some(v) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = v.strip_prefix("Bearer ") {
            return Some(token.trim().to_owned());
        }
    }
    headers.get("x-pley-token").and_then(|v| v.to_str().ok()).map(|v| v.trim().to_owned())
}

fn cookie_token(parts: &Parts) -> Option<String> {
    for cookie_header in parts.headers.get_all(header::COOKIE) {
        let Ok(cookies) = cookie_header.to_str() else { continue };
        for pair in cookies.split(';') {
            if let Some((name, value)) = pair.trim().split_once('=') {
                if name == COOKIE_NAME && !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}

fn query_token(parts: &Parts) -> Option<String> {
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
        assert!(verify_password("hunter22".into(), hash.clone()).await.unwrap());
        assert!(!verify_password("wrong".into(), hash).await.unwrap());
        assert!(!verify_password(String::new(), "!sso".into()).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_flood_of_passwords_queues() {
        // Many at once (an attacker trying usernames): at most a few hashes
        // in memory at any moment, and every check still answers.
        let hash = hash_password("hunter22".into()).await.unwrap();
        let checks = (0..24).map(|_| verify_password("hunter22".into(), hash.clone()));
        let results = futures::future::join_all(checks).await;
        assert!(results.into_iter().all(|r| r.unwrap()));
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
        assert!(validate_username("Zoë").is_ok(), "letters of any script");
        assert!(validate_username(&"a".repeat(64)).is_ok());
        assert!(validate_username(&"a".repeat(65)).is_err());
        assert!(validate_username("a/b").is_err());
        assert!(validate_username("<script>").is_err());
    }

    #[test]
    fn passwords_are_long_and_hard_to_guess() {
        assert!(validate_password("correct horse", "bob").is_ok());
        assert!(validate_password("seven77", "bob").is_err(), "too short");
        assert!(validate_password("日本語の言葉です", "bob").is_ok(), "characters, not bytes");
        assert!(validate_password("日本語の言葉", "bob").is_err());
        assert!(validate_password(&"x".repeat(257), "bob").is_err());
        assert!(validate_password("Password1", "bob").is_err(), "a common one, in any case");
        assert!(validate_password("qwertyuiop", "bob").is_err());
        assert!(validate_password("zzzzzzzzzz", "bob").is_err(), "one character over and over");
        assert!(validate_password("Bob.Smith", "bob.smith").is_err(), "the username");
        for common in COMMON_PASSWORDS {
            assert!(common.chars().count() >= MIN_PASSWORD_CHARS, "{common} is refused for its length anyway");
            assert_eq!(*common, common.to_lowercase());
        }
    }

    fn token_of(builder: axum::http::request::Builder) -> Option<String> {
        let (parts, _) = builder.body(()).unwrap().into_parts();
        token_from_parts(&parts)
    }

    #[test]
    fn token_sources_in_order_of_precedence() {
        let all = axum::http::Request::builder()
            .uri("/api/x?token=query")
            .header("cookie", "pley_token=cookie")
            .header("x-pley-token", "header")
            .header("authorization", "Bearer bearer");
        assert_eq!(token_of(all).as_deref(), Some("bearer"));
        let no_bearer = axum::http::Request::builder()
            .uri("/api/x?token=query")
            .header("cookie", "pley_token=cookie")
            .header("x-pley-token", " header ")
            .header("authorization", "Basic dXNlcjpwYXNz");
        assert_eq!(token_of(no_bearer).as_deref(), Some("header"), "other schemes are not tokens");
        let cookies = axum::http::Request::builder()
            .uri("/api/x?token=query")
            .header("cookie", "theme=dark")
            .header("cookie", "a=b;   pley_token=cookie; c=d");
        assert_eq!(token_of(cookies).as_deref(), Some("cookie"), "any cookie header, any position");
        let empty_cookie =
            axum::http::Request::builder().uri("/api/x?a=1&token=query").header("cookie", "pley_token=; x=y");
        assert_eq!(token_of(empty_cookie).as_deref(), Some("query"), "a cleared cookie is no token");
        assert_eq!(token_of(axum::http::Request::builder().uri("/api/x?token=")), None);
        assert_eq!(token_of(axum::http::Request::builder().uri("/api/x?mytoken=abc")), None);
        assert_eq!(token_of(axum::http::Request::builder().uri("/api/x")), None);
    }

    #[test]
    fn cookies_are_http_only_and_last_as_long_as_sessions() {
        let cookie = session_cookie("tok", false);
        assert!(cookie.starts_with("pley_token=tok;"));
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax") && cookie.contains("Path=/"));
        assert!(cookie.contains(&format!("Max-Age={SESSION_TTL_SECS}")));
        assert!(!cookie.contains("Secure") && session_cookie("tok", true).ends_with("; Secure"));
        let mut headers = axum::http::HeaderMap::new();
        assert!(!forwarded_https(&headers));
        headers.insert("x-forwarded-proto", "https, http".parse().unwrap());
        assert!(forwarded_https(&headers));
        assert!(clear_cookie().contains("pley_token=;") && clear_cookie().contains("Max-Age=0"));
    }

    async fn service_with_user(dir: &std::path::Path) -> (Db, AuthService) {
        let db = Db::open(&dir.join("pley.db")).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, all_libraries, max_age, created_at)
             VALUES (1, 'kid', 'x', 0, 0, 12, 0), (2, 'admin', 'x', 1, 0, 12, 0)",
        )
        .execute(&db.write)
        .await
        .unwrap();
        let service = AuthService::new(db.clone());
        (db, service)
    }

    #[tokio::test]
    async fn sessions_carry_the_users_restrictions() {
        let dir = tempfile::tempdir().unwrap();
        let (_db, auth) = service_with_user(dir.path()).await;
        let token = auth.create_session(1, Some("agent"), Some("10.0.0.2")).await.unwrap();
        let user = auth.validate(&token).await.unwrap();
        assert_eq!(
            user,
            AuthUser { id: 1, username: "kid".into(), is_admin: false, all_libraries: false, max_age: Some(12) }
        );
        let admin = auth.create_session(2, None, None).await.unwrap();
        let admin = auth.validate(&admin).await.unwrap();
        assert_eq!(admin.max_age, None, "administrators have no age limit");
        assert!(auth.validate("not-a-token").await.is_none());
    }

    #[tokio::test]
    async fn expired_and_disabled_sessions_stop_working() {
        let dir = tempfile::tempdir().unwrap();
        let (db, auth) = service_with_user(dir.path()).await;
        let expired = auth.create_session(1, None, None).await.unwrap();
        sqlx::query("UPDATE sessions SET expires_at = ? WHERE token_hash = ?")
            .bind(now() - 1)
            .bind(hash_token(&expired))
            .execute(&db.write)
            .await
            .unwrap();
        assert!(auth.validate(&expired).await.is_none());
        auth.purge_expired().await;
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions").fetch_one(&db.read).await.unwrap();
        assert_eq!(left, 0, "expired sessions are purged");

        // A cached session survives a change in the database until the
        // user is forgotten (what the users API does on every change).
        let token = auth.create_session(1, None, None).await.unwrap();
        assert!(auth.validate(&token).await.is_some());
        sqlx::query("UPDATE users SET disabled = 1 WHERE id = 1").execute(&db.write).await.unwrap();
        assert!(auth.validate(&token).await.is_some(), "served from the cache");
        auth.forget_user(1);
        assert!(auth.validate(&token).await.is_none(), "disabled users are signed out");

        // Revoking works whether or not the session is cached.
        sqlx::query("UPDATE users SET disabled = 0 WHERE id = 1").execute(&db.write).await.unwrap();
        let token = auth.create_session(1, None, None).await.unwrap();
        assert!(auth.validate(&token).await.is_some());
        auth.revoke(&token).await;
        assert!(auth.validate(&token).await.is_none());
    }

    #[tokio::test]
    async fn sessions_in_use_are_extended() {
        let dir = tempfile::tempdir().unwrap();
        let (db, auth) = service_with_user(dir.path()).await;
        let token = auth.create_session(1, None, None).await.unwrap();
        let stale = now() - TOUCH_INTERVAL_SECS - 60;
        sqlx::query("UPDATE sessions SET last_seen_at = ?, expires_at = ? WHERE token_hash = ?")
            .bind(stale)
            .bind(now() + 60)
            .bind(hash_token(&token))
            .execute(&db.write)
            .await
            .unwrap();
        assert!(auth.validate(&token).await.is_some());
        let (last_seen, expires): (i64, i64) =
            sqlx::query_as("SELECT last_seen_at, expires_at FROM sessions WHERE token_hash = ?")
                .bind(hash_token(&token))
                .fetch_one(&db.read)
                .await
                .unwrap();
        assert!(last_seen > stale);
        assert!(expires >= now() + SESSION_TTL_SECS - 5, "a full lifetime again");
    }
}
