//! Making sure the server at an address is the one the window signed in
//! to before its session goes there (docs/API.md, "Telling servers
//! apart"): an address can lead to another device later, on another network
//! or after the router hands it out again. The window's session gets a key
//! from its server (`/api/server/proof-key`), kept by address in
//! `proofs.json`; before the app opens a server, and before its own
//! requests with the session (notices, downloads), the server at that
//! address proves it holds the key. The pages the window shows can't be
//! held back this way, but they open only after a proof, and the notices
//! take the window back to the picker once one fails.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use ring::hmac;
use ring::rand::SecureRandom;
use serde::{Deserialize, Serialize};
use url::Url;

/// What the app says of a server that can't prove itself.
pub const NOT_YOURS: &str =
    "another device answers there (if your server was set up anew, forget it and connect again)";
/// A proof counts this long; a dropped connection asks for a new one sooner.
const PROVED_FOR: Duration = Duration::from_secs(30 * 60);
/// How long a server gets to answer.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
/// Before the server's id and the challenge in what a proof covers.
const PROOF_CONTEXT: &str = "pley-server-proof";

/// A key the server at an address gave the window's session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub server_id: String,
    pub proof_id: String,
    /// base64url without padding, 32 bytes.
    pub proof_key: String,
}

impl Key {
    fn bytes(&self) -> Option<Vec<u8>> {
        b64().decode(&self.proof_key).ok().filter(|k| k.len() == 32)
    }
}

/// Why the app keeps the session to itself.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// Nothing answered, or not as a Pley server does.
    Unreachable(String),
    /// Something answered without the proof.
    NotYours,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(why) => f.write_str(why),
            Self::NotYours => f.write_str(NOT_YOURS),
        }
    }
}

#[derive(Default)]
struct Kept {
    keys: HashMap<String, Key>,
    /// When the server at each address last proved itself.
    proved: HashMap<String, Instant>,
}

/// The keys, by address (an origin, as `http://nas:7539`).
pub struct Proofs {
    path: Option<PathBuf>,
    kept: Mutex<Kept>,
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

fn address(server: &Url) -> String {
    server.origin().ascii_serialization()
}

impl Proofs {
    /// The keys kept in `path` (none when it can't be read).
    pub fn load(path: PathBuf) -> Self {
        let keys = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { path: Some(path), kept: Mutex::new(Kept { keys, proved: HashMap::new() }) }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self { path: None, kept: Mutex::default() }
    }

    /// Keys for `server`, which it proved it holds a moment ago (or not).
    #[cfg(test)]
    pub fn for_tests(server: &Url, proved: bool) -> Self {
        let proofs = Self::in_memory();
        proofs.keep(server, tests::key());
        if proved {
            proofs.proved(server);
        }
        proofs
    }

    fn key(&self, server: &Url) -> Option<Key> {
        self.kept.lock().unwrap().keys.get(&address(server)).cloned()
    }

    fn keep(&self, server: &Url, key: Key) {
        let mut kept = self.kept.lock().unwrap();
        kept.keys.insert(address(server), key);
        self.save(&kept.keys);
    }

    /// Forgets the key for `server` (the server was forgotten).
    pub fn forget(&self, server: &Url) {
        let mut kept = self.kept.lock().unwrap();
        let address = address(server);
        kept.proved.remove(&address);
        if kept.keys.remove(&address).is_some() {
            self.save(&kept.keys);
        }
    }

    /// The next use of `server` asks for a proof again: a connection there
    /// dropped, and the computer may be on another network now.
    pub fn doubt(&self, server: &Url) {
        self.kept.lock().unwrap().proved.remove(&address(server));
    }

    fn proved(&self, server: &Url) {
        self.kept.lock().unwrap().proved.insert(address(server), Instant::now());
    }

    fn proved_lately(&self, server: &Url) -> bool {
        self.kept.lock().unwrap().proved.get(&address(server)).is_some_and(|at| at.elapsed() < PROVED_FOR)
    }

    fn save(&self, keys: &HashMap<String, Key>) {
        let Some(path) = &self.path else { return };
        let written = (|| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let partial = path.with_extension("json.part");
            std::fs::write(&partial, serde_json::to_vec_pretty(keys)?)?;
            std::fs::rename(&partial, path)
        })();
        if let Err(err) = written {
            eprintln!("cannot keep the servers' keys: {err}");
        }
    }
}

/// Whether `answered` proves `challenge` for `key`.
fn proves(key: &Key, challenge: &str, server_id: Option<&str>, answered: Option<&str>) -> bool {
    let (Some(bytes), Some(answered)) = (key.bytes(), answered.and_then(|a| b64().decode(a).ok())) else {
        return false;
    };
    server_id == Some(key.server_id.as_str())
        && hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, &bytes),
            format!("{PROOF_CONTEXT}\n{}\n{challenge}", key.server_id).as_bytes(),
            &answered,
        )
        .is_ok()
}

fn challenge() -> Option<String> {
    let mut bytes = [0u8; 16];
    ring::rand::SystemRandom::new().fill(&mut bytes).ok()?;
    Some(b64().encode(bytes))
}

/// What `/api/server` says, as far as proofs go.
#[derive(Deserialize)]
struct Answer {
    id: Option<String>,
    proof: Option<String>,
}

/// `/api/server` of `server`, asked to prove it holds `key` when there is
/// one, as JSON: what the app checks a server with before it opens it.
pub async fn ask(client: &reqwest::Client, server: &Url, proofs: &Proofs) -> Result<serde_json::Value, Refused> {
    let key = proofs.key(server);
    let mut endpoint = server.join("/api/server").map_err(|e| Refused::Unreachable(e.to_string()))?;
    let challenge = match &key {
        Some(key) => {
            let challenge = challenge().ok_or_else(|| Refused::Unreachable("no random numbers".into()))?;
            endpoint.query_pairs_mut().append_pair("challenge", &challenge).append_pair("proof_id", &key.proof_id);
            Some(challenge)
        }
        None => None,
    };
    let response = client.get(endpoint).timeout(ANSWER_TIMEOUT).send().await.map_err(|err| {
        let why = if err.is_timeout() {
            format!("no answer within {} seconds", ANSWER_TIMEOUT.as_secs())
        } else if err.is_connect() {
            "nothing answers there".to_owned()
        } else {
            err.without_url().to_string()
        };
        Refused::Unreachable(why)
    })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        // A server too old for proofs refuses the challenge: it is another
        // device all the same, as the key came from a server that knew them.
        return Err(match key {
            Some(_) => Refused::NotYours,
            None => Refused::Unreachable(format!("it answered HTTP {status}, so it isn't a Pley server")),
        });
    }
    let body: serde_json::Value = response.json().await.map_err(|_| match key {
        Some(_) => Refused::NotYours,
        None => Refused::Unreachable("that isn't a Pley server".into()),
    })?;
    if let (Some(key), Some(challenge)) = (key, challenge) {
        let answer: Answer = serde_json::from_value(body.clone()).unwrap_or(Answer { id: None, proof: None });
        if !proves(&key, &challenge, answer.id.as_deref(), answer.proof.as_deref()) {
            return Err(Refused::NotYours);
        }
        proofs.proved(server);
    }
    Ok(body)
}

/// Gets a key for `session` from `server` when the app has none for it.
pub async fn ask_for_key(client: &reqwest::Client, server: &Url, proofs: &Proofs, session: &str) {
    if proofs.key(server).is_some() {
        return;
    }
    let Ok(url) = server.join("/api/server/proof-key") else { return };
    let request = client.get(url).header(reqwest::header::COOKIE, session).timeout(ANSWER_TIMEOUT);
    let Ok(response) = request.send().await else { return };
    if !response.status().is_success() {
        return;
    }
    if let Ok(key) = response.json::<Key>().await
        && key.bytes().is_some()
    {
        proofs.keep(server, key);
        // It came from the server the window has been using: proved as much as that.
        proofs.proved(server);
    }
}

/// `session` (the window's cookies for `server`), once the server proved
/// itself: within the last half hour, or now. Without a key yet, it asks
/// for one.
pub async fn session(
    client: &reqwest::Client,
    server: &Url,
    proofs: &Proofs,
    session: Option<String>,
) -> Result<Option<String>, Refused> {
    let Some(session) = session else { return Ok(None) };
    if proofs.key(server).is_none() {
        ask_for_key(client, server, proofs, &session).await;
    } else if !proofs.proved_lately(server) {
        ask(client, server, proofs).await?;
    }
    Ok(Some(session))
}

#[cfg(test)]
pub(crate) mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    /// The proof of `challenge` under `key` from server `server_id`.
    fn proof(key: &[u8], server_id: &str, challenge: &str) -> hmac::Tag {
        hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, key),
            format!("{PROOF_CONTEXT}\n{server_id}\n{challenge}").as_bytes(),
        )
    }

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";
    const CHALLENGE: &str = "AAECAwQFBgcICQoLDA0ODw";

    pub fn key() -> Key {
        let bytes = [1u8; 32];
        Key { server_id: ID.into(), proof_id: "sAAECAwQFBgcICQoLDA0ODw".into(), proof_key: b64().encode(bytes) }
    }

    #[test]
    fn a_proof_is_the_servers_hmac() {
        // The server's known answer (apps/server/src/identity.rs).
        let known = "-OpCw2gYEDYBr-d7wh4JEYd7CHzZ8nc7pTc_UlsqVas";
        assert_eq!(b64().encode(proof(&key().bytes().unwrap(), ID, CHALLENGE).as_ref()), known);
        assert!(proves(&key(), CHALLENGE, Some(ID), Some(known)));
        assert!(!proves(&key(), "AAECAwQFBgcICQoLDA0OEA", Some(ID), Some(known)), "another challenge");
        assert!(!proves(&key(), CHALLENGE, Some("another"), Some(known)), "another server");
        assert!(!proves(&key(), CHALLENGE, Some(ID), None));
        assert!(!proves(&key(), CHALLENGE, None, Some(known)));
        let short = Key { proof_key: b64().encode([1u8; 16]), ..key() };
        assert!(!proves(&short, CHALLENGE, Some(ID), Some(known)));
    }

    /// A server that answers each connection with the next of `answers`
    /// (status and JSON body), and tells what each request asked.
    async fn serve(answers: Vec<(u16, String)>) -> (Url, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for (status, body) in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                let _ = tx.send(String::from_utf8_lossy(&request).into_owned());
                let answer = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(answer.as_bytes()).await.unwrap();
            }
        });
        (url, rx)
    }

    /// The challenge in a request for `/api/server`.
    fn challenge_in(request: &str) -> String {
        let path = request.split_whitespace().nth(1).unwrap();
        let url = Url::parse(&format!("http://x{path}")).unwrap();
        url.query_pairs().find(|(k, _)| k == "challenge").map(|(_, v)| v.into_owned()).unwrap()
    }

    #[tokio::test]
    async fn the_session_goes_only_where_the_server_proves_itself() {
        let client = reqwest::Client::new();
        let proofs = Proofs::in_memory();
        // No key yet: the session asks for one.
        let given = serde_json::json!({"server_id": ID, "proof_id": key().proof_id, "proof_key": key().proof_key});
        let (server, mut asked) = serve(vec![(200, given.to_string())]).await;
        let cookie = Some("pley_token=abc".to_owned());
        assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Ok(cookie.clone()));
        let request = asked.recv().await.unwrap();
        assert!(request.starts_with("GET /api/server/proof-key "), "{request}");
        assert!(request.contains("cookie: pley_token=abc"), "{request}");
        assert_eq!(proofs.key(&server), Some(key()));
        // Proved lately: nothing to ask.
        assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Ok(cookie.clone()));
        assert!(asked.try_recv().is_err());
    }

    #[tokio::test]
    async fn another_device_at_the_address_gets_nothing() {
        let client = reqwest::Client::new();
        let cookie = Some("pley_token=abc".to_owned());
        let wrong =
            serde_json::json!({"id": ID, "name": "Den", "proof": "-OpCw2gYEDYBr-d7wh4JEYd7CHzZ8nc7pTc_UlsqVas"});
        let another = serde_json::json!({"id": "another", "name": "Den", "version": "0.1.2"});
        for answer in
            [(200, wrong.to_string()), (200, another.to_string()), (400, "{}".to_owned()), (200, "<html>".to_owned())]
        {
            let (server, mut asked) = serve(vec![answer.clone()]).await;
            let proofs = Proofs::in_memory();
            proofs.keep(&server, key());
            assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Err(Refused::NotYours), "{answer:?}");
            let request = asked.recv().await.unwrap();
            assert!(request.contains("proof_id=sAAECAwQFBgcICQoLDA0ODw"), "{request}");
            assert!(!request.contains("pley_token"), "the session stays home: {request}");
        }
    }

    #[tokio::test]
    async fn the_server_proves_itself_again_after_a_doubt() {
        let client = reqwest::Client::new();
        let cookie = Some("pley_token=abc".to_owned());
        // The answer depends on the challenge, so this server answers
        // after reading it.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let bytes = key().bytes().unwrap();
        let answering = tokio::spawn(async move {
            let mut asked = 0;
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                let request = String::from_utf8_lossy(&request).into_owned();
                assert!(!request.contains("pley_token"), "{request}");
                let challenge = challenge_in(&request);
                let proved = b64().encode(proof(&bytes, ID, &challenge).as_ref());
                let body = serde_json::json!({"id": ID, "name": "Den", "proof": proved}).to_string();
                let answer = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(answer.as_bytes()).await.unwrap();
                asked += 1;
            }
            asked
        });
        let proofs = Proofs::in_memory();
        proofs.keep(&server, key());
        assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Ok(cookie.clone()));
        assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Ok(cookie.clone()), "proved lately");
        proofs.doubt(&server);
        assert_eq!(session(&client, &server, &proofs, cookie.clone()).await, Ok(cookie.clone()));
        assert_eq!(answering.await.unwrap(), 2, "proved at first and after the doubt");
        // Without a session, nothing to prove.
        assert_eq!(session(&client, &server, &proofs, None).await, Ok(None));
    }

    #[test]
    fn keys_are_kept_by_address_and_forgotten_with_the_server() {
        let dir = std::env::temp_dir().join(format!("pley-proofs-{}", std::process::id()));
        let path = dir.join("proofs.json");
        let server = Url::parse("http://nas:7539/library").unwrap();
        let proofs = Proofs::load(path.clone());
        proofs.keep(&server, key());
        assert_eq!(Proofs::load(path.clone()).key(&Url::parse("http://nas:7539").unwrap()), Some(key()));
        assert_eq!(Proofs::load(path.clone()).key(&Url::parse("http://nas:7540").unwrap()), None);
        proofs.forget(&server);
        assert_eq!(Proofs::load(path).key(&server), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
