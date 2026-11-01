//! Who this server is, so an app can tell it from another device at an
//! address it saved: its id (the one discovery answers), the addresses it
//! can be reached at, and proofs that it holds the key an app was given.
//!
//! Each sign-in of an app (a grant) gets its own proof key in the token
//! answers, derived from the server's `secret.key` and the grant's id, so
//! nothing more is stored and no app can pass for the server to another.
//! Before sending its token to an address, an app sends a challenge of its
//! own, and the server answers with an HMAC of it under that key. Every app
//! can check an HMAC-SHA256 (Roku's BrightScript included), which a
//! signature wouldn't allow.
//!
//! Pley's own sign-in has no grant: the desktop app asks for a key with the
//! window's session ([`Identity::session_key`]). Its id is random, and the
//! key derives from it, so nothing is stored for it either.

use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use base64::Engine;
use ring::hmac;

use crate::network::{Kind, Listener};

/// The challenge an app sends: 16 to 64 random bytes, base64url without
/// padding.
const CHALLENGE_BYTES: std::ops::RangeInclusive<usize> = 16..=64;
/// Before the server's id and the challenge in what a proof covers, so the
/// key signs nothing else.
const PROOF_CONTEXT: &str = "pley-server-proof";

pub struct Identity {
    /// The 40 hexadecimal characters discovery and the Plex-compatible API
    /// answer with.
    pub id: String,
    /// Derives the proof keys; `None` while `secret.key` can't be used.
    root: Option<hmac::Key>,
}

/// How many random bytes name a session's key.
const SESSION_KEY_ID_BYTES: usize = 16;

/// What an app keeps of a sign-in to tell its server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofKey {
    /// The grant's id, or `s` and a session key's random id: what the app
    /// sends with its challenge.
    pub id: String,
    /// The key, base64url without padding.
    pub key: String,
}

impl Identity {
    pub fn new(data_dir: &Path) -> Self {
        let id = crate::plex::PlexInfo::load(data_dir).machine_id;
        // TwoStep::new says in the log why the key can't be used.
        let secret = crate::two_step::load_key(&data_dir.join(crate::two_step::KEY_FILE)).ok();
        Self::with_secret(id, secret.as_ref().map(|s| s.as_slice()))
    }

    fn with_secret(id: String, secret: Option<&[u8]>) -> Self {
        let root = secret.map(|secret| {
            let derived = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, secret), b"pley server proofs");
            hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref())
        });
        Self { id, root }
    }

    /// The proof key of the sign-in `grant_id`, the same at every refresh.
    pub fn proof_key(&self, grant_id: i64) -> Option<ProofKey> {
        let key = self.grant_key(grant_id)?;
        Some(ProofKey { id: grant_id.to_string(), key: b64(&key) })
    }

    /// A new proof key for a session of pley's own sign-in, under a random id.
    pub fn session_key(&self) -> Option<ProofKey> {
        let mut bytes = [0u8; SESSION_KEY_ID_BYTES];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).ok()?;
        let id = b64(&bytes);
        let key = self.derive(&format!("session {id}"))?;
        Some(ProofKey { id: format!("s{id}"), key: b64(&key) })
    }

    /// The proof of `challenge` for the sign-in `proof_id`: `Ok(None)` when
    /// the server can't make one (its key can't be used), `Err` when the
    /// request is wrong.
    pub fn prove(&self, proof_id: &str, challenge: &str) -> Result<Option<String>, &'static str> {
        let derived_from = match proof_id.strip_prefix('s') {
            Some(id) => {
                let canonical = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(id)
                    .is_ok_and(|b| b.len() == SESSION_KEY_ID_BYTES && b64(&b) == id);
                if !canonical {
                    return Err("proof_id isn't a sign-in's id");
                }
                format!("session {id}")
            }
            None => {
                let grant_id = proof_id
                    .parse::<i64>()
                    .ok()
                    .filter(|&id| id > 0 && id.to_string() == proof_id)
                    .ok_or("proof_id isn't a sign-in's id")?;
                format!("grant {grant_id}")
            }
        };
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(challenge)
            .ok()
            .filter(|b| CHALLENGE_BYTES.contains(&b.len()))
            .ok_or("challenge must be 16 to 64 bytes in base64url without padding")?;
        // A challenge decodes from one spelling only, so the proof covers
        // what the app sent.
        if b64(&bytes) != challenge {
            return Err("challenge must be 16 to 64 bytes in base64url without padding");
        }
        let Some(key) = self.derive(&derived_from) else { return Ok(None) };
        Ok(Some(proof(&key, &self.id, challenge)))
    }

    fn grant_key(&self, grant_id: i64) -> Option<[u8; 32]> {
        self.derive(&format!("grant {grant_id}"))
    }

    fn derive(&self, what: &str) -> Option<[u8; 32]> {
        let root = self.root.as_ref()?;
        hmac::sign(root, what.as_bytes()).as_ref().try_into().ok()
    }
}

/// HMAC-SHA256 under `key` of `pley-server-proof`, the server's id and the
/// challenge, each on a line of its own, in base64url without padding.
pub fn proof(key: &[u8], server_id: &str, challenge: &str) -> String {
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, key),
        format!("{PROOF_CONTEXT}\n{server_id}\n{challenge}").as_bytes(),
    );
    b64(tag.as_ref())
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// The addresses of this server's web app on its networks: each web
/// listener that listens, at each address of a connected adapter for one on
/// every address. Loopback and link-local addresses, which no other device
/// keeps, are left out, and so are IPv6 ones, which adapters don't list.
pub fn network_addresses(listeners: &[Listener], adapters: &[crate::network::Adapter]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |addr: SocketAddr| {
        let url = format!("http://{addr}");
        if !out.contains(&url) {
            out.push(url);
        }
    };
    for listener in listeners.iter().filter(|l| l.kind == Kind::Web && l.error.is_none()) {
        let at = listener.address;
        match at.ip() {
            IpAddr::V4(ip) if ip.is_unspecified() => {
                for adapter in adapters.iter().filter(|a| a.up) {
                    for &ip in adapter.addresses.iter().filter(|ip| !ip.is_loopback() && !ip.is_link_local()) {
                        add(SocketAddr::new(IpAddr::V4(ip), at.port()));
                    }
                }
            }
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => add(at),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::network::Adapter;

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";
    const CHALLENGE: &str = "AAECAwQFBgcICQoLDA0ODw";

    fn identity() -> Identity {
        Identity::with_secret(ID.into(), Some(&[7; 32]))
    }

    #[test]
    fn a_proof_is_the_hmac_an_app_computes_with_its_key() {
        let identity = identity();
        let given = identity.proof_key(17).unwrap();
        assert_eq!(given.id, "17");
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&given.key).unwrap();
        assert_eq!(key.len(), 32);
        let answered = identity.prove("17", CHALLENGE).unwrap().unwrap();
        assert_eq!(answered, proof(&key, ID, CHALLENGE));
        assert_eq!(answered.len(), 43, "32 bytes in base64url");
    }

    #[test]
    fn each_sign_in_has_its_own_key_which_stays_with_the_secret() {
        let identity = identity();
        assert_eq!(identity.proof_key(17), identity.proof_key(17), "the same at every refresh");
        assert_ne!(identity.proof_key(17).unwrap().key, identity.proof_key(18).unwrap().key);
        assert_ne!(identity.prove("17", CHALLENGE), identity.prove("18", CHALLENGE));
        let another = Identity::with_secret(ID.into(), Some(&[8; 32]));
        assert_ne!(identity.proof_key(17), another.proof_key(17), "another server's secret, another key");
    }

    #[test]
    fn a_proof_covers_the_server_and_the_challenge() {
        let key = [1u8; 32];
        let challenge2 = "AAECAwQFBgcICQoLDA0OEA";
        assert_ne!(proof(&key, ID, CHALLENGE), proof(&key, ID, challenge2));
        assert_ne!(proof(&key, ID, CHALLENGE), proof(&key, "another", CHALLENGE));
        // A known answer, for the apps' tests.
        assert_eq!(proof(&key, ID, CHALLENGE), "-OpCw2gYEDYBr-d7wh4JEYd7CHzZ8nc7pTc_UlsqVas");
    }

    #[test]
    fn a_session_key_has_a_random_id_its_proofs_go_by() {
        let identity = identity();
        let given = identity.session_key().unwrap();
        assert!(given.id.starts_with('s') && given.id.len() == 23, "{}", given.id);
        assert_ne!(identity.session_key().unwrap(), given, "a new one each time");
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&given.key).unwrap();
        assert_eq!(identity.prove(&given.id, CHALLENGE).unwrap().unwrap(), proof(&key, ID, CHALLENGE));
        assert_ne!(identity.prove(&given.id, CHALLENGE), identity.prove("17", CHALLENGE));
        assert_eq!(Identity::with_secret(ID.into(), None).session_key(), None);
    }

    #[test]
    fn requests_are_checked() {
        let identity = identity();
        for (proof_id, challenge) in [
            ("0", CHALLENGE),
            ("-3", CHALLENGE),
            ("017", CHALLENGE),
            ("x", CHALLENGE),
            ("s", CHALLENGE),
            ("sAAECAwQFBgcICQoLDA0ODw=", CHALLENGE),
            ("sAAECAwQFBgcICQoLDA0O", CHALLENGE),
            ("sAAECAwQFBgcICQoLDA0ODx", CHALLENGE),
            ("17", "short"),
            ("17", "AAECAwQFBgcICQoLDA0ODw=="),
            ("17", "AAECAwQFBgcICQoLDA0ODw+"),
            ("17", &"A".repeat(87)),
        ] {
            assert!(identity.prove(proof_id, challenge).is_err(), "{proof_id} {challenge}");
        }
        assert!(identity.prove("17", &"A".repeat(86)).unwrap().is_some(), "64 bytes");
        assert!(identity.prove("sAAECAwQFBgcICQoLDA0ODw", CHALLENGE).unwrap().is_some());
    }

    #[test]
    fn without_its_key_the_server_proves_nothing() {
        let identity = Identity::with_secret(ID.into(), None);
        assert_eq!(identity.proof_key(17), None);
        assert_eq!(identity.prove("17", CHALLENGE), Ok(None));
        assert!(identity.prove("x", CHALLENGE).is_err(), "a wrong request is still wrong");
    }

    #[test]
    fn network_addresses_come_from_the_listeners_that_listen() {
        let adapter = |up, addresses: &[[u8; 4]]| Adapter {
            name: "eth0".into(),
            description: None,
            up,
            addresses: addresses.iter().map(|&a| Ipv4Addr::from(a)).collect(),
            network: None,
        };
        let adapters = [
            adapter(true, &[[192, 168, 1, 20], [169, 254, 3, 3]]),
            adapter(false, &[[10, 0, 0, 5]]),
            adapter(true, &[[172, 16, 4, 1]]),
        ];
        let listener = |kind, address: &str, error: Option<&str>| Listener {
            kind,
            address: address.parse().unwrap(),
            error: error.map(str::to_owned),
        };
        let listeners = [
            listener(Kind::Web, "0.0.0.0:7539", None),
            listener(Kind::Web, "127.0.0.1:8080", None),
            listener(Kind::Web, "192.168.1.20:7539", None),
            listener(Kind::Web, "192.168.1.30:7540", Some("in use")),
            listener(Kind::Plex, "0.0.0.0:32400", None),
            listener(Kind::Web, "[::]:7541", None),
        ];
        assert_eq!(network_addresses(&listeners, &adapters), ["http://192.168.1.20:7539", "http://172.16.4.1:7539"],);
    }
}
