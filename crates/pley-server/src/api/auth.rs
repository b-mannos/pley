use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::auth::{
    AuthUser, clear_cookie, hash_password, session_cookie, token_from_parts, validate_password, validate_username,
    verify_password,
};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::util::now;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserDto {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub created_at: i64,
}

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

async fn login_response(state: &AppState, user: UserDto, headers: &HeaderMap) -> ApiResult<Response> {
    let agent = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    let token = state.auth.create_session(user.id, agent).await?;
    let cookie = session_cookie(&token);
    Ok(([(header::SET_COOKIE, cookie)], Json(LoginResponse { user, token })).into_response())
}

pub async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Credentials>,
) -> ApiResult<Response> {
    validate_username(body.username.trim())?;
    validate_password(&body.password)?;
    let hash = hash_password(body.password).await?;
    // Insert only when there are no users yet (atomic with the write pool).
    let created: Option<UserDto> = sqlx::query_as(
        "INSERT INTO users (username, password_hash, is_admin, created_at)
         SELECT ?, ?, 1, ? WHERE NOT EXISTS (SELECT 1 FROM users)
         RETURNING id, username, is_admin, created_at",
    )
    .bind(body.username.trim())
    .bind(hash)
    .bind(now())
    .fetch_optional(&state.db.write)
    .await?;
    let user = created.ok_or_else(|| AppError::conflict("pley is already set up"))?;
    tracing::info!("created administrator account '{}'", user.username);
    login_response(&state, user, &headers).await
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Credentials>,
) -> ApiResult<Response> {
    let row: Option<(i64, String, bool, i64, String)> =
        sqlx::query_as("SELECT id, username, is_admin, created_at, password_hash FROM users WHERE username = ?")
            .bind(body.username.trim())
            .fetch_optional(&state.db.read)
            .await?;
    let Some((id, username, is_admin, created_at, hash)) = row else {
        // Spend comparable time so usernames can't be probed by timing.
        let _ = verify_password(body.password, "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$Zm9v".into()).await;
        return Err(AppError::Unauthorized);
    };
    if !verify_password(body.password, hash).await {
        return Err(AppError::Unauthorized);
    }
    login_response(&state, UserDto { id, username, is_admin, created_at }, &headers).await
}

pub async fn logout(State(state): State<AppState>, request: axum::extract::Request) -> Response {
    let (parts, _) = request.into_parts();
    if let Some(token) = token_from_parts(&parts) {
        state.auth.revoke(&token).await;
    }
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, clear_cookie())]).into_response()
}

pub async fn me(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<UserDto>> {
    let dto: UserDto = sqlx::query_as("SELECT id, username, is_admin, created_at FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_optional(&state.db.read)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(dto))
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
