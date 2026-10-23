//! First run: whoever reaches a server that has no account yet creates the
//! administrator. From this computer or the local network that takes only a
//! username and a password. From anywhere else (the internet, or through a
//! reverse proxy) it also takes a one-time setup code, which the server
//! prints in its log, so a server that is reachable from outside can't be
//! claimed by someone else before its owner gets to it.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use parking_lot::Mutex;
use rand_core::{OsRng, RngCore};

/// Letters and digits that can't be mistaken for one another.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const LENGTH: usize = 8;

/// The setup code, while the server has no account.
#[derive(Default)]
pub struct SetupCode(Mutex<Option<String>>);

impl SetupCode {
    /// A new code (`ABCD-EFGH`), replacing any earlier one.
    pub fn issue(&self) -> String {
        let mut bytes = [0u8; LENGTH];
        OsRng.fill_bytes(&mut bytes);
        let code: String = bytes.iter().map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char).collect();
        *self.0.lock() = Some(code.clone());
        format!("{}-{}", &code[..4], &code[4..])
    }

    /// The code as shown (`ABCD-EFGH`), while there is one.
    pub fn shown(&self) -> Option<String> {
        self.0.lock().as_deref().map(|code| format!("{}-{}", &code[..4], &code[4..]))
    }

    /// Whether `given` is the code, in any case, with or without the dash.
    pub fn matches(&self, given: &str) -> bool {
        let given: String =
            given.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
        let current = self.0.lock();
        current.as_deref().is_some_and(|code| constant_time_eq(code.as_bytes(), given.as_bytes()))
    }

    /// Setup is done: the code no longer claims anything.
    pub fn clear(&self) {
        *self.0.lock() = None;
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Whether the request comes straight from this computer or the local
/// network (see [`is_local_request`]).
pub struct LocalRequest(pub bool);

impl<S: Send + Sync> FromRequestParts<S> for LocalRequest {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|ConnectInfo(addr)| addr.ip());
        Ok(LocalRequest(is_local_request(&parts.headers, peer)))
    }
}

/// Headers that proxies (nginx, Caddy, Traefik, Cloudflare…) add: a request
/// that went through one may come from anywhere.
const PROXY_HEADERS: &[&str] = &["forwarded", "x-forwarded-for", "x-real-ip", "cf-connecting-ip", "true-client-ip"];

/// Whether a request comes straight from this computer or the local network:
/// a loopback, private or link-local address, and no proxy in between. A
/// request whose address isn't known counts as coming from elsewhere.
pub fn is_local_request(headers: &HeaderMap, peer: Option<IpAddr>) -> bool {
    if PROXY_HEADERS.iter().any(|h| headers.contains_key(*h)) {
        return false;
    }
    peer.is_some_and(is_local_address)
}

/// Loopback, private, link-local or Tailscale (carrier-grade NAT) addresses.
pub fn is_local_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                // Carrier-grade NAT space, which Tailscale uses for its devices.
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local_address(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                v6.is_loopback()
                    // Unique local (fc00::/7) and link-local (fe80::/10) addresses.
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_once_issued_whatever_the_case_or_dash() {
        let code = SetupCode::default();
        assert!(!code.matches(""), "no code before one is issued");
        let shown = code.issue();
        assert_eq!(code.shown().as_deref(), Some(shown.as_str()));
        assert_eq!(shown.len(), LENGTH + 1);
        assert_eq!(&shown[4..5], "-");
        assert!(shown.chars().filter(|c| *c != '-').all(|c| ALPHABET.contains(&(c as u8))));
        assert!(code.matches(&shown));
        assert!(code.matches(&shown.replace('-', "").to_lowercase()));
        assert!(code.matches(&format!(" {shown} ")));
        assert!(!code.matches(&shown[..LENGTH]), "part of it");
        assert!(!code.matches("AAAA-AAAA"));
        let again = code.issue();
        assert!(code.matches(&again));
        code.clear();
        assert!(!code.matches(&again), "setup is done");
        assert_eq!(code.shown(), None);
    }

    #[test]
    fn only_direct_requests_from_this_computer_or_network_are_local() {
        let none = HeaderMap::new();
        let local = |ip: &str| is_local_request(&none, Some(ip.parse().unwrap()));
        for ip in ["127.0.0.1", "::1", "192.168.1.20", "10.0.0.7", "172.20.0.1", "169.254.3.4", "100.101.102.103"] {
            assert!(local(ip), "{ip}");
        }
        for ip in ["fd12:3456:789a::1", "fe80::1", "::ffff:192.168.1.20"] {
            assert!(local(ip), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "100.128.0.1", "2001:4860:4860::8888", "::ffff:8.8.8.8"] {
            assert!(!local(ip), "{ip}");
        }
        assert!(!is_local_request(&none, None), "an unknown address");

        // A proxy on the same computer forwards requests from anywhere.
        for header in PROXY_HEADERS {
            let mut headers = HeaderMap::new();
            headers.insert(*header, "203.0.113.9".parse().unwrap());
            assert!(!is_local_request(&headers, Some("127.0.0.1".parse().unwrap())), "{header}");
        }
    }
}
