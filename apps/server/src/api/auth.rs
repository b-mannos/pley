use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::accounts::{self, UserDto};
use crate::auth::{
    AuthUser, ClientIp, CurrentToken, clear_cookie, hash_password, session_cookie, token_from_parts, validate_password,
    validate_username, verify_password,
};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::util::now;

#[derive(Serialize)]
pub struct LoginResponse {
    user: UserDto,
    token: String,
}

#[derive(Deserialize)]
pub struct Credentials {
    username: String,
    password: String,
}

pub async fn server_status(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.db.read).await?;
    Ok(Json(serde_json::json!({
        "name": state.settings.get().server_name,
        "version": env!("CARGO_PKG_VERSION"),
        "setup_required": users == 0,
    })))
}

pub(crate) async fn login_response(
    state: &AppState,
    user_id: i64,
    headers: &HeaderMap,
    ip: Option<&str>,
) -> ApiResult<Response> {
    let agent = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    let token = state.auth.create_session(user_id, agent, ip).await?;
    let user = accounts::user(&state.db, user_id).await?.ok_or(AppError::Unauthorized)?;
    let cookie = session_cookie(&token);
    Ok(([(header::SET_COOKIE, cookie)], Json(LoginResponse { user, token })).into_response())
}

pub async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    Json(body): Json<Credentials>,
) -> ApiResult<Response> {
    validate_username(body.username.trim())?;
    validate_password(&body.password)?;
    let hash = hash_password(body.password).await?;
    // Insert only when there are no users yet (atomic with the write pool).
    let created: Option<i64> = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, is_admin, created_at)
         SELECT ?, ?, 1, ? WHERE NOT EXISTS (SELECT 1 FROM users)
         RETURNING id",
    )
    .bind(body.username.trim())
    .bind(hash)
    .bind(now())
    .fetch_optional(&state.db.write)
    .await?;
    let id = created.ok_or_else(|| AppError::conflict("pley is already set up"))?;
    tracing::info!("created administrator account '{}'", body.username.trim());
    login_response(&state, id, &headers, ip.as_deref()).await
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    Json(body): Json<Credentials>,
) -> ApiResult<Response> {
    let username = body.username.trim().to_owned();
    let throttle_key = format!("login:{}", username.to_lowercase());
    state.auth.throttle.check(&throttle_key)?;
    let row: Option<(i64, String, bool)> =
        sqlx::query_as("SELECT id, password_hash, disabled FROM users WHERE username = ?")
            .bind(&username)
            .fetch_optional(&state.db.read)
            .await?;
    let Some((id, hash, disabled)) = row else {
        // Spend comparable time so usernames can't be probed by timing.
        let _ = verify_password(body.password, "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$Zm9v".into()).await;
        state.auth.throttle.failed(&throttle_key);
        return Err(AppError::Unauthorized);
    };
    if !verify_password(body.password, hash).await {
        state.auth.throttle.failed(&throttle_key);
        return Err(AppError::Unauthorized);
    }
    state.auth.throttle.succeeded(&throttle_key);
    if disabled {
        return Err(AppError::forbidden("this account is disabled"));
    }
    login_response(&state, id, &headers, ip.as_deref()).await
}

pub async fn logout(State(state): State<AppState>, request: axum::extract::Request) -> Response {
    let (parts, _) = request.into_parts();
    if let Some(token) = token_from_parts(&parts) {
        state.auth.revoke(&token).await;
    }
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, clear_cookie())]).into_response()
}

pub async fn me(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<UserDto>> {
    Ok(Json(accounts::user(&state.db, user.id).await?.ok_or(AppError::Unauthorized)?))
}

#[derive(Deserialize)]
pub struct PasswordChange {
    current_password: String,
    new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<PasswordChange>,
) -> ApiResult<StatusCode> {
    validate_password(&body.new_password)?;
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_one(&state.db.read)
        .await?;
    if hash == accounts::NO_PASSWORD {
        return Err(AppError::bad_request("managed profiles have no password"));
    }
    if !verify_password(body.current_password, hash).await {
        return Err(AppError::bad_request("the current password is wrong"));
    }
    let new_hash = hash_password(body.new_password).await?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(new_hash)
        .bind(user.id)
        .execute(&state.db.write)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ home profiles

/// Profiles a home member can switch to ("Who's watching?").
pub async fn profiles(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<UserDto>>> {
    let all = accounts::users(&state.db).await?;
    let me_home = all.iter().any(|u| u.id == user.id && u.home);
    if !me_home {
        return Ok(Json(Vec::new()));
    }
    Ok(Json(all.into_iter().filter(|u| u.home && !u.disabled).collect()))
}

#[derive(Deserialize)]
pub struct Switch {
    user_id: i64,
    #[serde(default)]
    pin: Option<String>,
    /// Needed to switch into an administrator profile without a PIN.
    #[serde(default)]
    password: Option<String>,
}

/// Switch this device to another home profile. The target's PIN is required
/// when it has one; an administrator profile without a PIN needs its password
/// unless the current profile is an administrator too.
pub async fn switch(
    State(state): State<AppState>,
    user: AuthUser,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    CurrentToken(token): CurrentToken,
    Json(body): Json<Switch>,
) -> ApiResult<Response> {
    let row: Option<(bool, bool, bool, Option<String>, String)> = sqlx::query_as(
        "SELECT t.home AND me.home, t.disabled, t.is_admin, t.pin_hash, t.password_hash
         FROM users t, users me WHERE t.id = ? AND me.id = ?",
    )
    .bind(body.user_id)
    .bind(user.id)
    .fetch_optional(&state.db.read)
    .await?;
    let Some((both_home, disabled, target_admin, pin_hash, password_hash)) = row else {
        return Err(AppError::not_found("profile not found"));
    };
    if !both_home || disabled {
        return Err(AppError::forbidden("you can't switch to this profile"));
    }
    if body.user_id != user.id {
        let key = format!("profile:{}", body.user_id);
        state.auth.throttle.check(&key)?;
        let ok = match (&pin_hash, target_admin && !user.is_admin) {
            (Some(pin_hash), _) => match &body.pin {
                Some(pin) => verify_password(pin.clone(), pin_hash.clone()).await,
                None => return Err(AppError::forbidden("this profile is protected by a PIN")),
            },
            (None, true) => match &body.password {
                Some(password) => verify_password(password.clone(), password_hash).await,
                None => return Err(AppError::forbidden("switching to an administrator needs their password")),
            },
            (None, false) => true,
        };
        if !ok {
            state.auth.throttle.failed(&key);
            return Err(AppError::forbidden(if pin_hash.is_some() { "wrong PIN" } else { "wrong password" }));
        }
        state.auth.throttle.succeeded(&key);
    }
    if let Some(token) = token {
        state.auth.revoke(&token).await;
    }
    login_response(&state, body.user_id, &headers, ip.as_deref()).await
}
