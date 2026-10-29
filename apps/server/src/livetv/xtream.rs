//! Xtream Codes accounts, the API most IPTV providers speak:
//! `player_api.php` says what the account is and lists its channels;
//! channel `n` plays at `/live/{user}/{password}/{n}.ts`; the guide is
//! `xmltv.php`. Servers send numbers as strings or numbers as they like,
//! so everything is read loosely.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the server says of the account.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// `Active`, `Expired`, `Banned`, `Disabled`.
    pub status: String,
    /// When it runs out (unix seconds); `None`: never.
    pub expires_at: Option<i64>,
    pub max_connections: Option<i64>,
    pub active_connections: Option<i64>,
    pub trial: bool,
}

/// A channel the account has.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub id: i64,
    pub number: Option<String>,
    pub name: String,
    pub logo: String,
    pub guide_id: String,
    pub category: String,
}

/// The server's base URL as given: `http://host:port` (a trailing slash,
/// or `player_api.php` pasted along, are dropped).
pub fn base(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix("/player_api.php").or_else(|| url.strip_suffix("/get.php")).unwrap_or(url);
    url.trim_end_matches('/').to_owned()
}

pub fn api_url(base: &str, username: &str, password: &str, action: Option<&str>) -> String {
    let mut url = format!("{base}/player_api.php?username={}&password={}", encode(username), encode(password));
    if let Some(action) = action {
        url.push_str("&action=");
        url.push_str(action);
    }
    url
}

pub fn stream_url(base: &str, username: &str, password: &str, id: i64) -> String {
    format!("{base}/live/{}/{}/{id}.ts", encode_path(username), encode_path(password))
}

pub fn guide_url(base: &str, username: &str, password: &str) -> String {
    format!("{base}/xmltv.php?username={}&password={}", encode(username), encode(password))
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

fn encode_path(text: &str) -> String {
    // Providers' user names and passwords are almost always plain; what isn't
    // is escaped as a path segment would be.
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The account in `player_api.php`'s answer; `Err` says why it isn't one
/// (wrong user name or password, most often).
pub fn account(answer: &Value) -> Result<Account, String> {
    let info = &answer["user_info"];
    if !info.is_object() {
        return Err("the server didn't answer as an Xtream Codes server does".into());
    }
    if number(&info["auth"]) == Some(0) {
        return Err("the server refused the user name and password".into());
    }
    Ok(Account {
        status: text(&info["status"]).unwrap_or_else(|| "Active".into()),
        expires_at: number(&info["exp_date"]).filter(|t| *t > 0),
        max_connections: number(&info["max_connections"]),
        active_connections: number(&info["active_cons"]),
        trial: number(&info["is_trial"]) == Some(1),
    })
}

/// The categories' names by id, from `get_live_categories`.
pub fn categories(answer: &Value) -> Vec<(String, String)> {
    answer
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((text(&c["category_id"])?, text(&c["category_name"]).unwrap_or_default())))
        .collect()
}

/// The channels, from `get_live_streams`, their categories named.
pub fn streams(answer: &Value, categories: &[(String, String)]) -> Vec<Stream> {
    answer
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| text(&s["stream_type"]).is_none_or(|t| t == "live" || t == "created_live" || t == "radio_streams"))
        .filter_map(|s| {
            let id = number(&s["stream_id"])?;
            let category_id = text(&s["category_id"]).unwrap_or_default();
            let category =
                categories.iter().find(|(id, _)| *id == category_id).map(|(_, name)| name.clone()).unwrap_or_default();
            Some(Stream {
                id,
                number: number(&s["num"]).filter(|n| *n > 0).map(|n| n.to_string()),
                name: text(&s["name"]).unwrap_or_else(|| format!("Channel {id}")),
                logo: text(&s["stream_icon"]).unwrap_or_default(),
                guide_id: text(&s["epg_channel_id"]).unwrap_or_default(),
                category,
            })
        })
        .collect()
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn number(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_accounts_and_channels_however_numbers_come() {
        let answer = json!({"user_info": {"auth": 1, "status": "Active", "exp_date": "1893456000",
            "max_connections": "2", "active_cons": 0, "is_trial": "0"}, "server_info": {}});
        assert_eq!(
            account(&answer).unwrap(),
            Account {
                status: "Active".into(),
                expires_at: Some(1_893_456_000),
                max_connections: Some(2),
                active_connections: Some(0),
                trial: false
            }
        );
        assert!(account(&json!({"user_info": {"auth": 0}})).is_err());
        assert!(account(&json!([])).is_err());
        let cats = categories(&json!([{"category_id": "7", "category_name": "UK"}, {"category_id": 8}]));
        let list = streams(
            &json!([
                {"num": 1, "name": "BBC One", "stream_type": "live", "stream_id": "123",
                 "stream_icon": "https://l/1.png", "epg_channel_id": "bbc1.uk", "category_id": "7"},
                {"num": "2", "name": "A film", "stream_type": "movie", "stream_id": 9},
                {"name": "No id"}
            ]),
            &cats,
        );
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0],
            Stream {
                id: 123,
                number: Some("1".into()),
                name: "BBC One".into(),
                logo: "https://l/1.png".into(),
                guide_id: "bbc1.uk".into(),
                category: "UK".into()
            }
        );
    }

    #[test]
    fn urls_follow_the_api() {
        let base = base("http://tv.example.com:8080/player_api.php");
        assert_eq!(base, "http://tv.example.com:8080");
        assert_eq!(
            api_url(&base, "me", "p&ss", Some("get_live_streams")),
            "http://tv.example.com:8080/player_api.php?username=me&password=p%26ss&action=get_live_streams"
        );
        assert_eq!(stream_url(&base, "me", "p/ss", 5), "http://tv.example.com:8080/live/me/p%2Fss/5.ts");
        assert_eq!(guide_url(&base, "me", "pw"), "http://tv.example.com:8080/xmltv.php?username=me&password=pw");
    }
}
