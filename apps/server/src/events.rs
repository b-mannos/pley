//! In-process event bus feeding the `/api/events` Server-Sent Events stream.

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    Queued,
    Walking,
    Probing,
    Metadata,
    Done,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    ScanStarted {
        library_id: i64,
    },
    ScanProgress {
        library_id: i64,
        phase: ScanPhase,
        processed: u64,
        total: u64,
    },
    ScanFinished {
        library_id: i64,
        added: u64,
        updated: u64,
        removed: u64,
    },
    LibraryChanged {
        library_id: i64,
    },
    LibrariesChanged,
    SessionsChanged,
    /// The user's My List changed (added, removed, or a movie in it was
    /// watched): their other devices fetch Home again.
    WatchlistChanged {
        #[serde(skip)]
        user_id: i64,
    },
    /// The user's watch state of an item changed (watched or not, a resume
    /// point started, moved or cleared, in or out of Continue Watching):
    /// their devices fetch Home and that item's pages again. `item_ids` is
    /// the item, then its parent and grandparent (a season and its show),
    /// whose counts follow it; `children`: everything under the first one
    /// changed too (a show or season marked as a whole).
    UserStateChanged {
        #[serde(skip)]
        user_id: i64,
        item_ids: Vec<i64>,
        children: bool,
    },
    /// A playback session was stopped by someone else than its player (an
    /// administrator, or its owner on another device): the player stops.
    /// `by_admin` is true when an administrator stopped another user's
    /// session.
    SessionStopped {
        #[serde(skip)]
        user_id: i64,
        session_id: String,
        message: Option<String>,
        by_admin: bool,
    },
}

impl ServerEvent {
    /// The library the event is about, if it is about one.
    pub fn library_id(&self) -> Option<i64> {
        match self {
            ServerEvent::ScanStarted { library_id }
            | ServerEvent::ScanProgress { library_id, .. }
            | ServerEvent::ScanFinished { library_id, .. }
            | ServerEvent::LibraryChanged { library_id } => Some(*library_id),
            ServerEvent::LibrariesChanged
            | ServerEvent::SessionsChanged
            | ServerEvent::WatchlistChanged { .. }
            | ServerEvent::UserStateChanged { .. }
            | ServerEvent::SessionStopped { .. } => None,
        }
    }

    /// The only user the event is for, if it is for one.
    pub fn user_id(&self) -> Option<i64> {
        match self {
            ServerEvent::WatchlistChanged { user_id }
            | ServerEvent::UserStateChanged { user_id, .. }
            | ServerEvent::SessionStopped { user_id, .. } => Some(*user_id),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<ServerEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(512);
        Self { tx }
    }

    pub fn send(&self, event: ServerEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_type_tag() {
        let json = serde_json::to_string(&ServerEvent::LibrariesChanged).unwrap();
        assert_eq!(json, r#"{"type":"libraries_changed"}"#);
        let json = serde_json::to_string(&ServerEvent::ScanProgress {
            library_id: 1,
            phase: ScanPhase::Probing,
            processed: 2,
            total: 3,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"scan_progress","library_id":1,"phase":"probing","processed":2,"total":3}"#);
        // Whom an event is for stays on the server.
        let json = serde_json::to_string(&ServerEvent::WatchlistChanged { user_id: 4 }).unwrap();
        assert_eq!(json, r#"{"type":"watchlist_changed"}"#);
        let event = ServerEvent::UserStateChanged { user_id: 4, item_ids: vec![12, 11, 10], children: false };
        assert_eq!(event.user_id(), Some(4));
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"user_state_changed","item_ids":[12,11,10],"children":false}"#);
        let json = serde_json::to_string(&ServerEvent::SessionStopped {
            user_id: 4,
            session_id: "s1".into(),
            message: None,
            by_admin: true,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"session_stopped","session_id":"s1","message":null,"by_admin":true}"#);
    }
}
