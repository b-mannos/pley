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
//! Nothing in this module is reachable over HTTP by itself.

pub mod clients;
pub mod fetch;
