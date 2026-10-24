//! User management (admins), profile self-service (avatars, PIN, devices) and
//! invitations.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Deserializer, Serialize};

use super::auth::{confirm_password, login_response};
use crate::accounts::{self, NO_PASSWORD, UserDto};
use crate::auth::{
    AdminUser, AuthUser, ClientIp, CurrentToken, hash_password, hash_token, validate_password, validate_username,
};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::streams::{self, SubtitleMode};
use crate::util::now;

/// `Some(None)` for an explicit `null`, `None` when the field is absent.
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

fn check_age(max_age: Option<i64>) -> ApiResult<()> {
    match max_age {
        Some(age) if !(0..=21).contains(&age) => Err(AppError::bad_request("the age limit must be between 0 and 21")),
        _ => Ok(()),
    }
}

async fn admins_left_after(state: &AppState, without: i64) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = 1 AND disabled = 0 AND id != ?")
        .bind(without)
        .fetch_one(&state.db.read)
        .await?)
}

// ------------------------------------------------------------------ admin: users

pub async fn list_users(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Vec<UserDto>>> {
    Ok(Json(accounts::users(&state.db).await?))
}

#[derive(Deserialize)]
pub struct NewUser {
    username: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    is_admin: bool,
    /// A profile without a password (children, guests), reached by switching.
    #[serde(default)]
    managed: bool,
    #[serde(default = "yes")]
    home: bool,
    #[serde(default = "yes")]
    all_libraries: bool,
    #[serde(default)]
    library_ids: Vec<i64>,
    #[serde(default)]
    max_age: Option<i64>,
    #[serde(default)]
    pin: Option<String>,
}

fn yes() -> bool {
    true
}

pub async fn create_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<NewUser>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    let username = body.username.trim().to_owned();
    validate_username(&username)?;
    check_age(body.max_age)?;
    if body.managed && body.is_admin {
        return Err(AppError::bad_request("a managed profile can't be an administrator"));
    }
    let password_hash = match (&body.password, body.managed) {
        (_, true) => NO_PASSWORD.to_owned(),
        (Some(password), false) => {
            validate_password(password, &username)?;
            hash_password(password.clone()).await?
        }
        (None, false) => return Err(AppError::bad_request("a password is required (or create a managed profile)")),
    };
    let pin_hash = match body.pin.as_deref().filter(|p| !p.is_empty()) {
        Some(pin) => {
            accounts::validate_pin(pin)?;
            Some(hash_password(pin.to_owned()).await?)
        }
        None => None,
    };
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&state.db.read)
        .await?;
    if exists.is_some() {
        return Err(AppError::conflict(format!("user '{username}' already exists")));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, display_name, is_admin, managed, home, all_libraries, max_age,
                            pin_hash, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&username)
    .bind(password_hash)
    .bind(body.display_name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()))
    .bind(body.is_admin)
    .bind(body.managed)
    .bind(body.home)
    .bind(body.all_libraries)
    .bind(body.max_age)
    .bind(pin_hash)
    .bind(now())
    .fetch_one(&state.db.write)
    .await?;
    if !body.all_libraries {
        accounts::set_libraries(&state.db, id, &body.library_ids).await?;
    }
    let user = accounts::user(&state.db, id).await?.ok_or_else(|| AppError::not_found("user not found"))?;
    Ok((StatusCode::CREATED, Json(user)))
}

#[derive(Deserialize)]
pub struct UserPatch {
    #[serde(default)]
    password: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    display_name: Option<Option<String>>,
    #[serde(default)]
    is_admin: Option<bool>,
    #[serde(default)]
    home: Option<bool>,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    all_libraries: Option<bool>,
    #[serde(default)]
    library_ids: Option<Vec<i64>>,
    #[serde(default, deserialize_with = "nullable")]
    max_age: Option<Option<i64>>,
    /// `null` removes the PIN.
    #[serde(default, deserialize_with = "nullable")]
    pin: Option<Option<String>>,
}

pub async fn update_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UserPatch>,
) -> ApiResult<Json<UserDto>> {
    let current = accounts::user(&state.db, id).await?.ok_or_else(|| AppError::not_found("user not found"))?;
    // Check everything first: a refused update changes nothing.
    if let Some(password) = &body.password {
        validate_password(password, &current.username)?;
    }
    if let Some(is_admin) = body.is_admin {
        if !is_admin && current.is_admin && admins_left_after(&state, id).await? == 0 {
            return Err(AppError::bad_request("pley needs at least one administrator"));
        }
        if is_admin && current.managed {
            return Err(AppError::bad_request("a managed profile can't be an administrator"));
        }
    }
    if body.disabled == Some(true) {
        if id == admin.id {
            return Err(AppError::bad_request("you can't disable your own account"));
        }
        if current.is_admin && admins_left_after(&state, id).await? == 0 {
            return Err(AppError::bad_request("pley needs at least one administrator"));
        }
    }
    if let Some(max_age) = body.max_age {
        check_age(max_age)?;
    }
    if let Some(Some(pin)) = body.pin.as_ref().map(|p| p.as_deref().filter(|p| !p.is_empty())) {
        accounts::validate_pin(pin)?;
    }

    if let Some(password) = body.password {
        let hash = hash_password(password).await?;
        sqlx::query("UPDATE users SET password_hash = ?, managed = 0 WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&state.db.write)
            .await?;
        // Sign the user out everywhere else.
        if id != admin.id {
            sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&state.db.write).await?;
        }
    }
    if let Some(name) = body.display_name {
        let name = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(is_admin) = body.is_admin {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(is_admin)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(home) = body.home {
        sqlx::query("UPDATE users SET home = ? WHERE id = ?").bind(home).bind(id).execute(&state.db.write).await?;
    }
    if let Some(disabled) = body.disabled {
        sqlx::query("UPDATE users SET disabled = ? WHERE id = ?")
            .bind(disabled)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(all) = body.all_libraries {
        sqlx::query("UPDATE users SET all_libraries = ? WHERE id = ?")
            .bind(all)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(ids) = &body.library_ids {
        accounts::set_libraries(&state.db, id, ids).await?;
    }
    if let Some(max_age) = body.max_age {
        sqlx::query("UPDATE users SET max_age = ? WHERE id = ?")
            .bind(max_age)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(pin) = body.pin {
        let hash = match pin.filter(|p| !p.is_empty()) {
            Some(pin) => Some(hash_password(pin).await?),
            None => None,
        };
        sqlx::query("UPDATE users SET pin_hash = ?, pin_failures = 0 WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&state.db.write)
            .await?;
    }
    state.auth.forget_user(id);
    Ok(Json(accounts::user(&state.db, id).await?.ok_or_else(|| AppError::not_found("user not found"))?))
}

pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    if id == admin.id {
        return Err(AppError::bad_request("you cannot delete your own account"));
    }
    let avatar: Option<Option<String>> =
        sqlx::query_scalar("SELECT avatar FROM users WHERE id = ?").bind(id).fetch_optional(&state.db.read).await?;
    let result = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&state.db.write).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("user not found"));
    }
    if let Some(Some(file)) = avatar {
        let _ = tokio::fs::remove_file(accounts::avatars_dir(&state.config.data_dir).join(file)).await;
    }
    state.auth.forget_user(id);
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ devices

#[derive(Serialize, sqlx::FromRow)]
struct SessionRow {
    token_hash: String,
    created_at: i64,
    last_seen_at: i64,
    user_agent: Option<String>,
    last_ip: Option<String>,
}

#[derive(Serialize)]
pub struct DeviceDto {
    /// Stable handle of the session (not the token).
    id: String,
    name: String,
    user_agent: Option<String>,
    ip: Option<String>,
    created_at: i64,
    last_seen_at: i64,
    current: bool,
}

async fn devices_of(state: &AppState, user_id: i64, current: Option<&str>) -> ApiResult<Vec<DeviceDto>> {
    let rows: Vec<SessionRow> = sqlx::query_as(
        "SELECT token_hash, created_at, last_seen_at, user_agent, last_ip FROM sessions
         WHERE user_id = ? AND expires_at > ? ORDER BY last_seen_at DESC",
    )
    .bind(user_id)
    .bind(now())
    .fetch_all(&state.db.read)
    .await?;
    let current = current.map(hash_token);
    Ok(rows
        .into_iter()
        .map(|r| DeviceDto {
            id: r.token_hash[..16].to_owned(),
            name: accounts::device_name(r.user_agent.as_deref()),
            current: current.as_deref() == Some(r.token_hash.as_str()),
            user_agent: r.user_agent,
            ip: r.last_ip,
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
        })
        .collect())
}

pub async fn my_devices(
    State(state): State<AppState>,
    user: AuthUser,
    CurrentToken(token): CurrentToken,
) -> ApiResult<Json<Vec<DeviceDto>>> {
    Ok(Json(devices_of(&state, user.id, token.as_deref()).await?))
}

pub async fn sign_out_device(
    State(state): State<AppState>,
    user: AuthUser,
    Path(device): Path<String>,
) -> ApiResult<StatusCode> {
    if device.len() < 16 || !device.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::not_found("device not found"));
    }
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = ? AND substr(token_hash, 1, 16) = ?")
        .bind(user.id)
        .bind(&device[..16])
        .execute(&state.db.write)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("device not found"));
    }
    state.auth.forget_user(user.id);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn user_devices(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<DeviceDto>>> {
    Ok(Json(devices_of(&state, id, None).await?))
}

/// Sign a user out on every device.
pub async fn sign_out_everywhere(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    if id == admin.id {
        return Err(AppError::bad_request("sign yourself out from your account page instead"));
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&state.db.write).await?;
    state.auth.forget_user(id);
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ own profile

#[derive(Deserialize)]
pub struct ProfilePatch {
    #[serde(default, deserialize_with = "nullable")]
    display_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    avatar_color: Option<Option<String>>,
    /// ISO 639 code; `null`: the file's default track.
    #[serde(default, deserialize_with = "nullable")]
    audio_language: Option<Option<String>>,
    /// ISO 639 code; `null`: the audio language.
    #[serde(default, deserialize_with = "nullable")]
    subtitle_language: Option<Option<String>>,
    #[serde(default)]
    subtitle_mode: Option<SubtitleMode>,
}

pub async fn update_me(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<ProfilePatch>,
) -> ApiResult<Json<UserDto>> {
    if let Some(name) = body.display_name {
        let name = name.map(|n| n.trim().chars().take(64).collect::<String>()).filter(|n| !n.is_empty());
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
            .bind(name)
            .bind(user.id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(color) = body.avatar_color {
        if let Some(c) = &color {
            accounts::validate_color(c)?;
        }
        sqlx::query("UPDATE users SET avatar_color = ? WHERE id = ?")
            .bind(color)
            .bind(user.id)
            .execute(&state.db.write)
            .await?;
    }
    for (column, value) in [("audio_language", body.audio_language), ("subtitle_language", body.subtitle_language)] {
        let Some(value) = value else { continue };
        if value.as_deref().is_some_and(|code| !streams::valid_language(code)) {
            return Err(AppError::bad_request(format!("{column} must be an ISO 639 code such as \"eng\"")));
        }
        sqlx::query(&format!("UPDATE users SET {column} = ? WHERE id = ?"))
            .bind(value)
            .bind(user.id)
            .execute(&state.db.write)
            .await?;
    }
    if let Some(mode) = body.subtitle_mode {
        sqlx::query("UPDATE users SET subtitle_mode = ? WHERE id = ?")
            .bind(mode.as_str())
            .bind(user.id)
            .execute(&state.db.write)
            .await?;
    }
    Ok(Json(accounts::user(&state.db, user.id).await?.ok_or(AppError::Unauthorized)?))
}

#[derive(Deserialize)]
pub struct PinChange {
    /// `null` removes the PIN.
    pin: Option<String>,
    /// Your password (not needed for managed profiles, which have none).
    #[serde(default)]
    password: Option<String>,
}

pub async fn set_my_pin(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<PinChange>,
) -> ApiResult<StatusCode> {
    let managed: bool =
        sqlx::query_scalar("SELECT managed FROM users WHERE id = ?").bind(user.id).fetch_one(&state.db.read).await?;
    if managed {
        return Err(AppError::forbidden("ask an administrator to change this profile's PIN"));
    }
    confirm_password(&state, &user, body.password).await?;
    let hash = match body.pin.filter(|p| !p.is_empty()) {
        Some(pin) => {
            accounts::validate_pin(&pin)?;
            Some(hash_password(pin).await?)
        }
        None => None,
    };
    sqlx::query("UPDATE users SET pin_hash = ?, pin_failures = 0 WHERE id = ?")
        .bind(hash)
        .bind(user.id)
        .execute(&state.db.write)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

const AVATAR_MAX_BYTES: usize = 10 * 1024 * 1024;

/// Upload a profile picture (any common image format, raw request body).
pub async fn upload_avatar(State(state): State<AppState>, user: AuthUser, body: Bytes) -> ApiResult<Json<UserDto>> {
    if body.is_empty() || body.len() > AVATAR_MAX_BYTES {
        return Err(AppError::bad_request("pictures must be smaller than 10 MB"));
    }
    let _decoding = crate::images::UPLOAD_DECODES.acquire().await.map_err(|e| AppError::Internal(e.into()))?;
    let jpeg = tokio::task::spawn_blocking(move || accounts::process_avatar(&body))
        .await
        .map_err(|e| AppError::Internal(e.into()))??;
    let dir = accounts::avatars_dir(&state.config.data_dir);
    tokio::fs::create_dir_all(&dir).await.map_err(|e| AppError::Internal(e.into()))?;
    let file = format!("{}-{}.jpg", user.id, now());
    tokio::fs::write(dir.join(&file), jpeg).await.map_err(|e| AppError::Internal(e.into()))?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar FROM users WHERE id = ?").bind(user.id).fetch_one(&state.db.read).await?;
    sqlx::query("UPDATE users SET avatar = ? WHERE id = ?").bind(&file).bind(user.id).execute(&state.db.write).await?;
    if let Some(old) = old.filter(|o| *o != file) {
        let _ = tokio::fs::remove_file(dir.join(old)).await;
    }
    Ok(Json(accounts::user(&state.db, user.id).await?.ok_or(AppError::Unauthorized)?))
}

pub async fn delete_avatar(State(state): State<AppState>, user: AuthUser) -> ApiResult<StatusCode> {
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar FROM users WHERE id = ?").bind(user.id).fetch_one(&state.db.read).await?;
    sqlx::query("UPDATE users SET avatar = NULL WHERE id = ?").bind(user.id).execute(&state.db.write).await?;
    if let Some(old) = old {
        let _ = tokio::fs::remove_file(accounts::avatars_dir(&state.config.data_dir).join(old)).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn avatar(State(state): State<AppState>, user: AuthUser, Path(id): Path<i64>) -> ApiResult<Response> {
    // Apps see their own profile's picture, and those of its household.
    if user.app.is_some() && id != user.id {
        let household: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users me, users them
                            WHERE me.id = ? AND them.id = ? AND me.home AND them.home AND NOT them.disabled)",
        )
        .bind(user.id)
        .bind(id)
        .fetch_one(&state.db.read)
        .await?;
        if !household {
            return Err(AppError::not_found("no picture"));
        }
    }
    let file: Option<Option<String>> =
        sqlx::query_scalar("SELECT avatar FROM users WHERE id = ?").bind(id).fetch_optional(&state.db.read).await?;
    let file = file.flatten().ok_or_else(|| AppError::not_found("no picture"))?;
    let bytes = tokio::fs::read(accounts::avatars_dir(&state.config.data_dir).join(file))
        .await
        .map_err(|_| AppError::not_found("no picture"))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=31536000, immutable"));
    Ok((headers, bytes).into_response())
}

// ------------------------------------------------------------------ invites

#[derive(sqlx::FromRow)]
struct InviteRow {
    code: String,
    created_at: i64,
    expires_at: Option<i64>,
    uses_left: i64,
    all_libraries: bool,
    library_ids: String,
    max_age: Option<i64>,
    note: Option<String>,
}

#[derive(Serialize)]
pub struct InviteDto {
    code: String,
    created_at: i64,
    expires_at: Option<i64>,
    uses_left: i64,
    all_libraries: bool,
    library_ids: Vec<i64>,
    max_age: Option<i64>,
    note: Option<String>,
}

impl From<InviteRow> for InviteDto {
    fn from(r: InviteRow) -> Self {
        InviteDto {
            library_ids: serde_json::from_str(&r.library_ids).unwrap_or_default(),
            code: r.code,
            created_at: r.created_at,
            expires_at: r.expires_at,
            uses_left: r.uses_left,
            all_libraries: r.all_libraries,
            max_age: r.max_age,
            note: r.note,
        }
    }
}

const INVITE_COLUMNS: &str = "code, created_at, expires_at, uses_left, all_libraries, library_ids, max_age, note";

pub async fn list_invites(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Vec<InviteDto>>> {
    let rows: Vec<InviteRow> =
        sqlx::query_as(&format!("SELECT {INVITE_COLUMNS} FROM invites ORDER BY created_at DESC"))
            .fetch_all(&state.db.read)
            .await?;
    Ok(Json(rows.into_iter().map(InviteDto::from).collect()))
}

#[derive(Deserialize)]
pub struct NewInvite {
    /// Days until the link stops working (default 7, 0 = never).
    #[serde(default)]
    expires_in_days: Option<i64>,
    #[serde(default)]
    uses: Option<i64>,
    #[serde(default = "yes")]
    all_libraries: bool,
    #[serde(default)]
    library_ids: Vec<i64>,
    #[serde(default)]
    max_age: Option<i64>,
    #[serde(default)]
    note: Option<String>,
}

pub async fn create_invite(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Json(body): Json<NewInvite>,
) -> ApiResult<(StatusCode, Json<InviteDto>)> {
    check_age(body.max_age)?;
    let days = body.expires_in_days.unwrap_or(7);
    let uses = body.uses.unwrap_or(1).clamp(1, 1000);
    let code = accounts::invite_code();
    let created_at = now();
    let expires_at = (days > 0).then(|| created_at + days * 86_400);
    let note = body.note.map(|n| n.trim().chars().take(200).collect::<String>()).filter(|n| !n.is_empty());
    sqlx::query(
        "INSERT INTO invites (code, created_by, created_at, expires_at, uses_left, all_libraries, library_ids, max_age, note)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&code)
    .bind(admin.id)
    .bind(created_at)
    .bind(expires_at)
    .bind(uses)
    .bind(body.all_libraries)
    .bind(serde_json::to_string(&body.library_ids).unwrap_or_else(|_| "[]".into()))
    .bind(body.max_age)
    .bind(&note)
    .execute(&state.db.write)
    .await?;
    let invite = InviteDto {
        code,
        created_at,
        expires_at,
        uses_left: uses,
        all_libraries: body.all_libraries,
        library_ids: body.library_ids,
        max_age: body.max_age,
        note,
    };
    Ok((StatusCode::CREATED, Json(invite)))
}

pub async fn delete_invite(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(code): Path<String>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM invites WHERE code = ?").bind(code).execute(&state.db.write).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn valid_invite(state: &AppState, code: &str) -> ApiResult<InviteDto> {
    let invite: Option<InviteRow> = sqlx::query_as(&format!(
        "SELECT {INVITE_COLUMNS} FROM invites WHERE code = ? AND uses_left > 0 AND (expires_at IS NULL OR expires_at > ?)"
    ))
    .bind(code)
    .bind(now())
    .fetch_optional(&state.db.read)
    .await?;
    invite.map(InviteDto::from).ok_or_else(|| AppError::not_found("this invitation is invalid or has expired"))
}

/// Public: is this invitation still valid? (Shows the server name on the sign-up page.)
pub async fn check_invite(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let invite = valid_invite(&state, &code).await?;
    Ok(Json(serde_json::json!({
        "server_name": state.settings.get().server_name,
        "expires_at": invite.expires_at,
    })))
}

#[derive(Deserialize)]
pub struct AcceptInvite {
    username: String,
    password: String,
    #[serde(default)]
    display_name: Option<String>,
}

/// Public: create an account with an invitation and sign in.
pub async fn accept_invite(
    State(state): State<AppState>,
    Path(code): Path<String>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    Json(body): Json<AcceptInvite>,
) -> ApiResult<Response> {
    let username = body.username.trim().to_owned();
    validate_username(&username)?;
    validate_password(&body.password, &username)?;
    let invite = valid_invite(&state, &code).await?;
    let hash = hash_password(body.password).await?;
    let mut tx = state.db.write.begin().await?;
    // Claim a use first so concurrent sign-ups can't overdraw the invitation.
    let claimed = sqlx::query(
        "UPDATE invites SET uses_left = uses_left - 1
         WHERE code = ? AND uses_left > 0 AND (expires_at IS NULL OR expires_at > ?)",
    )
    .bind(&code)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(AppError::not_found("this invitation is invalid or has expired"));
    }
    let taken: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = ?").bind(&username).fetch_optional(&mut *tx).await?;
    if taken.is_some() {
        return Err(AppError::conflict(format!("the username '{username}' is taken")));
    }
    // Invited people are friends of the server, not members of the home.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, display_name, home, all_libraries, max_age, created_at)
         VALUES (?, ?, ?, 0, ?, ?, ?) RETURNING id",
    )
    .bind(&username)
    .bind(hash)
    .bind(body.display_name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()))
    .bind(invite.all_libraries)
    .bind(invite.max_age)
    .bind(now())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    if !invite.all_libraries {
        accounts::set_libraries(&state.db, id, &invite.library_ids).await?;
    }
    tracing::info!("'{username}' joined with an invitation");
    login_response(&state, id, &headers, ip.as_deref()).await
}
