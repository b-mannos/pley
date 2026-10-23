//! Single sign-on with an OpenID Connect provider (Authentik, Authelia,
//! Keycloak, Pocket ID, Kanidm, Microsoft Entra…): the authorization code
//! flow with PKCE, a `state` bound to the browser by a cookie (so nobody can
//! sign someone else's browser in with their own sign-in link), and a nonce
//! checked in the ID token.
//!
//! pley either has a client registered with the provider (a client ID, and
//! a secret for confidential clients) or, with providers that take them,
//! uses its client metadata document as its client ID (CIMD,
//! draft-ietf-oauth-client-id-metadata-document): the provider reads
//! `https://<pley>/api/auth/sso/client`, and nothing needs registering.
//!
//! Sign-ins run at pley's public address, where the provider sends people
//! back. What a sign-in in progress needs (its state, nonce and PKCE
//! verifier) travels in an encrypted cookie rather than in the server's
//! memory, so starting sign-ins costs pley nothing to keep.
//!
//! An identity at the provider (its issuer and the person's `sub`) signs in
//! to the pley account it was linked to from the Account page. Accounts are
//! never matched by name or e-mail address, which providers often let
//! people choose. Identities pley doesn't know get an account only when the
//! administrator allows it, and that account starts without libraries. An
//! administrator can also let only the members of one group at the provider
//! in.
//!
//! pley asks the provider only who signed in ([`scopes`]): `openid` and
//! `email` (to show which account is linked), `profile` only to name new
//! accounts, `groups` only to check the group. It asks for no API access and
//! no refresh token, uses the access token at most once (for the claims an
//! ID token lacks) and keeps no tokens of the provider: from the provider,
//! only the issuer, the `sub` and the e-mail address are stored.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, header};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use parking_lot::Mutex;
use rand_core::{OsRng, RngCore};
use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::db::Db;
use crate::settings::Settings;
use crate::util::now;

/// Where sign-ins start (a page the browser is sent to).
pub const START_PATH: &str = "/api/auth/sso/start";
/// Where the provider sends people back.
pub const CALLBACK_PATH: &str = "/api/auth/sso/callback";
/// pley's client metadata document: its client ID with CIMD.
pub const CLIENT_PATH: &str = "/api/auth/sso/client";
/// The sign-in in progress (sealed [`Flow`]).
pub const FLOW_COOKIE: &str = "pley_sso";
/// A signed-in user's request to link their identity at the provider.
pub const LINK_COOKIE: &str = "pley_sso_link";
const COOKIE_PATH: &str = "/api/auth/sso";
/// How long someone has to sign in at the provider (seconds).
pub const FLOW_TTL: i64 = 10 * 60;
/// How long a link request waits for its sign-in to start (seconds).
pub const LINK_TTL: i64 = 2 * 60;
/// How long the provider's discovery document and keys are reused.
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);
/// How long a provider that couldn't be reached isn't asked again.
const FAILURE_TTL: Duration = Duration::from_secs(15);
/// The provider's keys are fetched again for an unknown key at most this often.
const KEY_REFRESH: Duration = Duration::from_secs(30);
/// Answers from the provider larger than this are refused.
const MAX_ANSWER: usize = 1024 * 1024;

#[derive(Debug, Clone, thiserror::Error)]
pub enum SsoError {
    #[error("single sign-on is off")]
    Off,
    /// What the provider said or did, or why it couldn't be reached.
    #[error("{0}")]
    Provider(String),
    #[error("the sign-in expired, or was started in another browser")]
    Expired,
    /// An ID token pley refuses.
    #[error("{0}")]
    Invalid(String),
    /// Someone outside the group that may sign in.
    #[error("{0}")]
    NotAllowed(String),
}

impl SsoError {
    /// What the web app is told (`?sso=…`): the details are in the log.
    pub fn code(&self) -> &'static str {
        match self {
            SsoError::Off => "off",
            SsoError::Provider(_) => "provider",
            SsoError::Expired => "expired",
            SsoError::Invalid(_) => "invalid",
            SsoError::NotAllowed(_) => "group",
        }
    }
}

fn provider_error(what: &str, err: reqwest::Error) -> SsoError {
    let why = if err.is_timeout() {
        "no answer in time".to_owned()
    } else if err.is_connect() {
        "nothing answers there".to_owned()
    } else {
        err.without_url().to_string()
    };
    SsoError::Provider(format!("couldn't reach the provider's {what}: {why}"))
}

/// The provider's discovery document (OpenID Connect Discovery 1.0).
#[derive(Debug, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    #[serde(default)]
    userinfo_endpoint: Option<String>,
    #[serde(default)]
    token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    id_token_signing_alg_values_supported: Vec<String>,
    /// CIMD: client IDs may be URLs of client metadata documents.
    #[serde(default)]
    client_id_metadata_document_supported: bool,
}

/// What pley needs to know about the provider.
#[derive(Debug, Clone)]
pub struct Provider {
    pub issuer: String,
    authorization: Url,
    token: Url,
    jwks: Url,
    userinfo: Option<Url>,
    auth_methods: Vec<String>,
    algorithms: Vec<String>,
    /// Takes client metadata documents as client IDs.
    pub cimd: bool,
}

/// A sign-in on its way through the provider, sealed into [`FLOW_COOKIE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    state: String,
    nonce: String,
    verifier: String,
    client_id: String,
    redirect_uri: String,
    /// The page to show afterwards.
    pub next: String,
    /// Link the identity to this signed-in user instead of signing in.
    pub link: Option<i64>,
    expires: i64,
}

#[derive(Serialize, Deserialize)]
struct LinkRequest {
    user: i64,
    expires: i64,
}

/// Who the provider says signed in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    /// The groups the provider says they are in (checked, never stored).
    pub groups: Vec<String>,
}

/// What pley asks the provider for: who signed in, and nothing more than
/// the settings need.
pub fn scopes(settings: &Settings) -> String {
    let mut scopes = String::from("openid email");
    if settings.sso_new_accounts {
        scopes.push_str(" profile");
    }
    if !settings.sso_group.is_empty() {
        scopes.push_str(" groups");
    }
    scopes
}

/// A claim that is one string or a list of them (`aud`, `groups`).
fn strings<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(de)? {
        OneOrMany::One(one) => vec![one],
        OneOrMany::Many(many) => many,
    })
}

#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default, deserialize_with = "strings")]
    aud: Vec<String>,
    #[serde(flatten)]
    person: PersonClaims,
}

/// What the ID token or the userinfo endpoint says about the person.
#[derive(Default, Deserialize)]
struct PersonClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "strings")]
    groups: Vec<String>,
}

impl PersonClaims {
    fn fill(self, identity: &mut Identity) {
        let text = |value: Option<String>| {
            value.map(|v| v.trim().chars().take(200).collect::<String>()).filter(|v| !v.is_empty())
        };
        let email_verified = self.email_verified;
        identity.email = identity.email.take().or(text(self.email.filter(|_| email_verified != Some(false))));
        identity.username = identity.username.take().or(text(self.preferred_username));
        identity.name = identity.name.take().or(text(self.name));
        if identity.groups.is_empty() {
            identity.groups = self.groups.into_iter().take(1000).collect();
        }
    }
}

#[derive(Deserialize)]
struct UserInfo {
    sub: String,
    #[serde(flatten)]
    person: PersonClaims,
}

#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// A key set, each key read on its own: keys pley can't use (encryption
/// keys, curves it doesn't know) don't spoil the others.
#[derive(Deserialize)]
struct KeySet {
    keys: Vec<serde_json::Value>,
}

/// 256 random bits, as URL-safe text.
fn random() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The PKCE challenge (S256) for a verifier.
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn web_url(value: &str, what: &str) -> Result<Url, SsoError> {
    Url::parse(value)
        .ok()
        .filter(|u| matches!(u.scheme(), "https" | "http") && u.host_str().is_some())
        .ok_or_else(|| SsoError::Provider(format!("the provider's {what} isn't a web address: {value}")))
}

fn algorithm_name(alg: Algorithm) -> &'static str {
    match alg {
        Algorithm::HS256 => "HS256",
        Algorithm::HS384 => "HS384",
        Algorithm::HS512 => "HS512",
        Algorithm::ES256 => "ES256",
        Algorithm::ES384 => "ES384",
        Algorithm::RS256 => "RS256",
        Algorithm::RS384 => "RS384",
        Algorithm::RS512 => "RS512",
        Algorithm::PS256 => "PS256",
        Algorithm::PS384 => "PS384",
        Algorithm::PS512 => "PS512",
        Algorithm::EdDSA => "EdDSA",
    }
}

/// Whether the provider's key `jwk` may have signed with `alg`.
fn key_fits(jwk: &Jwk, alg: Algorithm) -> bool {
    let family = match &jwk.algorithm {
        AlgorithmParameters::RSA(_) => matches!(
            alg,
            Algorithm::RS256
                | Algorithm::RS384
                | Algorithm::RS512
                | Algorithm::PS256
                | Algorithm::PS384
                | Algorithm::PS512
        ),
        AlgorithmParameters::EllipticCurve(_) => matches!(alg, Algorithm::ES256 | Algorithm::ES384),
        AlgorithmParameters::OctetKeyPair(_) => alg == Algorithm::EdDSA,
        AlgorithmParameters::OctetKey(_) => false,
    };
    // A key that names its algorithm or use is only good for those.
    let named = jwk.common.key_algorithm.is_none_or(|a| format!("{a:?}") == algorithm_name(alg));
    let signs = !matches!(jwk.common.public_key_use, Some(PublicKeyUse::Encryption | PublicKeyUse::Other(_)));
    family && named && signs
}

/// `application/x-www-form-urlencoded`, as RFC 6749 §2.3.1 wants client
/// credentials before HTTP Basic encodes them.
fn form_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// `next` when it is a page of the web app (`/library/…?x=1`), `/` otherwise.
pub fn local_path(next: Option<&str>) -> String {
    let Some(next) = next.filter(|n| n.len() <= 2048) else { return "/".into() };
    if !next.starts_with('/') || next.starts_with("//") || next.contains('\\') || next.chars().any(char::is_control) {
        return "/".into();
    }
    let base = Url::parse("http://pley.invalid/").expect("base URL");
    let Ok(url) = base.join(next) else { return "/".into() };
    if url.origin() != base.origin() || url.path().starts_with("/api/") {
        return "/".into();
    }
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    }
}

/// A cookie for the sign-in endpoints only; `max_age` 0 deletes it.
pub fn cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{name}={value}; Path={COOKIE_PATH}; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

/// The value of the cookie `name` in a request.
pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|cookies| {
        cookies.split(';').find_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            (key == name && !value.is_empty()).then(|| value.to_owned())
        })
    })
}

/// Whether a request was sent to the host of `public_url` (what the
/// browser sees may differ behind a proxy that rewrites `Host`).
pub fn at_public_address(headers: &HeaderMap, public_url: &str) -> bool {
    let Ok(public) = Url::parse(public_url) else { return false };
    let Some(host) = public.host_str() else { return false };
    let authority = match public.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    headers.get(header::HOST).and_then(|v| v.to_str().ok()).is_some_and(|h| h.eq_ignore_ascii_case(&authority))
}

struct Cached<T> {
    key: String,
    at: Instant,
    value: Result<Arc<T>, SsoError>,
}

impl<T> Cached<T> {
    fn fresh(&self, key: &str, ttl: Duration) -> Option<Result<Arc<T>, SsoError>> {
        let ttl = if self.value.is_ok() { ttl } else { FAILURE_TTL };
        (self.key == key && self.at.elapsed() < ttl).then(|| self.value.clone())
    }
}

pub struct Sso {
    /// Discovery documents and keys: may follow a redirect or two.
    http: reqwest::Client,
    /// The token request, which may carry the client secret: never redirected.
    token_http: reqwest::Client,
    /// Seals the cookies of sign-ins in progress; new each start, so a
    /// restart ends them.
    key: LessSafeKey,
    provider: Mutex<Option<Cached<Provider>>>,
    keys: Mutex<Option<Cached<Vec<Jwk>>>>,
}

impl Default for Sso {
    fn default() -> Self {
        Self::new()
    }
}

impl Sso {
    pub fn new() -> Self {
        let client = |redirects| {
            reqwest::Client::builder()
                .user_agent(concat!("pley/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(10))
                .redirect(redirects)
                .build()
                .expect("http client")
        };
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let key = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, &secret).expect("sealing key"));
        Self {
            http: client(reqwest::redirect::Policy::limited(3)),
            token_http: client(reqwest::redirect::Policy::none()),
            key,
            provider: Mutex::new(None),
            keys: Mutex::new(None),
        }
    }

    fn seal(&self, purpose: &'static str, value: &impl Serialize) -> String {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let mut data = serde_json::to_vec(value).expect("serializable");
        self.key
            .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(purpose), &mut data)
            .expect("sealing");
        let mut sealed = nonce.to_vec();
        sealed.extend(data);
        URL_SAFE_NO_PAD.encode(sealed)
    }

    fn open<T: DeserializeOwned>(&self, purpose: &'static str, sealed: &str) -> Option<T> {
        let mut nonce = URL_SAFE_NO_PAD.decode(sealed).ok()?;
        if nonce.len() < NONCE_LEN {
            return None;
        }
        let mut data = nonce.split_off(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(&nonce).ok()?;
        let plain = self.key.open_in_place(nonce, Aad::from(purpose), &mut data).ok()?;
        serde_json::from_slice(plain).ok()
    }

    /// Where the provider sends people back.
    pub fn redirect_uri(settings: &Settings) -> String {
        format!("{}{CALLBACK_PATH}", settings.public_url)
    }

    /// Where sign-ins start.
    pub fn start_url(settings: &Settings) -> String {
        format!("{}{START_PATH}", settings.public_url)
    }

    /// The client ID pley signs in with: the registered one, or the address
    /// of its client metadata document.
    pub fn client_id(settings: &Settings) -> String {
        if settings.sso_client_id.is_empty() {
            format!("{}{CLIENT_PATH}", settings.public_url)
        } else {
            settings.sso_client_id.clone()
        }
    }

    /// pley's client metadata document (CIMD), served at its client ID.
    pub fn client_metadata(settings: &Settings) -> serde_json::Value {
        let base = settings.public_url.as_str();
        serde_json::json!({
            "client_id": format!("{base}{CLIENT_PATH}"),
            "client_name": settings.server_name,
            "client_uri": base,
            "logo_uri": format!("{base}/icon-512.png"),
            "redirect_uris": [Self::redirect_uri(settings)],
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
            "scope": scopes(settings),
        })
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &Url, what: &str) -> Result<T, SsoError> {
        let response = self.http.get(url.clone()).send().await.map_err(|e| provider_error(what, e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(SsoError::Provider(format!("the provider's {what} answered HTTP {}", status.as_u16())));
        }
        read_json(response, what).await
    }

    /// The provider at `issuer`, from its discovery document.
    async fn discover(&self, issuer: &str) -> Result<Provider, SsoError> {
        let url = web_url(&format!("{}/.well-known/openid-configuration", issuer.trim_end_matches('/')), "issuer")?;
        let doc: Discovery = self.get_json(&url, "discovery document").await?;
        // OpenID Connect Discovery §4.3: exactly the issuer asked for.
        if doc.issuer != issuer {
            return Err(SsoError::Provider(format!(
                "the provider's issuer is {}, not {issuer}: enter it exactly so",
                doc.issuer
            )));
        }
        Ok(Provider {
            issuer: doc.issuer,
            authorization: web_url(&doc.authorization_endpoint, "authorization endpoint")?,
            token: web_url(&doc.token_endpoint, "token endpoint")?,
            jwks: web_url(&doc.jwks_uri, "key set")?,
            userinfo: doc.userinfo_endpoint.as_deref().map(|u| web_url(u, "userinfo endpoint")).transpose()?,
            auth_methods: doc.token_endpoint_auth_methods_supported,
            algorithms: doc.id_token_signing_alg_values_supported,
            cimd: doc.client_id_metadata_document_supported,
        })
    }

    /// The configured provider, discovered once an hour.
    pub async fn provider(&self, settings: &Settings) -> Result<Arc<Provider>, SsoError> {
        let issuer = settings.sso_issuer.as_str();
        if let Some(cached) = self.provider.lock().as_ref().and_then(|c| c.fresh(issuer, CACHE_TTL)) {
            return cached;
        }
        let value = self.discover(issuer).await.map(Arc::new);
        *self.provider.lock() = Some(Cached { key: issuer.to_owned(), at: Instant::now(), value: value.clone() });
        value
    }

    async fn fetch_keys(&self, url: &Url) -> Result<Vec<Jwk>, SsoError> {
        let set: KeySet = self.get_json(url, "key set").await?;
        Ok(set.keys.into_iter().filter_map(|key| serde_json::from_value(key).ok()).collect())
    }

    async fn key_set(&self, provider: &Provider, refresh: bool) -> Result<Arc<Vec<Jwk>>, SsoError> {
        let url = provider.jwks.as_str();
        let ttl = if refresh { KEY_REFRESH } else { CACHE_TTL };
        if let Some(cached) = self.keys.lock().as_ref().and_then(|c| c.fresh(url, ttl)) {
            return cached;
        }
        let value = self.fetch_keys(&provider.jwks).await.map(Arc::new);
        *self.keys.lock() = Some(Cached { key: url.to_owned(), at: Instant::now(), value: value.clone() });
        value
    }

    /// The provider's key that signed with `alg` (`kid`), fetching the keys
    /// again once when it isn't known (keys rotate).
    async fn key(&self, provider: &Provider, kid: Option<&str>, alg: Algorithm) -> Result<DecodingKey, SsoError> {
        for refresh in [false, true] {
            let keys = self.key_set(provider, refresh).await?;
            let jwk =
                keys.iter().filter(|k| key_fits(k, alg)).find(|k| kid.is_none() || k.common.key_id.as_deref() == kid);
            if let Some(jwk) = jwk {
                return DecodingKey::from_jwk(jwk)
                    .map_err(|e| SsoError::Invalid(format!("the provider's signing key can't be read: {e}")));
            }
        }
        Err(SsoError::Invalid("the ID token was signed with a key the provider doesn't list".into()))
    }

    /// Starts a sign-in (or, with `link`, the linking of a signed-in user's
    /// identity): the cookie that binds it to the browser, and the
    /// provider's page to send that browser to.
    pub async fn begin(&self, settings: &Settings, next: String, link: Option<i64>) -> Result<(String, Url), SsoError> {
        if !settings.sso_enabled {
            return Err(SsoError::Off);
        }
        let provider = self.provider(settings).await?;
        if settings.sso_client_id.is_empty() && !provider.cimd {
            return Err(SsoError::Provider(
                "the provider doesn't take client metadata documents: register pley with it and enter its client ID"
                    .into(),
            ));
        }
        let flow = Flow {
            state: random(),
            nonce: random(),
            verifier: random(),
            client_id: Self::client_id(settings),
            redirect_uri: Self::redirect_uri(settings),
            next,
            link,
            expires: now() + FLOW_TTL,
        };
        let mut url = provider.authorization.clone();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &flow.client_id)
            .append_pair("redirect_uri", &flow.redirect_uri)
            .append_pair("scope", &scopes(settings))
            .append_pair("state", &flow.state)
            .append_pair("nonce", &flow.nonce)
            .append_pair("code_challenge", &challenge(&flow.verifier))
            .append_pair("code_challenge_method", "S256");
        if link.is_some() {
            // Linking: whoever is to sign in with this identity signs in at
            // the provider now, not a session someone left there.
            url.query_pairs_mut().append_pair("prompt", "login");
        }
        Ok((self.seal("pley sso flow", &flow), url))
    }

    /// The sign-in the provider sent back with `state`, from the cookie of
    /// the browser that started it.
    pub fn flow(&self, cookie: Option<&str>, state: &str) -> Result<Flow, SsoError> {
        let flow: Flow = cookie.and_then(|c| self.open("pley sso flow", c)).ok_or(SsoError::Expired)?;
        if flow.expires <= now() || !constant_time_eq(&flow.state, state) {
            return Err(SsoError::Expired);
        }
        Ok(flow)
    }

    /// The cookie asking the next sign-in to link `user_id`'s identity.
    pub fn link_request(&self, user_id: i64) -> String {
        self.seal("pley sso link", &LinkRequest { user: user_id, expires: now() + LINK_TTL })
    }

    /// Whose identity a link request cookie asks to link.
    pub fn linking(&self, cookie: &str) -> Option<i64> {
        let request: LinkRequest = self.open("pley sso link", cookie)?;
        (request.expires > now()).then_some(request.user)
    }

    /// Completes `flow` with the provider's `code`: who signed in, when the
    /// settings let them in.
    pub async fn finish(&self, settings: &Settings, flow: &Flow, code: &str) -> Result<Identity, SsoError> {
        if !settings.sso_enabled {
            return Err(SsoError::Off);
        }
        let provider = self.provider(settings).await?;
        let mut form = vec![
            ("grant_type", "authorization_code".to_owned()),
            ("code", code.to_owned()),
            ("redirect_uri", flow.redirect_uri.clone()),
            ("code_verifier", flow.verifier.clone()),
        ];
        let mut request = self.token_http.post(provider.token.clone());
        // A client metadata document makes a public client: no secret.
        let secret = if settings.sso_client_id.is_empty() { "" } else { settings.sso_client_secret.as_str() };
        let post = provider.auth_methods.iter().any(|m| m == "client_secret_post")
            && !provider.auth_methods.iter().any(|m| m == "client_secret_basic");
        if secret.is_empty() {
            form.push(("client_id", flow.client_id.clone()));
        } else if post {
            form.push(("client_id", flow.client_id.clone()));
            form.push(("client_secret", secret.to_owned()));
        } else {
            request = request.basic_auth(form_encode(&flow.client_id), Some(form_encode(secret)));
        }
        let response = request.form(&form).send().await.map_err(|e| provider_error("token endpoint", e))?;
        let status = response.status().as_u16();
        let answer: TokenAnswer = read_json(response, "token endpoint")
            .await
            .map_err(|_| SsoError::Provider(format!("the provider's token endpoint answered HTTP {status}")))?;
        if let Some(error) = answer.error {
            let error: String = error.chars().filter(|c| c.is_ascii_graphic()).take(64).collect();
            return Err(SsoError::Provider(match answer.error_description {
                Some(why) => {
                    format!("the provider refused the sign-in ({error}): {}", why.chars().take(300).collect::<String>())
                }
                None => format!("the provider refused the sign-in ({error})"),
            }));
        }
        let token = answer.id_token.ok_or_else(|| {
            SsoError::Provider("the provider sent no ID token: does it allow the openid scope?".into())
        })?;
        let mut identity = self.verify(settings, &provider, flow, &token).await?;
        // Some providers (Authelia, for one) put little in ID tokens and the
        // rest behind the userinfo endpoint.
        let lacking = identity.email.is_none()
            || (settings.sso_new_accounts && identity.username.is_none() && identity.name.is_none())
            || (!settings.sso_group.is_empty() && identity.groups.is_empty());
        if let (true, Some(url), Some(access)) = (lacking, &provider.userinfo, answer.access_token.as_deref()) {
            match self.userinfo(url, access).await {
                // OpenID Connect Core §5.3.2: only about the same person.
                Ok(info) if info.sub == identity.subject => info.person.fill(&mut identity),
                Ok(_) => tracing::warn!("single sign-on: the provider's userinfo is about someone else; ignored"),
                Err(err) => tracing::debug!("single sign-on: no userinfo: {err}"),
            }
        }
        let group = settings.sso_group.trim_start_matches('/');
        if !group.is_empty() && !identity.groups.iter().any(|g| g.trim_start_matches('/') == group) {
            return Err(SsoError::NotAllowed(if identity.groups.is_empty() {
                format!(
                    "the provider said nothing about groups: allow pley the groups scope, or put groups in its ID tokens (only members of {group} may sign in)"
                )
            } else {
                format!("{} isn't in {group}, the group that may sign in", identity.subject)
            }));
        }
        Ok(identity)
    }

    async fn userinfo(&self, url: &Url, access_token: &str) -> Result<UserInfo, SsoError> {
        let response = self
            .token_http
            .get(url.clone())
            .bearer_auth(access_token)
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| provider_error("userinfo endpoint", e))?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(SsoError::Provider(format!("the provider's userinfo endpoint answered HTTP {status}")));
        }
        read_json(response, "userinfo endpoint").await
    }

    /// Checks an ID token: signed by the provider, for this client and this
    /// sign-in (the nonce), and current.
    async fn verify(
        &self,
        settings: &Settings,
        provider: &Provider,
        flow: &Flow,
        token: &str,
    ) -> Result<Identity, SsoError> {
        let header = jsonwebtoken::decode_header(token)
            .map_err(|_| SsoError::Invalid("the ID token isn't one pley can read".into()))?;
        let alg = header.alg;
        if !provider.algorithms.is_empty() && !provider.algorithms.iter().any(|a| a == algorithm_name(alg)) {
            return Err(SsoError::Invalid(format!(
                "the ID token is signed with {}, which the provider doesn't list",
                algorithm_name(alg)
            )));
        }
        let key = match alg {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
                // OpenID Connect Core §10.1: keyed with the client secret.
                let secret = settings.sso_client_secret.as_str();
                if secret.is_empty() || settings.sso_client_id.is_empty() {
                    return Err(SsoError::Invalid(
                        "the ID token is signed with a shared secret, and pley has none".into(),
                    ));
                }
                DecodingKey::from_secret(secret.as_bytes())
            }
            _ => self.key(provider, header.kid.as_deref(), alg).await?,
        };
        let mut validation = Validation::new(alg);
        validation.set_audience(&[&flow.client_id]);
        validation.set_issuer(&[&provider.issuer]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 60;
        let claims = jsonwebtoken::decode::<IdClaims>(token, &key, &validation)
            .map_err(|e| SsoError::Invalid(format!("the ID token was refused: {e}")))?
            .claims;
        if !claims.nonce.as_deref().is_some_and(|n| constant_time_eq(n, &flow.nonce)) {
            return Err(SsoError::Invalid("the ID token wasn't issued for this sign-in".into()));
        }
        // OpenID Connect Core §3.1.3.7: with several audiences, pley is the party it was issued to.
        if claims.aud.len() > 1 && claims.azp.as_deref() != Some(flow.client_id.as_str()) {
            return Err(SsoError::Invalid("the ID token was issued to another client".into()));
        }
        if claims.sub.is_empty() || claims.sub.len() > 255 {
            return Err(SsoError::Invalid("the ID token names nobody".into()));
        }
        let mut identity = Identity { issuer: provider.issuer.clone(), subject: claims.sub, ..Default::default() };
        claims.person.fill(&mut identity);
        Ok(identity)
    }

    /// For the settings' Test button: whether `issuer` answers as an
    /// OpenID Connect provider, and how pley would sign in with it.
    pub async fn check(&self, issuer: &str, client_id: &str, public_url: &str) -> Result<String, SsoError> {
        let provider = self.discover(issuer).await?;
        let keys = self.fetch_keys(&provider.jwks).await?;
        if keys.is_empty() && !provider.algorithms.iter().all(|a| a.starts_with("HS")) {
            return Err(SsoError::Provider("found the provider, but its key set has no key pley can use".into()));
        }
        if client_id.is_empty() && !provider.cimd {
            return Err(SsoError::Provider(
                "found the provider, but it doesn't take client metadata documents: register pley with it and enter its client ID"
                    .into(),
            ));
        }
        let how = if client_id.is_empty() {
            format!("it reads pley's client metadata document at {public_url}{CLIENT_PATH}")
        } else {
            format!("client {client_id} needs {public_url}{CALLBACK_PATH} as a redirect URI")
        };
        let plain = if provider.token.scheme() == "http" { " Its token endpoint isn't encrypted (http)." } else { "" };
        Ok(format!("Found {}: {how}.{plain}", provider.issuer))
    }
}

/// A JSON answer of at most [`MAX_ANSWER`] bytes.
async fn read_json<T: DeserializeOwned>(mut response: reqwest::Response, what: &str) -> Result<T, SsoError> {
    let unexpected = || SsoError::Provider(format!("the provider's {what} isn't what pley expects"));
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| provider_error(what, e))? {
        if body.len() + chunk.len() > MAX_ANSWER {
            return Err(unexpected());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| unexpected())
}

// ------------------------------------------------------------------ accounts

/// The user `identity` is linked to: its id, and whether it is disabled.
pub async fn linked_user(db: &Db, identity: &Identity) -> sqlx::Result<Option<(i64, bool)>> {
    sqlx::query_as(
        "SELECT u.id, u.disabled FROM user_identities i JOIN users u ON u.id = i.user_id
         WHERE i.issuer = ? AND i.subject = ?",
    )
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .fetch_optional(&db.read)
    .await
}

/// Links `identity` to `user_id` (replacing the user's earlier identity at
/// that provider); `false` when it belongs to someone else.
pub async fn link(db: &Db, identity: &Identity, user_id: i64) -> sqlx::Result<bool> {
    let mut tx = db.write.begin().await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM user_identities WHERE issuer = ? AND subject = ?")
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .fetch_optional(&mut *tx)
        .await?;
    if owner.is_some_and(|owner| owner != user_id) {
        return Ok(false);
    }
    sqlx::query("DELETE FROM user_identities WHERE user_id = ? AND issuer = ?")
        .bind(user_id)
        .bind(&identity.issuer)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO user_identities (issuer, subject, user_id, email, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .bind(user_id)
        .bind(&identity.email)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Notes a sign-in through `identity`.
pub async fn used(db: &Db, identity: &Identity) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE user_identities SET last_used_at = ?, email = COALESCE(?, email) WHERE issuer = ? AND subject = ?",
    )
    .bind(now())
    .bind(&identity.email)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .execute(&db.write)
    .await?;
    Ok(())
}

/// The identity at `issuer` linked to `user_id`: its e-mail address, if
/// the provider said.
pub async fn linked_identity(db: &Db, user_id: i64, issuer: &str) -> sqlx::Result<Option<Option<String>>> {
    sqlx::query_scalar("SELECT email FROM user_identities WHERE user_id = ? AND issuer = ?")
        .bind(user_id)
        .bind(issuer)
        .fetch_optional(&db.read)
        .await
}

pub async fn unlink(db: &Db, user_id: i64, issuer: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM user_identities WHERE user_id = ? AND issuer = ?")
        .bind(user_id)
        .bind(issuer)
        .execute(&db.write)
        .await?;
    Ok(done.rows_affected() > 0)
}

/// A username for someone new from the provider: their preferred username,
/// e-mail address or name, kept to the characters usernames allow.
pub fn username_for(identity: &Identity) -> String {
    let source =
        identity.username.as_deref().or(identity.email.as_deref()).or(identity.name.as_deref()).unwrap_or_default();
    let clean: String = source
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@') { c } else { '-' })
        .take(48)
        .collect();
    let clean = clean.trim_matches(['-', '.']).to_owned();
    if clean.is_empty() { "user".to_owned() } else { clean }
}

/// The password of accounts made through single sign-on: no password
/// matches it (Argon2 hashes start with `$`).
pub const NO_PASSWORD: &str = "!sso";

/// Makes the account for someone new from the provider: a regular user
/// without libraries (until an administrator gives them some), outside the
/// household's profile switcher and without a password, linked to
/// `identity`.
pub async fn create_user(db: &Db, identity: &Identity) -> sqlx::Result<i64> {
    let base = username_for(identity);
    let mut tx = db.write.begin().await?;
    let mut username = base.clone();
    for n in 2.. {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = ?)")
            .bind(&username)
            .fetch_one(&mut *tx)
            .await?;
        if !taken {
            break;
        }
        username = format!("{base}-{n}");
    }
    let display = identity.name.as_deref().filter(|n| n.chars().count() <= 64);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, display_name, home, all_libraries, created_at)
         VALUES (?, ?, ?, 0, 0, ?) RETURNING id",
    )
    .bind(&username)
    .bind(NO_PASSWORD)
    .bind(display)
    .bind(now())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO user_identities (issuer, subject, user_id, email, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .bind(id)
        .bind(&identity.email)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenges_are_s256() {
        // RFC 7636, appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(random().len(), 43);
        assert_ne!(random(), random());
    }

    #[test]
    fn sealed_cookies_only_open_unchanged_and_for_their_purpose() {
        let sso = Sso::new();
        let sealed = sso.link_request(7);
        assert_eq!(sso.linking(&sealed), Some(7));
        assert!(sso.open::<LinkRequest>("pley sso flow", &sealed).is_none(), "another purpose");
        assert_eq!(Sso::new().linking(&sealed), None, "another server (or a restart)");
        let mut bytes = URL_SAFE_NO_PAD.decode(&sealed).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_eq!(sso.linking(&URL_SAFE_NO_PAD.encode(bytes)), None, "tampered with");
        assert_eq!(sso.linking("not-base64!"), None);
        assert_eq!(sso.linking(""), None);
        let stale = sso.seal("pley sso link", &LinkRequest { user: 7, expires: now() - 1 });
        assert_eq!(sso.linking(&stale), None, "expired");
    }

    #[test]
    fn flows_need_their_state_and_their_time() {
        let sso = Sso::new();
        let flow = Flow {
            state: "s".repeat(43),
            nonce: "n".into(),
            verifier: "v".into(),
            client_id: "pley".into(),
            redirect_uri: "https://pley.example.com/api/auth/sso/callback".into(),
            next: "/".into(),
            link: None,
            expires: now() + 60,
        };
        let sealed = sso.seal("pley sso flow", &flow);
        assert_eq!(sso.flow(Some(&sealed), &flow.state).unwrap(), flow);
        assert!(matches!(sso.flow(Some(&sealed), "another state"), Err(SsoError::Expired)));
        assert!(matches!(sso.flow(None, &flow.state), Err(SsoError::Expired)), "another browser");
        let old = sso.seal("pley sso flow", &Flow { expires: now(), ..flow.clone() });
        assert!(matches!(sso.flow(Some(&old), &flow.state), Err(SsoError::Expired)));
    }

    #[test]
    fn afterwards_only_pages_of_the_web_app() {
        assert_eq!(local_path(Some("/library/kx7bq2m?sort=year")), "/library/kx7bq2m?sort=year");
        assert_eq!(local_path(Some("/account")), "/account");
        for bad in ["https://evil.example", "//evil.example", "/\\evil.example", "/\tevil", "evil", "/api/auth/logout"]
        {
            assert_eq!(local_path(Some(bad)), "/", "{bad}");
        }
        assert_eq!(local_path(Some("/a/../../b")), "/b");
        assert_eq!(local_path(None), "/");
    }

    #[test]
    fn requests_tell_whether_they_came_to_the_public_address() {
        let headers = |host: &str| {
            let mut h = HeaderMap::new();
            h.insert(header::HOST, host.parse().unwrap());
            h
        };
        assert!(at_public_address(&headers("pley.example.com"), "https://pley.example.com"));
        assert!(at_public_address(&headers("PLEY.example.com"), "https://pley.example.com"));
        assert!(at_public_address(&headers("pley.lan:7539"), "http://pley.lan:7539"));
        assert!(!at_public_address(&headers("192.168.1.20:7539"), "https://pley.example.com"));
        assert!(!at_public_address(&headers("pley.example.com"), ""));
        assert!(!at_public_address(&HeaderMap::new(), "https://pley.example.com"));
    }

    #[test]
    fn cookies_are_read_by_name() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "pley_token=abc; pley_sso=xyz".parse().unwrap());
        assert_eq!(read_cookie(&headers, FLOW_COOKIE).as_deref(), Some("xyz"));
        assert_eq!(read_cookie(&headers, LINK_COOKIE), None);
        assert_eq!(
            cookie(FLOW_COOKIE, "v", 600, true),
            "pley_sso=v; Path=/api/auth/sso; HttpOnly; SameSite=Lax; Max-Age=600; Secure"
        );
    }

    #[test]
    fn new_users_get_a_username_from_what_the_provider_says() {
        let who = |username: Option<&str>, email: Option<&str>, name: Option<&str>| Identity {
            issuer: "https://auth".into(),
            subject: "1".into(),
            email: email.map(Into::into),
            username: username.map(Into::into),
            name: name.map(Into::into),
            groups: Vec::new(),
        };
        assert_eq!(username_for(&who(Some("ann"), Some("a@x.org"), None)), "ann");
        assert_eq!(username_for(&who(None, Some("ann@example.org"), None)), "ann@example.org");
        assert_eq!(username_for(&who(None, None, Some("Ann Smith"))), "Ann-Smith");
        assert_eq!(username_for(&who(Some("../../etc"), None, None)), "etc");
        assert_eq!(username_for(&who(None, None, None)), "user");
    }

    #[test]
    fn pley_asks_only_for_what_the_settings_need() {
        let mut settings = Settings::default();
        assert_eq!(scopes(&settings), "openid email");
        settings.sso_new_accounts = true;
        assert_eq!(scopes(&settings), "openid email profile");
        settings.sso_group = "pley-users".into();
        assert_eq!(scopes(&settings), "openid email profile groups");
        settings.sso_new_accounts = false;
        assert_eq!(scopes(&settings), "openid email groups");
    }

    #[test]
    fn claims_fill_in_what_is_missing_only() {
        let mut identity = Identity { email: Some("ann@example.org".into()), ..Default::default() };
        let claims: PersonClaims = serde_json::from_value(serde_json::json!({
            "email": "other@example.org", "preferred_username": " ann ", "groups": "pley-users"
        }))
        .unwrap();
        claims.fill(&mut identity);
        assert_eq!(identity.email.as_deref(), Some("ann@example.org"));
        assert_eq!(identity.username.as_deref(), Some("ann"));
        assert_eq!(identity.groups, ["pley-users"]);
        let unverified: PersonClaims =
            serde_json::from_value(serde_json::json!({"email": "x@example.org", "email_verified": false})).unwrap();
        let mut fresh = Identity::default();
        unverified.fill(&mut fresh);
        assert_eq!(fresh.email, None, "unverified addresses aren't shown as the linked account");
    }

    #[test]
    fn client_credentials_are_form_encoded_for_basic_auth() {
        assert_eq!(form_encode("a b:c/d"), "a+b%3Ac%2Fd");
    }

    #[test]
    fn the_metadata_document_names_itself_and_its_redirect() {
        let settings =
            Settings { server_name: "Den".into(), public_url: "https://pley.example.com".into(), ..Default::default() };
        let doc = Sso::client_metadata(&settings);
        assert_eq!(doc["client_id"], "https://pley.example.com/api/auth/sso/client");
        assert_eq!(doc["redirect_uris"][0], "https://pley.example.com/api/auth/sso/callback");
        assert_eq!(doc["token_endpoint_auth_method"], "none");
        assert_eq!(Sso::client_id(&settings), doc["client_id"]);
        let registered = Settings { sso_client_id: "pley".into(), ..settings };
        assert_eq!(Sso::client_id(&registered), "pley");
    }
}
