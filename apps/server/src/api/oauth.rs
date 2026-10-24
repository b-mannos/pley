//! The authorization server's endpoints: apps sign in on pley's own page
//! (OAuth 2.1, draft-ietf-oauth-v2-1-16, "OAuth 2.1" below), with PKCE and
//! client metadata documents (see [`crate::oauth`]).
//!
//! * `/.well-known/oauth-authorization-server` says where everything is
//!   (RFC 8414), for the address it was asked at.
//! * The authorization endpoint is a page of the web app,
//!   `/oauth/authorize`. It hands the app's request to
//!   `POST /api/oauth/requests`, which checks it, shows the person what the
//!   app asks for, and takes their answer; the browser then goes back to the
//!   app through `GET /api/oauth/requests/{id}/continue` (a 303). These take
//!   the web app's cookie and nothing else, from pley's own page only.
//! * Apps exchange codes and refresh tokens at `POST /api/oauth/token` and
//!   sign out at `POST /api/oauth/revoke`: forms, no cookies, no secrets.
//! * People see and sign out the apps using their account at
//!   `/api/me/apps`; administrators, anyone's at `/api/users/{id}/apps`.

use std::collections::HashSet;

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use crate::auth::grants::{self, GrantRow, Issued, NewGrant, RefreshError, RefreshInput};
use crate::auth::scopes::{Scope, Scopes};
use crate::auth::{AdminUser, AuthUser, ClientIp, cookie_token, forwarded_https, hash_token};
use crate::error::{ApiResult, AppError};
use crate::oauth::clients::{self, ClientError, RedirectKind};
use crate::oauth::codes::{self, CodeGrant, Redeem};
use crate::oauth::pending::{self, DecideError, PendingRequest, ReturnedError};
use crate::state::AppState;
use crate::util::now;

/// The longest query the consent page may hand over.
const QUERY_MAX: usize = 8 * 1024;
/// The longest return address an app may ask for.
const REDIRECT_URI_MAX: usize = 512;
/// The longest `state` an app may send (bytes).
const STATE_MAX: usize = 1024;

const NOT_FROM_PLEY: &str = "this page must be opened from pley itself";
const OVER: &str = "this request is over: start again from the app";
const BUSY: &str = "too many sign-ins in progress: try again in a minute";

// ------------------------------------------------------------------ issuer

/// The address this server was reached at (RFC 8414 §2's issuer): "https"
/// when the proxy in front says so, the Host header, and nothing else, in
/// its canonical form (lower-case host, no default port, no trailing slash).
/// Each request has its own, so a server works at its home and public
/// addresses alike; a forged Host only changes the forger's own answers.
pub(crate) fn issuer(headers: &HeaderMap) -> ApiResult<String> {
    let unknown = || AppError::bad_request("pley can't tell which address it was reached at: open it by its address");
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).map(str::trim).unwrap_or_default();
    if host.is_empty() || host.contains(['/', '\\', '?', '#', '@']) {
        return Err(unknown());
    }
    let scheme = if forwarded_https(headers) { "https" } else { "http" };
    let url = Url::parse(&format!("{scheme}://{host}")).map_err(|_| unknown())?;
    let plain = url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.host().is_some();
    if !plain {
        return Err(unknown());
    }
    Ok(url.origin().ascii_serialization())
}

/// The host of an issuer.
fn issuer_host(issuer: &str) -> String {
    Url::parse(issuer).ok().and_then(|url| url.host_str().map(str::to_owned)).unwrap_or_default()
}

/// Whether an app's `resource` (RFC 8707) names this server: absent, or the
/// issuer with or without a trailing slash.
fn is_this_server(resource: Option<&str>, issuer: &str) -> bool {
    resource.is_none_or(|resource| resource.strip_suffix('/').unwrap_or(resource) == issuer)
}

// ------------------------------------------------------------------ metadata

/// `GET /.well-known/oauth-authorization-server` (RFC 8414): where an app
/// signs in, for the address it asked at. Never cached: it depends on Host.
pub async fn metadata(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let issuer = issuer(&headers)?;
    let body = json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/api/oauth/token"),
        "revocation_endpoint": format!("{issuer}/api/oauth/revoke"),
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"],
        "revocation_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": Scope::ALL.map(Scope::name),
        "authorization_response_iss_parameter_supported": true,
        "client_id_metadata_document_supported": state.settings.get().third_party_apps,
        "service_documentation": "https://github.com/hhaidar/pley/blob/master/docs/API.md",
    });
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response())
}

/// Any other `/.well-known/` document: pley has none (and not the web app's
/// page either, which would look like an answer).
pub async fn no_document() -> AppError {
    AppError::not_found("no such document")
}

// ------------------------------------------------------------------ the web app's own page

/// Someone signed in to pley's web app, on pley's own page: the session
/// cookie only (never `Authorization`, `X-Pley-Token` or `?token=`, so
/// neither an app nor a script can allow an app), and for changes, a request
/// whose Origin is this server (and Sec-Fetch-Site same-origin when the
/// browser sends it). No CORS anywhere (RFC 9700 §2.6).
pub struct WebUser {
    pub user: AuthUser,
    /// SHA-256 of the session token: requests belong to this session.
    pub session_hash: String,
}

impl FromRequestParts<AppState> for WebUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = cookie_token(parts).ok_or(AppError::Unauthorized)?;
        // Never an app's token: validate takes sessions only.
        let user = state.auth.validate(&token).await.ok_or(AppError::Unauthorized)?;
        if !from_this_site(parts) {
            return Err(AppError::forbidden(NOT_FROM_PLEY));
        }
        Ok(WebUser { user, session_hash: hash_token(&token) })
    }
}

/// Whether a request comes from pley's own page (see [`WebUser`]).
/// Sec-Fetch-Site: <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Sec-Fetch-Site>.
fn from_this_site(parts: &Parts) -> bool {
    let site = parts.headers.get("sec-fetch-site").map(|v| v.to_str().unwrap_or_default());
    if parts.method == Method::GET || parts.method == Method::HEAD {
        // Going back to the app: a navigation from the page, or typed.
        return site.is_none_or(|site| matches!(site, "same-origin" | "none"));
    }
    if site.is_some_and(|site| site != "same-origin") {
        return false;
    }
    let Some(origin) = parts.headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return false };
    let Ok(origin) = Url::parse(origin) else { return false };
    if !matches!(origin.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = parts.headers.get(header::HOST).and_then(|v| v.to_str().ok()) else { return false };
    if host.contains(['/', '\\', '?', '#', '@']) {
        return false;
    }
    let Ok(host) = Url::parse(&format!("{}://{}", origin.scheme(), host.trim())) else { return false };
    origin.host().is_some()
        && origin.host() == host.host()
        && origin.port_or_known_default() == host.port_or_known_default()
}

// ------------------------------------------------------------------ requests (the consent page)

#[derive(Deserialize)]
pub struct StartRequest {
    /// The consent page's query, without "?".
    query: String,
}

#[derive(Serialize)]
struct ClientDto<'a> {
    id: &'a str,
    host: &'a str,
    name: Option<&'a str>,
    first_party: bool,
}

/// The parameters of a query or a form, each at most once (OAuth 2.1 §3.1):
/// the name of one given twice is the error.
fn parameters(text: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut seen = HashSet::new();
    let mut params = Vec::new();
    for (name, value) in url::form_urlencoded::parse(text) {
        if !seen.insert(name.clone()) {
            return Err(name.into_owned());
        }
        params.push((name.into_owned(), value.into_owned()));
    }
    Ok(params)
}

/// The value of `name`, when it was given and isn't empty.
fn param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
}

/// Whether `name` was given at all (empty too).
fn given(params: &[(String, String)], name: &str) -> bool {
    params.iter().any(|(n, _)| n == name)
}

/// A parameter's name for a message, when it is a plain one.
fn shown_name(name: &str) -> Option<&str> {
    (name.len() <= 32 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')).then_some(name)
}

/// `POST /api/oauth/requests`: the consent page hands over the app's
/// request (its query). Checked in this order (endpoint 4 of the design):
/// a. every parameter at most once;
/// b. the app (built in, or read from its description when allowed);
/// c. the return address, registered (exact, but for a loopback port).
///
/// What fails there is shown on the page and never sent to the app: pley
/// can't know the address is the app's (OAuth 2.1 §4.1.2.1; RFC 9700
/// §4.11.2). Everything after that goes back to the app, but only when the
/// person clicks (OAuth 2.1 §7.13.2: no open redirector).
pub async fn create_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    web: WebUser,
    Json(body): Json<StartRequest>,
) -> ApiResult<Json<Value>> {
    let issuer = issuer(&headers)?;
    if body.query.len() > QUERY_MAX {
        return Err(AppError::bad_request("the app's request is too long: ask the app's publisher to fix it"));
    }
    let params = parameters(body.query.as_bytes()).map_err(|name| {
        AppError::bad_request(match shown_name(&name) {
            Some(name) => format!("the app's request gives {name} twice: ask the app's publisher to fix it"),
            None => "the app's request gives a parameter twice: ask the app's publisher to fix it".to_owned(),
        })
    })?;
    let client_id = param(&params, "client_id").ok_or_else(|| {
        AppError::bad_request("the app didn't say which app it is: ask the app's publisher to fix it")
    })?;
    let third_party_allowed = state.settings.get().third_party_apps;
    let client =
        state.oauth.documents.get(client_id, web.user.id, third_party_allowed, &issuer_host(&issuer)).await.map_err(
            |err| match err {
                ClientError::Busy => {
                    AppError::TooManyRequests("too many app descriptions read: try again in a minute".into())
                }
                err => AppError::bad_request(err.to_string()),
            },
        )?;
    let redirect_uri = param(&params, "redirect_uri").ok_or_else(|| {
        AppError::bad_request("the app didn't say where to send you back: ask the app's publisher to fix it")
    })?;
    let unlisted =
        || AppError::bad_request("this app didn't list that return address: ask the app's publisher to fix it");
    if redirect_uri.len() > REDIRECT_URI_MAX {
        return Err(unlisted());
    }
    let kind = clients::redirect_kind(&client, redirect_uri).map_err(|_| unlisted())?;

    let checked = check_request(&params, &client, &issuer);
    let (scopes, error) = match checked {
        Ok(scopes) => (scopes, None),
        Err(error) => (Scopes::default(), Some(error)),
    };
    let request = PendingRequest {
        user_id: web.user.id,
        session_hash: web.session_hash,
        issuer,
        app: (&client).into(),
        redirect_uri: redirect_uri.to_owned(),
        state: param(&params, "state").map(str::to_owned),
        code_challenge: if error.is_none() {
            param(&params, "code_challenge").unwrap_or_default().to_owned()
        } else {
            String::new()
        },
        scopes,
        error: error.clone(),
    };
    let id = state.oauth.pending.create(request).map_err(|_| AppError::Unavailable(BUSY.into()))?;
    let client_dto =
        ClientDto { id: &client.id, host: &client.host, name: client.name.as_deref(), first_party: client.first_party };
    if let Some(error) = error {
        return Ok(Json(json!({
            "status": "error",
            "error": error.error,
            "message": error.description,
            "back_url": format!("/api/oauth/requests/{id}/continue"),
            "client": client_dto,
        })));
    }
    let redirect = match kind {
        RedirectKind::Web { host } => json!({"kind": "web", "to": host}),
        RedirectKind::App { scheme } => json!({"kind": "app", "to": scheme}),
        RedirectKind::Loopback => {
            let host = Url::parse(redirect_uri).ok().and_then(|url| url.host_str().map(str::to_owned));
            json!({"kind": "loopback", "to": host.unwrap_or_default()})
        }
    };
    Ok(Json(json!({
        "status": "consent",
        "id": id,
        "expires_at": now() + pending::PENDING_LIFETIME.as_secs() as i64,
        "client": client_dto,
        "redirect": redirect,
        "scopes": scopes.names(),
    })))
}

/// The checks of a request whose mistakes go back to the app (d. of
/// [`create_request`]); the scopes it asks for. Descriptions are ASCII
/// (RFC 6749 §4.1.2.1): the app gets them as error_description.
fn check_request(params: &[(String, String)], client: &clients::Client, issuer: &str) -> Result<Scopes, ReturnedError> {
    let fail = |error: &'static str, description: &str| ReturnedError { error, description: description.to_owned() };
    if param(params, "response_type") != Some("code") {
        return Err(fail(
            "unsupported_response_type",
            "this app asked to sign in in a way pley doesn't support (response_type): ask the app's publisher to fix it",
        ));
    }
    if param(params, "response_mode").is_some_and(|mode| mode != "query") {
        return Err(fail(
            "invalid_request",
            "this app asked for its answer in a way pley doesn't support (response_mode): ask the app's publisher to \
             fix it",
        ));
    }
    if given(params, "request") || given(params, "request_uri") {
        return Err(fail("invalid_request", "request objects aren't supported: ask the app's publisher to fix it"));
    }
    let challenge_ok = param(params, "code_challenge_method") == Some("S256")
        && param(params, "code_challenge").is_some_and(codes::is_challenge);
    if !challenge_ok {
        return Err(fail(
            "invalid_request",
            "this app didn't protect its sign-in the way pley requires (PKCE with S256): ask the app's publisher to \
             fix it",
        ));
    }
    let asked = match param(params, "scope").map(Scopes::parse) {
        None => Ok(Scopes::default()),
        Some(parsed) => parsed,
    };
    let asked = match asked {
        Ok(asked) if asked.is_empty() => Scopes::default().with(Scope::Library),
        Ok(asked) => asked,
        Err(name) if name == "openid" => {
            return Err(fail(
                "invalid_scope",
                "pley doesn't sign people in to other sites (openid): ask the app's publisher to fix it",
            ));
        }
        Err(_) => {
            return Err(fail(
                "invalid_scope",
                "this app asked for access pley doesn't offer: ask the app's publisher to fix it",
            ));
        }
    };
    if client.scope_cap.as_deref().is_some_and(|cap| !asked.is_subset_of(Scopes::from_stored(cap))) {
        return Err(fail(
            "invalid_scope",
            "this app asked for more than its description allows: ask the app's publisher to fix it",
        ));
    }
    if !is_this_server(param(params, "resource"), issuer) {
        return Err(fail(
            "invalid_target",
            "this app asked for another server (resource): ask the app's publisher to fix it",
        ));
    }
    if param(params, "state").is_some_and(|state| state.len() > STATE_MAX) {
        return Err(fail("invalid_request", "this app's state is too long: ask the app's publisher to fix it"));
    }
    Ok(asked)
}

#[derive(Deserialize)]
pub struct Decision {
    allow: bool,
}

/// `POST /api/oauth/requests/{id}`: the person allows the app (a code is
/// made) or not. The same answer again gets the same reply; the other one
/// a conflict.
pub async fn decide(
    State(state): State<AppState>,
    web: WebUser,
    Path(id): Path<String>,
    Json(body): Json<Decision>,
) -> ApiResult<Json<Value>> {
    let decided = state.oauth.pending.decide(&id, web.user.id, &web.session_hash, body.allow, |request| {
        state.oauth.codes.new_code(CodeGrant {
            app: request.app.clone(),
            redirect_uri: request.redirect_uri.clone(),
            code_challenge: request.code_challenge.clone(),
            user_id: request.user_id,
            scopes: request.scopes,
        })
    });
    match decided {
        Ok(()) => Ok(Json(json!({"continue_url": format!("/api/oauth/requests/{id}/continue")}))),
        Err(DecideError::NotFound) => Err(AppError::not_found(OVER)),
        Err(DecideError::Conflict) => {
            Err(AppError::conflict("this request was answered already: start again from the app"))
        }
        Err(DecideError::Code(_)) => Err(AppError::Unavailable(BUSY.into())),
    }
}

/// `GET /api/oauth/requests/{id}/continue`: back to the app with the
/// answer, by a 303 (RFC 9700 §4.12) to its return address as it was sent:
/// code, state and iss when allowed; error (access_denied, or the request's
/// mistake), state and iss otherwise (RFC 9207: iss in every answer).
/// It repeats while the code works and wasn't exchanged, and for a minute
/// after a refusal or a mistake.
pub async fn continue_to_app(
    State(state): State<AppState>,
    web: WebUser,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let back = state
        .oauth
        .pending
        .location(&id, web.user.id, &web.session_hash)
        .filter(|back| back.code.as_deref().is_none_or(|code| state.oauth.codes.unredeemed(code)))
        .ok_or_else(|| AppError::not_found(OVER))?;
    let location = HeaderValue::from_str(&back.location).map_err(|e| AppError::Internal(e.into()))?;
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, location), (header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
    )
        .into_response())
}

// ------------------------------------------------------------------ token and revocation (apps)

/// An error of the token and revocation endpoints (RFC 6749 §5.2).
#[derive(Debug)]
pub struct OAuthError {
    status: StatusCode,
    error: &'static str,
    description: String,
    /// The app tried HTTP Basic authentication: say how it would work.
    basic: bool,
}

impl OAuthError {
    fn new(error: &'static str, description: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, error, description: description.into(), basic: false }
    }

    fn invalid_request(description: impl Into<String>) -> Self {
        Self::new("invalid_request", description)
    }

    fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new("invalid_grant", description)
    }

    fn invalid_client(description: &str, basic: bool) -> Self {
        Self { status: StatusCode::UNAUTHORIZED, basic, ..Self::new("invalid_client", description) }
    }

    fn server_error(err: impl std::fmt::Display) -> Self {
        tracing::error!("an app's token request failed: {err}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            ..Self::new("server_error", "pley couldn't answer: try again")
        }
    }
}

/// What every answer of the token endpoint says about caching (RFC 6749 §5.1).
const NO_STORE: [(header::HeaderName, HeaderValue); 2] = [
    (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
    (header::PRAGMA, HeaderValue::from_static("no-cache")),
];

impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        let body = Json(json!({"error": self.error, "error_description": self.description}));
        let mut response = (self.status, NO_STORE, body).into_response();
        if self.basic {
            response.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static(r#"Basic realm="pley""#));
        }
        response
    }
}

/// The parameters of a token or revocation request: a form
/// (application/x-www-form-urlencoded) of at most 16 KiB, each parameter at
/// most once, and no client authentication (every app is public).
struct Form(Vec<(String, String)>);

impl Form {
    fn read(headers: &HeaderMap, body: Result<Bytes, BytesRejection>) -> Result<Form, OAuthError> {
        let basic = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split_once(' '))
            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("basic"));
        if basic {
            return Err(OAuthError::invalid_client(
                "pley's apps have no secrets: leave out the Authorization header",
                true,
            ));
        }
        let form = headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/x-www-form-urlencoded"));
        if !form {
            return Err(OAuthError::invalid_request(
                "send the parameters as a form (application/x-www-form-urlencoded)",
            ));
        }
        let body = body.map_err(|_| OAuthError::invalid_request("the request is too large"))?;
        let params = parameters(&body).map_err(|name| {
            OAuthError::invalid_request(match shown_name(&name) {
                Some(name) => format!("{name} is given twice"),
                None => "a parameter is given twice".to_owned(),
            })
        })?;
        if given(&params, "client_secret") {
            return Err(OAuthError::invalid_client("pley's apps have no secrets: leave out client_secret", false));
        }
        Ok(Form(params))
    }

    fn get(&self, name: &str) -> Option<&str> {
        param(&self.0, name)
    }

    fn required(&self, name: &'static str) -> Result<&str, OAuthError> {
        self.get(name).ok_or_else(|| OAuthError::invalid_request(format!("{name} is missing")))
    }

    /// `resource` (RFC 8707), when sent, must be this server.
    fn check_resource(&self, headers: &HeaderMap) -> Result<(), OAuthError> {
        let Some(resource) = self.get("resource") else { return Ok(()) };
        let issuer = issuer(headers).map_err(|_| OAuthError::invalid_request("the Host header is missing or wrong"))?;
        if is_this_server(Some(resource), &issuer) {
            Ok(())
        } else {
            Err(OAuthError::new("invalid_target", "tokens are for this server only (resource)"))
        }
    }
}

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

/// A token answer (RFC 6749 §5.1), never cached.
fn tokens(issued: Issued) -> Response {
    let mut body = json!({
        "access_token": issued.access_token,
        "token_type": "Bearer",
        "expires_in": issued.expires_in,
        "scope": issued.scopes.to_string(),
    });
    if let Some(refresh) = issued.refresh_token {
        body["refresh_token"] = json!(refresh);
    }
    (StatusCode::OK, NO_STORE, Json(body)).into_response()
}

/// `POST /api/oauth/token`: a code for the first tokens
/// (`authorization_code`), or a refresh token for new ones
/// (`refresh_token`, rotated every time).
pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let form = match Form::read(&headers, body) {
        Ok(form) => form,
        Err(err) => return err.into_response(),
    };
    let answer = match form.get("grant_type") {
        Some("authorization_code") => exchange_code(&state, &headers, ip, &form).await,
        Some("refresh_token") => refresh(&state, &headers, ip, &form).await,
        Some(_) => Err(OAuthError::new("unsupported_grant_type", "pley knows authorization_code and refresh_token")),
        None => Err(OAuthError::invalid_request("grant_type is missing")),
    };
    answer.unwrap_or_else(IntoResponse::into_response)
}

/// grant_type=authorization_code (OAuth 2.1 §4.1.3). Anything wrong leaves
/// the code as it was; a right exchange after the first ends what the first
/// one got (§7.5.3).
async fn exchange_code(
    state: &AppState,
    headers: &HeaderMap,
    ip: Option<String>,
    form: &Form,
) -> Result<Response, OAuthError> {
    let code = form.required("code")?;
    let client_id = form.required("client_id")?;
    let verifier = form.required("code_verifier")?;
    if !codes::is_verifier(verifier) {
        return Err(OAuthError::invalid_request("code_verifier must be 43 to 128 of A-Z a-z 0-9 . _ ~ -"));
    }
    form.check_resource(headers)?;
    let grant = match state.oauth.codes.redeem(code, client_id, form.get("redirect_uri"), verifier) {
        Redeem::First(grant) => grant,
        Redeem::Invalid(why) => return Err(OAuthError::invalid_grant(why)),
        Redeem::Replay(grant_id) => {
            tracing::warn!(
                "{client_id} exchanged a code a second time (someone else may have it): its access is ended"
            );
            if let Some(grant_id) = grant_id {
                if let Err(err) = state.auth.end_grant(grant_id).await {
                    tracing::error!("could not end an app's access: {err}");
                }
            }
            return Err(OAuthError::invalid_grant("the code was used already"));
        }
    };
    // Things may have changed in the minute since the person allowed it.
    let user = match crate::auth::AuthUser::load(&state.db, grant.user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            state.oauth.codes.failed(code);
            return Err(OAuthError::invalid_grant("the account is disabled"));
        }
        Err(err) => {
            state.oauth.codes.failed(code);
            return Err(OAuthError::server_error(err));
        }
    };
    if clients::builtin(&grant.app.id).is_none() && !state.settings.get().third_party_apps {
        state.oauth.codes.failed(code);
        return Err(OAuthError::invalid_grant("apps by other publishers can't sign in to this server"));
    }
    let issued = state
        .auth
        .issue(NewGrant {
            user_id: user.id,
            approved_by: user.id,
            client_id: grant.app.id.clone(),
            client_name: grant.app.name.clone(),
            scopes: grant.scopes,
            refresh: grant.app.refresh,
            first_party: grant.app.first_party,
            user_agent: user_agent(headers),
            ip,
        })
        .await;
    let issued = match issued {
        Ok(issued) => issued,
        Err(err) => {
            state.oauth.codes.failed(code);
            return Err(OAuthError::server_error(err));
        }
    };
    if state.oauth.codes.issued(code, issued.grant_id) {
        tracing::warn!(
            "{} exchanged a code a second time while it was being issued (someone else may have it): its access is \
             ended",
            grant.app.label()
        );
        if let Err(err) = state.auth.end_grant(issued.grant_id).await {
            tracing::error!("could not end an app's access: {err}");
        }
        return Err(OAuthError::invalid_grant("the code was used already"));
    }
    tracing::info!("{} may use {}'s account ({})", grant.app.label(), user.username, issued.scopes);
    Ok(tokens(issued))
}

/// grant_type=refresh_token (OAuth 2.1 §4.3): rotated every time; a spent
/// one shown again ends the app's access (see [`grants::refresh`]).
async fn refresh(
    state: &AppState,
    headers: &HeaderMap,
    ip: Option<String>,
    form: &Form,
) -> Result<Response, OAuthError> {
    let token = form.required("refresh_token")?;
    let client_id = form.required("client_id")?;
    let scope = match form.get("scope").map(Scopes::parse) {
        None => None,
        Some(Ok(scope)) => Some(scope),
        Some(Err(_)) => return Err(OAuthError::new("invalid_scope", "the scope names something pley doesn't offer")),
    };
    form.check_resource(headers)?;
    if clients::builtin(client_id).is_none() && !state.settings.get().third_party_apps {
        return Err(OAuthError::invalid_grant("apps by other publishers can't sign in to this server"));
    }
    let input = RefreshInput {
        token: token.to_owned(),
        client_id: client_id.to_owned(),
        scope,
        user_agent: user_agent(headers),
        ip,
    };
    match state.auth.refresh(input).await {
        Ok(issued) => Ok(tokens(issued)),
        Err(RefreshError::WiderScope) => {
            Err(OAuthError::new("invalid_scope", "the app asked for more than it was allowed"))
        }
        Err(RefreshError::Internal(err)) => Err(OAuthError::server_error(err)),
        Err(err) => Err(OAuthError::invalid_grant(err.to_string())),
    }
}

/// `POST /api/oauth/revoke` (RFC 7009): an app signs out. A token of that
/// app (access or refresh, spent or expired) ends its whole grant. Every
/// well-formed request gets 200 and an empty body, whatever the token
/// (RFC 7009 §2.2).
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let form = match Form::read(&headers, body) {
        Ok(form) => form,
        Err(err) => return err.into_response(),
    };
    let (token, client_id) = match (form.required("token"), form.required("client_id")) {
        (Ok(token), Ok(client_id)) => (token, client_id),
        (Err(err), _) | (_, Err(err)) => return err.into_response(),
    };
    let grant = grants::find_grant_of_token(&state.db, token).await.ok().flatten();
    if let Some((grant_id, owner)) = grant.filter(|(_, owner)| owner == client_id) {
        let described = describe(&state, grant_id).await;
        if state.auth.revoke_app_token(token, Some(&owner)).await {
            if let Some((app, user)) = described {
                tracing::info!("{app} was signed out of {user}'s account");
            }
        }
    }
    (StatusCode::OK, NO_STORE).into_response()
}

/// How a grant is called in logs: the app and the profile it acts as.
async fn describe(state: &AppState, grant_id: i64) -> Option<(String, String)> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT g.client_id, u.username FROM app_grants g JOIN users u ON u.id = g.user_id WHERE g.id = ?",
    )
    .bind(grant_id)
    .fetch_optional(&state.db.read)
    .await
    .ok()
    .flatten();
    row.map(|(client_id, user)| (app_label(&client_id), user))
}

/// pley's own apps by name, others by host.
fn app_label(client_id: &str) -> String {
    match clients::builtin(client_id) {
        Some(client) => client.name.clone().unwrap_or_else(|| client.host.clone()),
        None => client_host(client_id),
    }
}

fn client_host(client_id: &str) -> String {
    Url::parse(client_id).ok().and_then(|url| url.host_str().map(str::to_owned)).unwrap_or_else(|| client_id.to_owned())
}

// ------------------------------------------------------------------ apps lists

#[derive(Serialize)]
pub struct Person {
    id: i64,
    name: String,
}

/// An app allowed to use an account (`api/apps.ts`'s AppGrant).
#[derive(Serialize)]
pub struct AppGrant {
    id: i64,
    client_id: String,
    /// pley's own apps: their name. Others: what the app called itself
    /// when it was allowed, which pley can't check.
    name: Option<String>,
    first_party: bool,
    host: String,
    scopes: Vec<&'static str>,
    /// The profile it acts as.
    profile: Person,
    approved_by: Person,
    /// Of its last token request.
    device: Option<String>,
    ip: Option<String>,
    created_at: i64,
    last_used_at: i64,
}

impl From<GrantRow> for AppGrant {
    fn from(row: GrantRow) -> Self {
        let builtin = clients::builtin(&row.client_id);
        AppGrant {
            id: row.id,
            name: builtin.map_or(row.client_name, |client| client.name.clone()),
            first_party: builtin.is_some(),
            host: client_host(&row.client_id),
            client_id: row.client_id,
            scopes: row.scopes.names(),
            profile: Person { id: row.user_id, name: row.user_name },
            approved_by: Person { id: row.approved_by, name: row.approved_by_name },
            device: row.user_agent.as_deref().map(|ua| crate::accounts::device_name(Some(ua))),
            ip: row.last_ip,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

async fn apps_of(state: &AppState, user_id: i64) -> ApiResult<Vec<AppGrant>> {
    Ok(grants::list(&state.db, user_id).await?.into_iter().map(AppGrant::from).collect())
}

/// Ends grant `grant_id` when it acts as `user_id` or they allowed it.
async fn sign_out_app(state: &AppState, user_id: i64, grant_id: i64) -> ApiResult<StatusCode> {
    let listed = grants::list(&state.db, user_id).await?.into_iter().any(|row| row.id == grant_id);
    if !listed {
        return Err(AppError::not_found("no such app"));
    }
    let described = describe(state, grant_id).await;
    if state.auth.end_grant(grant_id).await? {
        if let Some((app, user)) = described {
            tracing::info!("{app} was signed out of {user}'s account");
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/me/apps`: the apps acting as the caller or allowed by them,
/// most recently used first.
pub async fn my_apps(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<AppGrant>>> {
    Ok(Json(apps_of(&state, user.id).await?))
}

/// `DELETE /api/me/apps/{id}`.
pub async fn sign_out_my_app(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    sign_out_app(&state, user.id, id).await
}

/// `GET /api/users/{id}/apps` (administrators).
pub async fn user_apps(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<AppGrant>>> {
    Ok(Json(apps_of(&state, id).await?))
}

/// `DELETE /api/users/{id}/apps/{grant}` (administrators).
pub async fn sign_out_user_app(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path((id, grant)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    sign_out_app(&state, id, grant).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, value.parse().unwrap());
        }
        headers
    }

    #[test]
    fn the_issuer_is_the_address_asked_at() {
        let at = |pairs: &[(&'static str, &str)]| issuer(&headers(pairs)).ok();
        assert_eq!(at(&[("host", "Pley.Local:7539")]).as_deref(), Some("http://pley.local:7539"));
        assert_eq!(at(&[("host", "pley.local:80")]).as_deref(), Some("http://pley.local"));
        assert_eq!(
            at(&[("host", "pley.example.com:443"), ("x-forwarded-proto", "https, http")]).as_deref(),
            Some("https://pley.example.com")
        );
        assert_eq!(at(&[("host", "[::1]:7539")]).as_deref(), Some("http://[::1]:7539"));
        assert_eq!(at(&[("host", "192.168.1.5:7539")]).as_deref(), Some("http://192.168.1.5:7539"));
        for bad in ["", "user@pley.local", "pley.local/path", "pley.local?x", "pley.local#x", "pley.local:port", "a b"]
        {
            assert_eq!(at(&[("host", bad)]), None, "{bad}");
        }
        assert_eq!(at(&[]), None, "no Host");
    }

    #[test]
    fn resources_name_this_server() {
        assert!(is_this_server(None, "http://pley.local:7539"));
        assert!(is_this_server(Some("http://pley.local:7539"), "http://pley.local:7539"));
        assert!(is_this_server(Some("http://pley.local:7539/"), "http://pley.local:7539"));
        assert!(!is_this_server(Some("http://pley.local:7539/api"), "http://pley.local:7539"));
        assert!(!is_this_server(Some("https://pley.local:7539"), "http://pley.local:7539"));
    }

    #[test]
    fn parameters_come_once() {
        assert_eq!(parameters(b"a=1&b=%20x+y").unwrap(), [("a".into(), "1".into()), ("b".into(), " x y".into())]);
        assert_eq!(parameters(b"a=1&b=2&a=1"), Err("a".to_owned()));
        assert_eq!(parameters(b"a=&a"), Err("a".to_owned()), "empty ones count");
        assert_eq!(shown_name("client_id"), Some("client_id"));
        assert_eq!(shown_name("<b>"), None);
    }
}
