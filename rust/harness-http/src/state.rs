use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use harness_core::events::Event;
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

#[cfg(feature = "postgres")]
use harness_core::storage::postgres::PostgresStore;

/// Shared application state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Mutex<Box<dyn EventStore + Send>>>,
    /// Broadcast channel for real-time event streaming via WebSocket.
    pub event_tx: broadcast::Sender<Event>,
}

impl AppState {
    pub fn new(database_url: &str) -> anyhow::Result<Self> {
        let store: Box<dyn EventStore + Send> = create_store(database_url)?;
        let (event_tx, _) = broadcast::channel(256);
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            event_tx,
        })
    }
}

fn create_store(database_url: &str) -> anyhow::Result<Box<dyn EventStore + Send>> {
    #[cfg(feature = "postgres")]
    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        eprintln!("Storage: PostgreSQL");
        let store = PostgresStore::connect(database_url)?;
        return Ok(Box::new(store));
    }

    // Default: SQLite
    let path = std::path::PathBuf::from(database_url);
    eprintln!("Storage: SQLite at {}", path.display());
    let store = SqliteStore::open(&path)?;
    Ok(Box::new(store))
}
