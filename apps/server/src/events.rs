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
    ScanStarted { library_id: i64 },
    ScanProgress { library_id: i64, phase: ScanPhase, processed: u64, total: u64 },
    ScanFinished { library_id: i64, added: u64, updated: u64, removed: u64 },
    LibraryChanged { library_id: i64 },
    LibrariesChanged,
    SessionsChanged,
}

impl ServerEvent {
    /// The library the event is about, if it is about one.
    pub fn library_id(&self) -> Option<i64> {
        match self {
            ServerEvent::ScanStarted { library_id }
            | ServerEvent::ScanProgress { library_id, .. }
            | ServerEvent::ScanFinished { library_id, .. }
            | ServerEvent::LibraryChanged { library_id } => Some(*library_id),
            ServerEvent::LibrariesChanged | ServerEvent::SessionsChanged => None,
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
    }
}
