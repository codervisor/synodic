use std::path::Path;
use std::sync::{Arc, Mutex};

use harness_core::storage::sqlite::SqliteStore;

/// Shared application state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Mutex<SqliteStore>>,
}

impl AppState {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let store = SqliteStore::open(db_path)?;
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
        })
    }
}
