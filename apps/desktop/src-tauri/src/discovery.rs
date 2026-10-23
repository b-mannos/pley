//! Finding pley servers on the local network: `pley?` broadcast to UDP port
//! 7539, the answers collected for a moment (the server side is
//! apps/server/src/discovery.rs). A server on this computer that only
//! listens on 127.0.0.1 doesn't answer broadcasts, so it is asked directly.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;

use crate::servers::check_server;

pub const PORT: u16 = 7539;
const REQUEST: &[u8] = b"pley?";
/// How long answers are waited for.
const WAIT: Duration = Duration::from_millis(1500);

/// A server that answered.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoundServer {
    pub url: String,
    pub name: String,
    pub id: Option<String>,
    pub version: String,
}

/// A server's answer.
#[derive(Debug, Deserialize)]
struct Announcement {
    pley: u32,
    id: String,
    name: String,
    version: String,
    port: u16,
}

/// The server behind an answer from `from`, if it is one.
fn read_answer(bytes: &[u8], from: SocketAddr) -> Option<FoundServer> {
    let answer: Announcement = serde_json::from_slice(bytes).ok()?;
    if answer.pley != 1 || answer.port == 0 {
        return None;
    }
    let host = match from.ip() {
        ip if ip.is_loopback() => "localhost".to_owned(),
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => format!("[{v6}]"),
        },
    };
    Some(FoundServer {
        url: format!("http://{host}:{}", answer.port),
        name: answer.name,
        id: Some(answer.id),
        version: answer.version,
    })
}

/// Where to send the request: every network's broadcast address, the
/// general one, and this computer.
fn targets() -> Vec<SocketAddr> {
    let mut out = vec![SocketAddr::from((Ipv4Addr::BROADCAST, PORT)), SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))];
    for interface in if_addrs::get_if_addrs().unwrap_or_default() {
        if let if_addrs::IfAddr::V4(v4) = interface.addr {
            if let Some(broadcast) = v4.broadcast.filter(|_| !v4.ip.is_loopback()) {
                let target = SocketAddr::from((broadcast, PORT));
                if !out.contains(&target) {
                    out.push(target);
                }
            }
        }
    }
    out
}

/// Asks `targets` and gathers the answers for `wait`, one entry per server
/// (by id): this computer's address wins over its network ones.
pub async fn search(targets: &[SocketAddr], wait: Duration) -> Vec<FoundServer> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else { return Vec::new() };
    let _ = socket.set_broadcast(true);
    for target in targets {
        let _ = socket.send_to(REQUEST, target).await;
    }
    let mut found: HashMap<String, FoundServer> = HashMap::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + wait;
    while let Ok(Ok((len, from))) = tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await {
        let Some(server) = read_answer(&buf[..len], from) else { continue };
        let key = server.id.clone().unwrap_or_else(|| server.url.clone());
        let local = server.url.starts_with("http://localhost:");
        if local || !found.contains_key(&key) {
            found.insert(key, server);
        }
    }
    let mut list: Vec<FoundServer> = found.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.url.cmp(&b.url)));
    list
}

#[tauri::command]
pub async fn discover() -> Vec<FoundServer> {
    let mut found = search(&targets(), WAIT).await;
    if !found.iter().any(|s| s.url.starts_with("http://localhost:")) {
        let local = format!("http://localhost:{PORT}");
        if let Ok(info) = check_server(local.clone()).await {
            found.insert(0, FoundServer { url: local, name: info.name, id: None, version: info.version });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_become_addresses() {
        let answer = br#"{"pley":1,"id":"ab","name":"Den","version":"0.1.0","port":7539}"#;
        let found = read_answer(answer, "192.168.1.20:7539".parse().unwrap()).unwrap();
        assert_eq!((found.url.as_str(), found.name.as_str(), found.id.as_deref()), ("http://192.168.1.20:7539", "Den", Some("ab")));
        assert_eq!(read_answer(answer, "127.0.0.1:7539".parse().unwrap()).unwrap().url, "http://localhost:7539");
        assert_eq!(read_answer(answer, "[fd00::5]:7539".parse().unwrap()).unwrap().url, "http://[fd00::5]:7539");
        assert_eq!(read_answer(b"hello", "192.168.1.20:7539".parse().unwrap()), None);
        let future = br#"{"pley":2,"id":"ab","name":"Den","version":"9.0.0","port":7539}"#;
        assert_eq!(read_answer(future, "192.168.1.20:7539".parse().unwrap()), None);
    }

    #[tokio::test]
    async fn gathers_one_answer_per_server() {
        // Two sockets standing in for one server's two addresses, and another server.
        let same_a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let other = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let targets = [same_a.local_addr().unwrap(), other.local_addr().unwrap()];
        for (socket, id, name) in [(same_a, "a", "Den"), (other, "b", "Attic")] {
            tokio::spawn(async move {
                let mut buf = [0u8; 16];
                let (len, from) = socket.recv_from(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], REQUEST);
                let answer = format!(r#"{{"pley":1,"id":"{id}","name":"{name}","version":"0.1.0","port":7539}}"#);
                socket.send_to(answer.as_bytes(), from).await.unwrap();
                socket.send_to(answer.as_bytes(), from).await.unwrap();
            });
        }
        let found = search(&targets, Duration::from_millis(400)).await;
        assert_eq!(found.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Attic", "Den"]);
    }
}
