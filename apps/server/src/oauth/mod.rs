//! Apps that use pley: pley is an OAuth 2.1 authorization server for them.
//! An app sends a person to pley's own sign-in page, the person allows it,
//! and the app gets tokens that open only what was allowed.
//!
//! pley is no OpenID Connect provider: it lets apps use pley, and signs
//! nobody in to other sites.
//!
//! Apps are known by their client ID, the https address of a JSON document
//! describing them (OAuth Client ID Metadata Document,
//! draft-ietf-oauth-client-id-metadata-document-02, "CIMD" below):
//! * pley's own apps are built in ([`clients::builtin`]) and never fetched,
//!   so they sign in to servers without internet access;
//! * apps by other publishers are read from their documents
//!   ([`fetch::Documents`]), only when an administrator allows them, and
//!   carefully: a document is believed no further than the rules in
//!   [`clients`] allow.
//!
//! A sign-in in progress lives here too: the request the consent page shows
//! ([`pending`]), the code an allowed app exchanges for its tokens
//! ([`codes`]), and the request of a TV that signs in with a code it shows
//! ([`devices`]). The HTTP side is `api/oauth.rs`.

pub mod clients;
pub mod codes;
pub mod devices;
pub mod fetch;
pub mod pending;

use crate::config::Config;

/// What the authorization server keeps between the steps of a sign-in, in
/// memory: one process, so a restart only cancels sign-ins in progress.
pub struct AuthorizationServer {
    /// The apps that may ask (see [`fetch::Documents`]).
    pub documents: fetch::Documents,
    /// Requests shown on the consent page, until they are answered.
    pub pending: pending::Pending,
    /// Codes handed to apps that were allowed, until they are exchanged.
    pub codes: codes::Codes,
    /// Requests of TVs signing in with a code, until they have their answer.
    pub devices: devices::Devices,
}

impl AuthorizationServer {
    pub fn new(config: &Config) -> Self {
        Self {
            documents: fetch::Documents::new(config.client_documents.clone()),
            pending: pending::Pending::default(),
            codes: codes::Codes::default(),
            devices: devices::Devices::default(),
        }
    }

    /// Forgets what is over: requests, codes, documents and counters.
    pub fn sweep(&self) {
        self.pending.sweep();
        self.codes.sweep();
        self.documents.sweep();
        self.devices.sweep();
    }
}

/// What a sign-in remembers of the app it is for, as it was when the
/// person was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSnapshot {
    pub id: String,
    pub host: String,
    /// What the app calls itself (pley vouches for it only for its own apps).
    pub name: Option<String>,
    pub first_party: bool,
    /// The app gets refresh tokens.
    pub refresh: bool,
}

impl From<&clients::Client> for AppSnapshot {
    fn from(client: &clients::Client) -> Self {
        Self {
            id: client.id.clone(),
            host: client.host.clone(),
            name: client.name.clone(),
            first_party: client.first_party,
            refresh: client.refresh,
        }
    }
}

impl AppSnapshot {
    /// How the app is called in logs: pley's own apps by name, others by
    /// host, which is all pley can vouch for.
    pub fn label(&self) -> &str {
        match (&self.name, self.first_party) {
            (Some(name), true) => name,
            _ => &self.host,
        }
    }
}
