//! Two-step sign-in: after the password, a code from an authenticator app
//! (TOTP, RFC 6238: HMAC-SHA1, 6 digits, 30-second steps, which every app
//! reads) or one of the account's ten recovery codes.
//!
//! * The authenticator secrets are sealed (ChaCha20-Poly1305) with a key kept
//!   next to the database in `<data>/secret.key`: a copy of the database or
//!   of its backups can't make codes.
//! * Recovery codes have 80 random bits and are stored as SHA-256 hashes, so
//!   they still sign in when that key is lost.
//! * A code works once (the step of the last one accepted is kept). Wrong
//!   codes are throttled like passwords, and after 100 in a row only a
//!   recovery code signs in (NIST SP 800-63B, 5.2.2).
//! * A secret shown for setting up stays in memory until a code from it
//!   turns two-step sign-in on.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use dashmap::DashMap;
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::hmac;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::accounts::Throttle;
use crate::db::Db;
use crate::error::AppError;
use crate::util::now;

/// Seconds per code.
const STEP_SECS: i64 = 30;
/// Steps before and after the current one whose codes still work: clocks
/// drift, and typing a code takes a few seconds.
const DRIFT_STEPS: i64 = 1;
const DIGITS: usize = 6;
/// 160 bits, as RFC 4226 recommends.
const SECRET_BYTES: usize = 20;
/// Wrong codes in a row after which only recovery codes sign in.
pub const MAX_FAILURES_IN_A_ROW: i64 = 100;
const RECOVERY_CODES: usize = 10;
/// Characters of a recovery code, 5 bits each.
const RECOVERY_CHARS: usize = 16;
/// Crockford's base32: no I, L, O or U, which pass for other characters.
const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
/// Wrong codes while setting up before the secret shown is dropped.
const SETUP_TRIES: u32 = 5;
/// How long a secret shown for setting up waits for its first code.
pub const SETUP_TTL: Duration = Duration::from_secs(30 * 60);
/// The key's file in the data folder.
pub const KEY_FILE: &str = "secret.key";

const MISSING: &str = "enter the 6-digit code from your authenticator app";
const WRONG: &str = "that code didn't work: enter the newest code from your authenticator app, or a recovery code";
const USED: &str = "that code was used already: wait for the next one";
const LOCKED: &str = "too many wrong codes in a row: sign in with a recovery code, or ask an administrator to turn two-step \
    sign-in off for you";

pub struct TwoStep {
    db: Db,
    /// Seals the authenticator secrets. `None` when `secret.key` can't be
    /// used (the log says why): authenticator codes can't be set up or
    /// checked then, recovery codes still work.
    key: Option<LessSafeKey>,
    /// Secrets shown for setting up, per user, until a code confirms one.
    pending: DashMap<i64, Pending>,
}

struct Pending {
    secret: [u8; SECRET_BYTES],
    since: Instant,
    wrong: u32,
}

/// Two-step sign-in of an account, as its owner sees it.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub enabled: bool,
    /// When it was turned on.
    pub since: Option<i64>,
    pub recovery_codes_left: i64,
}

/// A new secret to add to an authenticator app.
#[derive(Debug, Clone, Serialize)]
pub struct Setup {
    /// Base32, to type in.
    pub secret: String,
    /// `otpauth://totp/…`: what the QR code says, and a link that opens an
    /// authenticator app on the same device.
    pub uri: String,
    pub qr: Qr,
    /// The secret must be confirmed with a code before then.
    pub expires_at: i64,
}

/// A QR code: its width in modules, and an SVG path of its dark modules
/// (a unit square each) for the page to draw.
#[derive(Debug, Clone, Serialize)]
pub struct Qr {
    pub size: i32,
    pub path: String,
}

enum Outcome {
    Right,
    Wrong,
    Used,
}

impl TwoStep {
    pub fn new(db: Db, data_dir: &Path) -> Self {
        let path = data_dir.join(KEY_FILE);
        let key = match load_key(&path) {
            Ok(bytes) => UnboundKey::new(&aead::CHACHA20_POLY1305, &bytes).ok().map(LessSafeKey::new),
            Err(err) => {
                tracing::error!(
                    "can't use {}: {err}. Codes from authenticator apps can't be set up or checked until it can \
                     (recovery codes still work)",
                    path.display()
                );
                None
            }
        };
        Self { db, key, pending: DashMap::new() }
    }

    /// Says in the log when secrets in the database were sealed with another
    /// key than the one in the data folder (a database brought over without
    /// its `secret.key`).
    pub async fn check_key(&self) {
        let Ok(rows) =
            sqlx::query_as::<_, (i64, String)>("SELECT user_id, secret FROM two_step").fetch_all(&self.db.read).await
        else {
            return;
        };
        let unreadable = rows.iter().filter(|(id, sealed)| self.open(*id, sealed).is_none()).count();
        if unreadable > 0 && self.key.is_some() {
            tracing::warn!(
                "{unreadable} account(s) set up two-step sign-in with another {KEY_FILE} than the one in the data \
                 folder: they sign in with a recovery code (or an administrator turns two-step sign-in off for them), \
                 then set it up again"
            );
        }
    }

    pub async fn is_on(&self, user_id: i64) -> sqlx::Result<bool> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM two_step WHERE user_id = ?)")
            .bind(user_id)
            .fetch_one(&self.db.read)
            .await
    }

    pub async fn status(&self, user_id: i64) -> sqlx::Result<Status> {
        let since: Option<i64> = sqlx::query_scalar("SELECT created_at FROM two_step WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&self.db.read)
            .await?;
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recovery_codes WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&self.db.read)
            .await?;
        Ok(Status { enabled: since.is_some(), since, recovery_codes_left: if since.is_some() { left } else { 0 } })
    }

    /// A new secret for `user_id`'s authenticator app, waiting for its first
    /// code ([`TwoStep::finish`]). Apps list it as pley, `username` (at
    /// `server_name`).
    pub fn begin(&self, user_id: i64, username: &str, server_name: &str) -> Result<Setup, AppError> {
        // Saying so now beats failing once the app has the secret.
        self.key()?;
        let mut secret = [0u8; SECRET_BYTES];
        getrandom::fill(&mut secret).expect("the system's random numbers");
        self.pending.retain(|_, p| p.since.elapsed() < SETUP_TTL);
        self.pending.insert(user_id, Pending { secret, since: Instant::now(), wrong: 0 });
        let encoded = base32(&secret);
        let server = server_name.trim();
        let account = if server.is_empty() || server.eq_ignore_ascii_case("pley") {
            username.to_owned()
        } else {
            format!("{username}@{server}")
        };
        let uri = otpauth_uri("Pley", &account, &encoded);
        let qr = qr(&uri).ok_or_else(|| AppError::Internal(anyhow::anyhow!("the QR code of a secret doesn't fit")))?;
        Ok(Setup { secret: encoded, uri, qr, expires_at: now() + SETUP_TTL.as_secs() as i64 })
    }

    /// Turns two-step sign-in on with a code from the secret [`TwoStep::begin`]
    /// showed; the new recovery codes, to show once.
    pub async fn finish(&self, user_id: i64, code: &str) -> Result<Vec<String>, AppError> {
        let expired = || AppError::bad_request("setting up took too long: start again for a new QR code");
        let secret = match self.pending.get(&user_id) {
            Some(p) if p.since.elapsed() < SETUP_TTL => p.secret,
            _ => return Err(expired()),
        };
        let Some(entered) = digits(code) else {
            return Err(AppError::bad_request("enter the 6-digit code your authenticator app shows"));
        };
        let Some(step) = matching_step(&secret, entered, now()) else {
            // A few tries: guessing would hand out recovery codes.
            let mut dropped = false;
            if let Some(mut p) = self.pending.get_mut(&user_id) {
                p.wrong += 1;
                dropped = p.wrong >= SETUP_TRIES;
            }
            if dropped {
                self.pending.remove(&user_id);
                return Err(expired());
            }
            return Err(AppError::bad_request(
                "that code didn't work: check that your phone's clock is right, and enter the newest code",
            ));
        };
        let sealed = self.seal(user_id, &secret)?;
        let codes = new_recovery_codes();
        let mut tx = self.db.write.begin().await?;
        let added = sqlx::query(
            "INSERT INTO two_step (user_id, secret, last_step, created_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(sealed)
        .bind(step)
        .bind(now())
        .execute(&mut *tx)
        .await?;
        if added.rows_affected() == 0 {
            return Err(AppError::conflict("two-step sign-in is on already"));
        }
        store_recovery_codes(&mut tx, user_id, &codes).await?;
        tx.commit().await?;
        self.pending.remove(&user_id);
        Ok(codes)
    }

    /// The second step of a sign-in as `user_id`: nothing when two-step
    /// sign-in is off, else `code` must be a code from the authenticator app
    /// (once) or an unused recovery code.
    pub async fn check(&self, throttle: &Throttle, user_id: i64, code: Option<&str>) -> Result<(), AppError> {
        let row: Option<(String, i64, i64)> =
            sqlx::query_as("SELECT secret, last_step, failures FROM two_step WHERE user_id = ?")
                .bind(user_id)
                .fetch_optional(&self.db.read)
                .await?;
        let Some((sealed, last_step, failures)) = row else { return Ok(()) };
        let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
            return Err(AppError::TwoStep(MISSING.into()));
        };
        let attempt = throttle.begin(&format!("two-step:{user_id}"))?;
        let outcome = if let Some(entered) = digits(code) {
            if failures >= MAX_FAILURES_IN_A_ROW {
                return Err(AppError::TwoStep(LOCKED.into()));
            }
            let secret = self.open(user_id, &sealed).ok_or_else(|| {
                AppError::Unavailable(
                    "this server can't check your authenticator's codes any more (its key changed): sign in with a \
                     recovery code, then set up two-step sign-in again"
                        .into(),
                )
            })?;
            match matching_step(&secret, entered, now()) {
                Some(step) if step > last_step => {
                    // Once only, also against another sign-in with it right now.
                    let used = sqlx::query(
                        "UPDATE two_step SET last_step = ?, failures = 0 WHERE user_id = ? AND last_step < ?",
                    )
                    .bind(step)
                    .bind(user_id)
                    .bind(step)
                    .execute(&self.db.write)
                    .await?;
                    if used.rows_affected() == 1 { Outcome::Right } else { Outcome::Used }
                }
                Some(_) => Outcome::Used,
                None => Outcome::Wrong,
            }
        } else if let Some(normalized) = recovery_code(code) {
            let used = sqlx::query("DELETE FROM recovery_codes WHERE user_id = ? AND code_hash = ?")
                .bind(user_id)
                .bind(recovery_hash(user_id, &normalized))
                .execute(&self.db.write)
                .await?;
            if used.rows_affected() == 1 {
                sqlx::query("UPDATE two_step SET failures = 0 WHERE user_id = ?")
                    .bind(user_id)
                    .execute(&self.db.write)
                    .await?;
                tracing::info!("user {user_id} signed in with a recovery code");
                Outcome::Right
            } else {
                Outcome::Wrong
            }
        } else {
            Outcome::Wrong
        };
        if let Outcome::Right = outcome {
            attempt.succeeded();
            return Ok(());
        }
        attempt.failed();
        let failures: Option<i64> =
            sqlx::query_scalar("UPDATE two_step SET failures = failures + 1 WHERE user_id = ? RETURNING failures")
                .bind(user_id)
                .fetch_optional(&self.db.write)
                .await?;
        if failures == Some(MAX_FAILURES_IN_A_ROW) {
            tracing::warn!(
                "{MAX_FAILURES_IN_A_ROW} wrong two-step codes in a row for user {user_id}: only recovery codes sign in \
                 to it now"
            );
        }
        Err(AppError::TwoStep(if let Outcome::Used = outcome { USED } else { WRONG }.into()))
    }

    /// New recovery codes for `user_id`; the old ones stop working.
    pub async fn new_recovery_codes(&self, user_id: i64) -> Result<Vec<String>, AppError> {
        if !self.is_on(user_id).await? {
            return Err(AppError::bad_request("two-step sign-in is off"));
        }
        let codes = new_recovery_codes();
        let mut tx = self.db.write.begin().await?;
        store_recovery_codes(&mut tx, user_id, &codes).await?;
        tx.commit().await?;
        Ok(codes)
    }

    /// Turns two-step sign-in off for `user_id`; whether it was on.
    pub async fn turn_off(&self, user_id: i64) -> sqlx::Result<bool> {
        self.pending.remove(&user_id);
        let mut tx = self.db.write.begin().await?;
        let removed = sqlx::query("DELETE FROM two_step WHERE user_id = ?").bind(user_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?").bind(user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(removed.rows_affected() > 0)
    }

    fn key(&self) -> Result<&LessSafeKey, AppError> {
        self.key.as_ref().ok_or_else(|| {
            AppError::Unavailable(format!(
                "two-step sign-in with an authenticator app is unavailable: the server can't use its {KEY_FILE} (its \
                 log says why)"
            ))
        })
    }

    fn seal(&self, user_id: i64, secret: &[u8]) -> Result<String, AppError> {
        let key = self.key()?;
        let mut nonce = [0u8; aead::NONCE_LEN];
        getrandom::fill(&mut nonce).expect("the system's random numbers");
        let mut sealed = secret.to_vec();
        key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(aad(user_id)), &mut sealed)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("sealing an authenticator secret failed")))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&sealed);
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    fn open(&self, user_id: i64, sealed: &str) -> Option<Vec<u8>> {
        let key = self.key.as_ref()?;
        let mut nonce = base64::engine::general_purpose::STANDARD.decode(sealed).ok()?;
        if nonce.len() <= aead::NONCE_LEN {
            return None;
        }
        let mut data = nonce.split_off(aead::NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(&nonce).ok()?;
        key.open_in_place(nonce, Aad::from(aad(user_id)), &mut data).ok().map(|plain| plain.to_vec())
    }
}

/// Binds a sealed secret to its account: moved to another, it doesn't open.
fn aad(user_id: i64) -> [u8; 16] {
    let mut aad = *b"two-step\0\0\0\0\0\0\0\0";
    aad[8..].copy_from_slice(&user_id.to_be_bytes());
    aad
}

/// The key in `path`, made on first use. A file that isn't a key is left
/// alone: replacing it would lose every secret sealed with it.
fn load_key(path: &Path) -> Result<[u8; 32], String> {
    match std::fs::read_to_string(path) {
        Ok(text) => hex::decode(text.trim())
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            .ok_or_else(|| "it isn't a key Pley made".to_owned()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).expect("the system's random numbers");
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(path).map_err(|e| e.to_string())?;
            let written = file.write_all(format!("{}\n", hex::encode(key)).as_bytes()).and_then(|()| file.sync_all());
            if let Err(err) = written {
                drop(file);
                let _ = std::fs::remove_file(path);
                return Err(err.to_string());
            }
            Ok(key)
        }
        Err(err) => Err(err.to_string()),
    }
}

/// The code of `secret` for counter `step` (HOTP, RFC 4226).
fn hotp(secret: &[u8], step: u64) -> u32 {
    let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret), &step.to_be_bytes());
    let digest = tag.as_ref();
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let value = u32::from_be_bytes([digest[offset] & 0x7f, digest[offset + 1], digest[offset + 2], digest[offset + 3]]);
    value % 10u32.pow(DIGITS as u32)
}

/// The time step around `at` (unix seconds) whose code is `code`.
fn matching_step(secret: &[u8], code: u32, at: i64) -> Option<i64> {
    let current = at.div_euclid(STEP_SECS);
    (current - DRIFT_STEPS..=current + DRIFT_STEPS)
        .filter(|step| *step >= 0)
        .find(|step| hotp(secret, *step as u64) == code)
}

/// The code an authenticator app shows for `secret` at `at` (unix seconds).
pub fn code_at(secret: &[u8], at: i64) -> String {
    format!("{:0width$}", hotp(secret, at.div_euclid(STEP_SECS).max(0) as u64), width = DIGITS)
}

/// The number in a 6-digit code (`"123 456"` and `"123-456"` too).
fn digits(input: &str) -> Option<u32> {
    let code: String = input.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if code.len() == DIGITS && code.bytes().all(|b| b.is_ascii_digit()) { code.parse().ok() } else { None }
}

/// A recovery code as stored (`"7K2M-Q9XD 4HPT-3VWR"` → `"7k2mq9xd4hpt3vwr"`),
/// reading I and L as 1 and O as 0; `None` when it can't be one.
fn recovery_code(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_lowercase() {
            'i' | 'l' => '1',
            'o' => '0',
            c => c,
        })
        .collect();
    (code.len() == RECOVERY_CHARS && code.bytes().all(|b| RECOVERY_ALPHABET.contains(&b))).then_some(code)
}

fn recovery_hash(user_id: i64, code: &str) -> String {
    hex::encode(Sha256::digest(format!("{user_id}:{code}")))
}

/// Ten codes like `7k2m-q9xd-4hpt-3vwr`.
fn new_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODES)
        .map(|_| {
            let mut bytes = [0u8; RECOVERY_CHARS];
            getrandom::fill(&mut bytes).expect("the system's random numbers");
            let mut code = String::with_capacity(RECOVERY_CHARS + RECOVERY_CHARS / 4);
            for (i, b) in bytes.iter().enumerate() {
                if i > 0 && i % 4 == 0 {
                    code.push('-');
                }
                // 256 is a multiple of 32: every character as likely.
                code.push(char::from(RECOVERY_ALPHABET[usize::from(b & 31)]));
            }
            code
        })
        .collect()
}

/// Replaces `user_id`'s recovery codes with `codes`.
async fn store_recovery_codes(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: i64,
    codes: &[String],
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?").bind(user_id).execute(&mut **tx).await?;
    for code in codes {
        let normalized = recovery_code(code).unwrap_or_default();
        sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?, ?)")
            .bind(user_id)
            .bind(recovery_hash(user_id, &normalized))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 without padding, as authenticator apps take secrets.
pub fn base32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(BASE32[(buffer >> bits) as usize & 31]));
        }
        buffer &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(char::from(BASE32[(buffer << (5 - bits)) as usize & 31]));
    }
    out
}

/// The bytes of a base32 secret (either case, spaces and padding allowed).
pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 5 / 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        if !c.is_ascii() {
            return None;
        }
        let value = BASE32.iter().position(|&b| char::from(b) == c.to_ascii_uppercase())?;
        buffer = (buffer << 5) | value as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The `otpauth://` URI authenticator apps read (Key Uri Format).
fn otpauth_uri(issuer: &str, account: &str, secret: &str) -> String {
    let (issuer, account) = (percent(issuer), percent(account));
    format!(
        "otpauth://totp/{issuer}:{account}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits={DIGITS}&period={STEP_SECS}"
    )
}

/// Percent-encodes all but unreserved characters (RFC 3986).
fn percent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// `text` as a QR code (medium error correction).
fn qr(text: &str) -> Option<Qr> {
    let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
    let size = code.size();
    let mut path = String::new();
    for y in 0..size {
        let mut x = 0;
        while x < size {
            if !code.get_module(x, y) {
                x += 1;
                continue;
            }
            // A run of dark modules is one rectangle.
            let start = x;
            while x < size && code.get_module(x, y) {
                x += 1;
            }
            let _ = write!(path, "M{start} {y}h{}v1h-{}z", x - start, x - start);
        }
    }
    Some(Qr { size, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &[u8] = b"12345678901234567890";

    #[test]
    fn codes_are_the_rfcs() {
        // RFC 4226, appendix D.
        let hotp_codes = [755224, 287082, 359152, 969429, 338314, 254676, 287922, 162583, 399871, 520489];
        for (counter, expected) in hotp_codes.into_iter().enumerate() {
            assert_eq!(hotp(RFC_SECRET, counter as u64), expected);
        }
        // RFC 6238, appendix B (SHA-1), the last 6 of its 8 digits.
        for (at, expected) in [
            (59, "287082"),
            (1111111109, "081804"),
            (1111111111, "050471"),
            (1234567890, "005924"),
            (2000000000, "279037"),
            (20000000000, "353130"),
        ] {
            assert_eq!(code_at(RFC_SECRET, at), expected, "at {at}");
        }
    }

    #[test]
    fn codes_work_a_step_early_or_late() {
        let at = 1_700_000_000;
        let step = at / STEP_SECS;
        let code = |s: i64| hotp(RFC_SECRET, s as u64);
        assert_eq!(matching_step(RFC_SECRET, code(step), at), Some(step));
        assert_eq!(matching_step(RFC_SECRET, code(step - 1), at), Some(step - 1), "typed as it changed");
        assert_eq!(matching_step(RFC_SECRET, code(step + 1), at), Some(step + 1), "a phone a little ahead");
        assert_eq!(matching_step(RFC_SECRET, code(step - 2), at), None);
        assert_eq!(matching_step(RFC_SECRET, code(step + 2), at), None);
    }

    #[test]
    fn codes_are_read_as_people_type_them() {
        assert_eq!(digits("123456"), Some(123456));
        assert_eq!(digits(" 012 345 "), Some(12345));
        assert_eq!(digits("012-345"), Some(12345));
        assert_eq!(digits("12345"), None);
        assert_eq!(digits("1234567"), None);
        assert_eq!(digits("12345a"), None);
        assert_eq!(digits("１２３４５６"), None, "digits of other scripts");
    }

    #[test]
    fn recovery_codes_are_random_and_forgiving() {
        let codes = new_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODES);
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), RECOVERY_CODES);
        for code in &codes {
            assert_eq!(code.len(), 19, "{code}");
            assert_eq!(code.matches('-').count(), 3, "{code}");
            assert_eq!(recovery_code(code).map(|c| c.len()), Some(RECOVERY_CHARS));
            assert_eq!(recovery_code(&code.to_uppercase().replace('-', " ")), recovery_code(code));
        }
        assert_eq!(recovery_code("ILO0-1111-2222-3333").as_deref(), Some("1100111122223333"), "Crockford's reading");
        assert_eq!(recovery_code("uuuu-1111-2222-3333"), None, "not in the alphabet");
        assert_eq!(recovery_code("1111-2222-3333"), None, "too short");
        assert_ne!(recovery_hash(1, "1111222233334444"), recovery_hash(2, "1111222233334444"), "per account");
    }

    #[test]
    fn base32_is_rfc_4648() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "MY"),
            (b"fo", "MZXQ"),
            (b"foo", "MZXW6"),
            (b"foob", "MZXW6YQ"),
            (b"fooba", "MZXW6YTB"),
            (b"foobar", "MZXW6YTBOI"),
        ] {
            assert_eq!(base32(bytes), text);
            assert_eq!(base32_decode(text).as_deref(), Some(bytes));
        }
        assert_eq!(base32_decode("mzxw 6ytb oi======").as_deref(), Some(&b"foobar"[..]));
        assert_eq!(base32_decode("MZXW1"), None, "1 isn't base32");
        assert_eq!(base32_decode("MZXWŁ"), None);
        let secret = [0xa5u8; SECRET_BYTES];
        assert_eq!(base32(&secret).len(), 32, "160 bits, no padding");
    }

    #[test]
    fn uris_follow_the_key_uri_format() {
        let uri = otpauth_uri("pley", "bob@Living room", "JBSWY3DPEHPK3PXP");
        assert_eq!(
            uri,
            "otpauth://totp/pley:bob%40Living%20room?secret=JBSWY3DPEHPK3PXP&issuer=pley&algorithm=SHA1&digits=6&period=30"
        );
        assert_eq!(percent("a:b/c?d&e=f é"), "a%3Ab%2Fc%3Fd%26e%3Df%20%C3%A9");
    }

    #[test]
    fn qr_codes_are_paths_of_their_dark_modules() {
        let code = qr(&otpauth_uri("pley", "someone@a server with a long name", &base32(&[7; SECRET_BYTES]))).unwrap();
        assert!((21..=177).contains(&code.size) && (code.size - 17) % 4 == 0, "a QR code's size: {}", code.size);
        // The finder pattern in the corner: a row of 7 dark modules first.
        assert!(code.path.starts_with("M0 0h7v1h-7z"), "{}", &code.path[..40]);
        assert!(code.path.split('M').skip(1).all(|run| run.ends_with('z')));
    }

    #[tokio::test]
    async fn secrets_are_sealed_for_their_account_with_the_key_in_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("pley.db")).await.unwrap();
        let two_step = TwoStep::new(db.clone(), dir.path());
        let key = std::fs::read_to_string(dir.path().join(KEY_FILE)).unwrap();
        assert_eq!(key.trim().len(), 64, "32 bytes in hex");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(KEY_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "only pley's account reads it");
        }
        let sealed = two_step.seal(7, RFC_SECRET).unwrap();
        assert!(!sealed.contains("12345678901234567890"));
        assert_eq!(two_step.open(7, &sealed).as_deref(), Some(RFC_SECRET));
        assert_eq!(two_step.open(8, &sealed), None, "another account's");
        assert_ne!(two_step.seal(7, RFC_SECRET).unwrap(), sealed, "a new nonce each time");

        // The same key next time; another folder's key doesn't open it.
        assert_eq!(TwoStep::new(db.clone(), dir.path()).open(7, &sealed).as_deref(), Some(RFC_SECRET));
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(TwoStep::new(db.clone(), elsewhere.path()).open(7, &sealed), None);

        // A file that isn't a key stays as it is, and nothing gets sealed.
        std::fs::write(elsewhere.path().join(KEY_FILE), "not a key").unwrap();
        let broken = TwoStep::new(db, elsewhere.path());
        assert!(matches!(broken.seal(7, RFC_SECRET), Err(AppError::Unavailable(_))));
        assert!(matches!(broken.begin(7, "bob", "pley"), Err(AppError::Unavailable(_))));
        assert_eq!(std::fs::read_to_string(elsewhere.path().join(KEY_FILE)).unwrap(), "not a key");
    }
}
