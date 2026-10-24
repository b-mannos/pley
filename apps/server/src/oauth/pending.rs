//! Requests waiting on the consent page: what an app asked for, checked,
//! until the person allows it or not, and then where the browser goes back
//! to. In memory only.
//!
//! A request belongs to the person and the browser session that opened the
//! page (the SHA-256 of its session token), and to the address pley was
//! reached at (the issuer, sent back as `iss`, RFC 9207). It lasts 10
//! minutes; each person has at most 10 (the oldest goes) and the server
//! 1,000. Once answered, the way back repeats for a minute: while the code
//! works and wasn't exchanged when allowed, and for 60 seconds after a
//! refusal or an error.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use url::Url;

use super::AppSnapshot;
use crate::auth::new_token;
use crate::auth::scopes::Scopes;

/// How long a request waits for an answer.
pub const PENDING_LIFETIME: Duration = Duration::from_secs(600);
/// How long the way back repeats after a refusal or an error (and how long a
/// code works).
pub const ANSWER_LIFETIME: Duration = Duration::from_secs(60);
/// Requests per person; past it the oldest goes.
pub const MAX_PENDING_PER_USER: usize = 10;
/// Requests in all.
pub const MAX_PENDING: usize = 1_000;

/// A mistake in an app's request that goes back to it, but only when the
/// person clicks "Back to the app" (OAuth 2.1 -16 §4.1.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedError {
    /// The OAuth error code: `invalid_request`, `invalid_scope`, …
    pub error: &'static str,
    /// What went wrong, for the person and the app (ASCII).
    pub description: String,
}

/// A request an app made, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRequest {
    pub user_id: i64,
    /// SHA-256 of the session token of the browser that opened the page.
    pub session_hash: String,
    /// The address pley was reached at.
    pub issuer: String,
    pub app: AppSnapshot,
    /// As the app sent it: answers go there, port included.
    pub redirect_uri: String,
    pub state: Option<String>,
    /// Empty when `error` says it was missing or wrong.
    pub code_challenge: String,
    pub scopes: Scopes,
    /// Set when the request can't be allowed: it only goes back.
    pub error: Option<ReturnedError>,
}

/// Too many requests are waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

/// Why a decision wasn't taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecideError<E> {
    /// Not this person's or this session's, or over.
    NotFound,
    /// Decided the other way already, or a request that can only go back.
    Conflict,
    /// The code couldn't be made.
    Code(E),
}

/// The way back to the app once a request is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continue {
    /// The app's return address with the answer.
    pub location: String,
    /// The code in it, when the app was allowed: the way back repeats only
    /// while it works and wasn't exchanged.
    pub code: Option<String>,
}

struct Entry {
    request: PendingRequest,
    created: Instant,
    /// When it is forgotten.
    until: Instant,
    decision: Option<bool>,
    answer: Option<Continue>,
}

impl Entry {
    fn alive(&self, now: Instant) -> bool {
        now < self.until
    }

    fn belongs_to(&self, user_id: i64, session_hash: &str) -> bool {
        self.request.user_id == user_id && self.request.session_hash == session_hash
    }
}

/// The requests waiting, by id.
#[derive(Default)]
pub struct Pending {
    requests: Mutex<HashMap<String, Entry>>,
}

/// `redirect_uri` with `params`, then `state` (when the app sent one) and
/// `iss` added to its query (its own query kept).
pub fn answer_url(request: &PendingRequest, params: &[(&str, &str)]) -> String {
    let Ok(mut url) = Url::parse(&request.redirect_uri) else { return request.redirect_uri.clone() };
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params {
            query.append_pair(name, value);
        }
        if let Some(state) = &request.state {
            query.append_pair("state", state);
        }
        query.append_pair("iss", &request.issuer);
    }
    url.into()
}

impl Pending {
    /// Keeps a request; its id. The person's oldest goes past 10.
    pub fn create(&self, request: PendingRequest) -> Result<String, Full> {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        requests.retain(|_, entry| entry.alive(now));
        loop {
            let mine = requests.iter().filter(|(_, entry)| entry.request.user_id == request.user_id);
            let (count, oldest) = mine.fold((0, None::<(&String, Instant)>), |(count, oldest), (id, entry)| {
                let older = oldest.is_none_or(|(_, created)| entry.created < created);
                (count + 1, if older { Some((id, entry.created)) } else { oldest })
            });
            match oldest {
                Some((id, _)) if count >= MAX_PENDING_PER_USER => {
                    let id = id.clone();
                    requests.remove(&id);
                }
                _ => break,
            }
        }
        if requests.len() >= MAX_PENDING {
            return Err(Full);
        }
        let id = new_token();
        let entry = Entry { request, created: now, until: now + PENDING_LIFETIME, decision: None, answer: None };
        requests.insert(id.clone(), entry);
        Ok(id)
    }

    /// The request `id`, when it is this person's and this session's and
    /// isn't over.
    pub fn get(&self, id: &str, user_id: i64, session_hash: &str) -> Option<PendingRequest> {
        let now = Instant::now();
        let requests = self.requests.lock();
        let entry = requests.get(id).filter(|entry| entry.alive(now) && entry.belongs_to(user_id, session_hash))?;
        Some(entry.request.clone())
    }

    /// Allows the app (with the code `make_code` makes for the request) or
    /// not. The same decision again changes nothing; the other one is a
    /// conflict.
    pub fn decide<E>(
        &self,
        id: &str,
        user_id: i64,
        session_hash: &str,
        allow: bool,
        make_code: impl FnOnce(&PendingRequest) -> Result<String, E>,
    ) -> Result<(), DecideError<E>> {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        let Some(entry) =
            requests.get_mut(id).filter(|entry| entry.alive(now) && entry.belongs_to(user_id, session_hash))
        else {
            return Err(DecideError::NotFound);
        };
        if entry.request.error.is_some() {
            return Err(DecideError::Conflict);
        }
        match entry.decision {
            Some(decided) if decided == allow => return Ok(()),
            Some(_) => return Err(DecideError::Conflict),
            None => {}
        }
        let answer = if allow {
            let code = make_code(&entry.request).map_err(DecideError::Code)?;
            Continue { location: answer_url(&entry.request, &[("code", &code)]), code: Some(code) }
        } else {
            Continue { location: answer_url(&entry.request, &[("error", "access_denied")]), code: None }
        };
        entry.decision = Some(allow);
        entry.answer = Some(answer);
        entry.until = now + ANSWER_LIFETIME;
        Ok(())
    }

    /// Where the browser goes back to, once the request is answered (or
    /// can only go back): for a minute after the answer, or after the first
    /// time back for a request with a mistake (the person may read the page
    /// for a while before clicking).
    pub fn location(&self, id: &str, user_id: i64, session_hash: &str) -> Option<Continue> {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        let entry = requests.get_mut(id).filter(|entry| entry.alive(now) && entry.belongs_to(user_id, session_hash))?;
        if entry.answer.is_none() {
            let error = entry.request.error.clone()?;
            let params = [("error", error.error), ("error_description", error.description.as_str())];
            entry.answer = Some(Continue { location: answer_url(&entry.request, &params), code: None });
            entry.until = now + ANSWER_LIFETIME;
        }
        entry.answer.clone()
    }

    /// Forgets requests that are over.
    pub fn sweep(&self) {
        let now = Instant::now();
        self.requests.lock().retain(|_, entry| entry.alive(now));
    }

    /// Makes request `id` end in `left` (tests).
    #[cfg(test)]
    fn ends_in(&self, id: &str, left: Duration) {
        if let Some(entry) = self.requests.lock().get_mut(id) {
            entry.until = Instant::now() + left;
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.requests.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(user_id: i64) -> PendingRequest {
        PendingRequest {
            user_id,
            session_hash: format!("session-of-{user_id}"),
            issuer: "http://pley.local:7539".into(),
            app: AppSnapshot {
                id: "https://hhaidar.github.io/pley/oauth/desktop.json".into(),
                host: "hhaidar.github.io".into(),
                name: Some("pley for desktop".into()),
                first_party: true,
                refresh: true,
            },
            redirect_uri: "http://127.0.0.1:53111/oauth/callback?keep=1".into(),
            state: Some("st&te".into()),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into(),
            scopes: Scopes::parse("library").unwrap(),
            error: None,
        }
    }

    fn code(_: &PendingRequest) -> Result<String, ()> {
        Ok("c0de".into())
    }

    #[test]
    fn requests_belong_to_their_person_and_session() {
        let pending = Pending::default();
        let id = pending.create(request(1)).unwrap();
        assert_eq!(id.len(), 43);
        assert_eq!(pending.get(&id, 1, "session-of-1"), Some(request(1)));
        assert_eq!(pending.get(&id, 2, "session-of-1"), None);
        assert_eq!(pending.get(&id, 1, "another session"), None);
        assert_eq!(pending.decide(&id, 1, "another session", true, code), Err(DecideError::NotFound));
        assert_eq!(pending.location(&id, 1, "session-of-1"), None, "not answered yet");
        assert_eq!(pending.decide(&id, 1, "session-of-1", true, code), Ok(()));
        assert_eq!(pending.location(&id, 1, "another session"), None);
        let back = pending.location(&id, 1, "session-of-1").unwrap();
        assert_eq!(back.code.as_deref(), Some("c0de"));
        assert_eq!(
            back.location,
            "http://127.0.0.1:53111/oauth/callback?keep=1&code=c0de&state=st%26te&iss=http%3A%2F%2Fpley.local%3A7539"
        );
    }

    #[test]
    fn a_decision_stands() {
        let pending = Pending::default();
        let id = pending.create(request(1)).unwrap();
        assert_eq!(pending.decide(&id, 1, "session-of-1", false, code), Ok(()));
        // The same again changes nothing, and makes no code.
        let no_code = |_: &PendingRequest| -> Result<String, ()> { panic!("no code for a refusal") };
        assert_eq!(pending.decide(&id, 1, "session-of-1", false, no_code), Ok(()));
        assert_eq!(pending.decide(&id, 1, "session-of-1", true, code), Err(DecideError::Conflict));
        let back = pending.location(&id, 1, "session-of-1").unwrap();
        assert_eq!(back.code, None);
        assert!(back.location.contains("?keep=1&error=access_denied&state=st%26te&iss="), "{}", back.location);
        // A code that can't be made decides nothing.
        let id = pending.create(request(1)).unwrap();
        assert_eq!(pending.decide(&id, 1, "session-of-1", true, |_| Err("full")), Err(DecideError::Code("full")));
        assert_eq!(pending.decide(&id, 1, "session-of-1", false, code), Ok(()));
    }

    #[test]
    fn mistakes_only_go_back() {
        let pending = Pending::default();
        let mut wrong = request(1);
        wrong.state = None;
        wrong.error = Some(ReturnedError { error: "invalid_scope", description: "no such scope".into() });
        let id = pending.create(wrong).unwrap();
        assert_eq!(pending.decide(&id, 1, "session-of-1", true, code), Err(DecideError::Conflict));
        let back = pending.location(&id, 1, "session-of-1").unwrap();
        assert_eq!(
            back.location,
            "http://127.0.0.1:53111/oauth/callback?keep=1&error=invalid_scope&error_description=no+such+scope&iss=\
             http%3A%2F%2Fpley.local%3A7539"
        );
        // It repeats for a minute after the first time.
        assert_eq!(pending.location(&id, 1, "session-of-1"), Some(back));
        pending.ends_in(&id, Duration::ZERO);
        assert_eq!(pending.location(&id, 1, "session-of-1"), None);
    }

    #[test]
    fn requests_end() {
        let pending = Pending::default();
        let id = pending.create(request(1)).unwrap();
        pending.ends_in(&id, Duration::ZERO);
        assert_eq!(pending.get(&id, 1, "session-of-1"), None);
        assert_eq!(pending.decide(&id, 1, "session-of-1", true, code), Err(DecideError::NotFound));
        pending.sweep();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn requests_are_capped() {
        let pending = Pending::default();
        let ids: Vec<String> = (0..MAX_PENDING_PER_USER).map(|_| pending.create(request(1)).unwrap()).collect();
        // The 11th pushes the oldest out.
        let newest = pending.create(request(1)).unwrap();
        assert_eq!(pending.get(&ids[0], 1, "session-of-1"), None);
        assert!(ids[1..].iter().chain([&newest]).all(|id| pending.get(id, 1, "session-of-1").is_some()));
        assert_eq!(pending.len(), MAX_PENDING_PER_USER);
        // At most 1,000 in all.
        for user in 2..=(MAX_PENDING / MAX_PENDING_PER_USER) as i64 {
            for _ in 0..MAX_PENDING_PER_USER {
                pending.create(request(user)).unwrap();
            }
        }
        assert_eq!(pending.len(), MAX_PENDING);
        assert_eq!(pending.create(request(1000)), Err(Full));
        // Someone with requests still gets a new one, in place of their oldest.
        assert!(pending.create(request(1)).is_ok());
        // Room comes back as they end.
        pending.ends_in(&newest, Duration::ZERO);
        assert!(pending.create(request(1000)).is_ok());
    }
}
