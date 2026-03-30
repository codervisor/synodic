pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

use crate::events::{Event, EventFilter, Stats};
use anyhow::Result;

/// Abstraction over event persistence backends.
pub trait EventStore {
    fn insert(&self, event: &Event) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<Event>>;
    fn list(&self, filter: &EventFilter) -> Result<Vec<Event>>;
    fn resolve(&self, id: &str, notes: &str) -> Result<()>;
    fn stats(&self, filter: &EventFilter) -> Result<Stats>;
    fn search(&self, query: &str, limit: usize) -> Result<Vec<Event>>;

    /// Check that the storage backend is reachable.
    fn ping(&self) -> Result<()> {
        Ok(())
    }
}
