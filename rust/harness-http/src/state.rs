use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use harness_core::events::Event;
use harness_core::storage::sqlite::SqliteStore;

/// Shared application state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Mutex<SqliteStore>>,
    /// Broadcast channel for real-time event streaming via WebSocket.
    pub event_tx: broadcast::Sender<Event>,
}

impl AppState {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let store = SqliteStore::open(db_path)?;
        let (event_tx, _) = broadcast::channel(256);
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            event_tx,
        })
    }
}
