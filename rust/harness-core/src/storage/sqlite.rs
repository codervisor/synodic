use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

use crate::events::{Event, EventFilter, Stats};
use crate::storage::EventStore;

/// SQLite-backed event store. Default backend for local development.
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("opening SQLite database")?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory database (useful for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory SQLite")?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                title TEXT NOT NULL,
                severity TEXT NOT NULL,
                source TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                resolved INTEGER NOT NULL DEFAULT 0,
                resolution_notes TEXT,
                created_at TEXT NOT NULL,
                resolved_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
            CREATE INDEX IF NOT EXISTS idx_events_severity ON events(severity);
            CREATE INDEX IF NOT EXISTS idx_events_resolved ON events(resolved);
            CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

            CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
                id,
                title,
                source,
                resolution_notes,
                content='events',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS events_ai AFTER INSERT ON events BEGIN
                INSERT INTO events_fts(id, title, source, resolution_notes)
                VALUES (new.id, new.title, new.source, new.resolution_notes);
            END;

            CREATE TRIGGER IF NOT EXISTS events_au AFTER UPDATE ON events BEGIN
                INSERT INTO events_fts(events_fts, id, title, source, resolution_notes)
                VALUES ('delete', old.id, old.title, old.source, old.resolution_notes);
                INSERT INTO events_fts(id, title, source, resolution_notes)
                VALUES (new.id, new.title, new.source, new.resolution_notes);
            END;",
        )
        .context("running migrations")?;
        Ok(())
    }

    fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<Event> {
        let event_type_str: String = row.get("event_type")?;
        let severity_str: String = row.get("severity")?;
        let metadata_str: String = row.get("metadata")?;
        let created_str: String = row.get("created_at")?;
        let resolved_at_str: Option<String> = row.get("resolved_at")?;

        Ok(Event {
            id: row.get("id")?,
            event_type: event_type_str
                .parse()
                .map_err(|e: String| rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                ))?,
            title: row.get("title")?,
            severity: severity_str
                .parse()
                .map_err(|e: String| rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                ))?,
            source: row.get("source")?,
            metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::Value::Object(Default::default())),
            resolved: row.get::<_, i32>("resolved")? != 0,
            resolution_notes: row.get("resolution_notes")?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            resolved_at: resolved_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .ok()
            }),
        })
    }
}

impl EventStore for SqliteStore {
    fn insert(&self, event: &Event) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events (id, event_type, title, severity, source, metadata, resolved, resolution_notes, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.id,
                event.event_type.as_str(),
                event.title,
                event.severity.as_str(),
                event.source,
                serde_json::to_string(&event.metadata)?,
                event.resolved as i32,
                event.resolution_notes,
                event.created_at.to_rfc3339(),
                event.resolved_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare("SELECT * FROM events WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], Self::row_to_event)?;
        match rows.next() {
            Some(Ok(event)) => Ok(Some(event)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    fn list(&self, filter: &EventFilter) -> Result<Vec<Event>> {
        let mut sql = String::from("SELECT * FROM events WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref et) = filter.event_type {
            param_values.push(Box::new(et.as_str().to_string()));
            sql.push_str(&format!(" AND event_type = ?{}", param_values.len()));
        }
        if let Some(ref sev) = filter.severity {
            param_values.push(Box::new(sev.as_str().to_string()));
            sql.push_str(&format!(" AND severity = ?{}", param_values.len()));
        }
        if filter.unresolved_only {
            sql.push_str(" AND resolved = 0");
        }
        if let Some(ref src) = filter.source {
            param_values.push(Box::new(src.clone()));
            sql.push_str(&format!(" AND source = ?{}", param_values.len()));
        }
        if let Some(since) = filter.since {
            param_values.push(Box::new(since.to_rfc3339()));
            sql.push_str(&format!(" AND created_at >= ?{}", param_values.len()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_ref.as_slice(), Self::row_to_event)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    fn resolve(&self, id: &str, notes: &str) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE events SET resolved = 1, resolution_notes = ?1, resolved_at = ?2 WHERE id = ?3",
            params![notes, chrono::Utc::now().to_rfc3339(), id],
        )?;
        if updated == 0 {
            anyhow::bail!("event not found: {id}");
        }
        Ok(())
    }

    fn stats(&self, filter: &EventFilter) -> Result<Stats> {
        let events = self.list(filter)?;
        let total = events.len();
        let unresolved = events.iter().filter(|e| !e.resolved).count();

        let mut by_type: HashMap<String, usize> = HashMap::new();
        let mut by_severity: HashMap<String, usize> = HashMap::new();
        for e in &events {
            *by_type.entry(e.event_type.as_str().to_string()).or_default() += 1;
            *by_severity.entry(e.severity.as_str().to_string()).or_default() += 1;
        }

        Ok(Stats {
            total,
            unresolved,
            by_type,
            by_severity,
        })
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        let sql = "SELECT e.* FROM events e
                   JOIN events_fts f ON e.id = f.id
                   WHERE events_fts MATCH ?1
                   ORDER BY e.created_at DESC
                   LIMIT ?2";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![query, limit as i64], Self::row_to_event)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Event, EventType, Severity};

    fn test_store() -> SqliteStore {
        SqliteStore::open_in_memory().unwrap()
    }

    fn sample_event(title: &str, et: EventType, sev: Severity) -> Event {
        Event::new(
            et,
            title.to_string(),
            sev,
            "claude".to_string(),
            serde_json::json!({}),
        )
    }

    #[test]
    fn test_insert_and_get() {
        let store = test_store();
        let event = sample_event("test error", EventType::ToolCallError, Severity::Medium);
        let id = event.id.clone();
        store.insert(&event).unwrap();

        let fetched = store.get(&id).unwrap().unwrap();
        assert_eq!(fetched.title, "test error");
        assert_eq!(fetched.event_type, EventType::ToolCallError);
        assert!(!fetched.resolved);
    }

    #[test]
    fn test_get_missing() {
        let store = test_store();
        assert!(store.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_list_all() {
        let store = test_store();
        store.insert(&sample_event("a", EventType::Hallucination, Severity::Low)).unwrap();
        store.insert(&sample_event("b", EventType::ToolCallError, Severity::High)).unwrap();

        let all = store.list(&EventFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_list_filter_type() {
        let store = test_store();
        store.insert(&sample_event("a", EventType::Hallucination, Severity::Low)).unwrap();
        store.insert(&sample_event("b", EventType::ToolCallError, Severity::High)).unwrap();

        let filtered = store.list(&EventFilter {
            event_type: Some(EventType::Hallucination),
            ..Default::default()
        }).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "a");
    }

    #[test]
    fn test_list_filter_unresolved() {
        let store = test_store();
        let e1 = sample_event("a", EventType::Hallucination, Severity::Low);
        let e2 = sample_event("b", EventType::ToolCallError, Severity::High);
        let id = e1.id.clone();
        store.insert(&e1).unwrap();
        store.insert(&e2).unwrap();
        store.resolve(&id, "fixed").unwrap();

        let unresolved = store.list(&EventFilter {
            unresolved_only: true,
            ..Default::default()
        }).unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].title, "b");
    }

    #[test]
    fn test_resolve() {
        let store = test_store();
        let event = sample_event("to resolve", EventType::Misalignment, Severity::High);
        let id = event.id.clone();
        store.insert(&event).unwrap();

        store.resolve(&id, "addressed in PR #42").unwrap();

        let fetched = store.get(&id).unwrap().unwrap();
        assert!(fetched.resolved);
        assert_eq!(fetched.resolution_notes.as_deref(), Some("addressed in PR #42"));
        assert!(fetched.resolved_at.is_some());
    }

    #[test]
    fn test_resolve_missing() {
        let store = test_store();
        assert!(store.resolve("nonexistent", "notes").is_err());
    }

    #[test]
    fn test_stats() {
        let store = test_store();
        store.insert(&sample_event("a", EventType::Hallucination, Severity::Low)).unwrap();
        store.insert(&sample_event("b", EventType::ToolCallError, Severity::High)).unwrap();
        store.insert(&sample_event("c", EventType::Hallucination, Severity::Medium)).unwrap();

        let id = {
            let all = store.list(&EventFilter::default()).unwrap();
            all[0].id.clone()
        };
        store.resolve(&id, "done").unwrap();

        let stats = store.stats(&EventFilter::default()).unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.unresolved, 2);
        assert_eq!(*stats.by_type.get("hallucination").unwrap_or(&0), 2);
        assert_eq!(*stats.by_type.get("tool_call_error").unwrap_or(&0), 1);
    }

    #[test]
    fn test_search() {
        let store = test_store();
        store.insert(&sample_event("missing file reference", EventType::Hallucination, Severity::Low)).unwrap();
        store.insert(&sample_event("command failed", EventType::ToolCallError, Severity::High)).unwrap();

        let results = store.search("missing", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "missing file reference");
    }

    #[test]
    fn test_list_with_limit() {
        let store = test_store();
        for i in 0..10 {
            store.insert(&sample_event(&format!("event {i}"), EventType::ToolCallError, Severity::Low)).unwrap();
        }
        let limited = store.list(&EventFilter { limit: Some(3), ..Default::default() }).unwrap();
        assert_eq!(limited.len(), 3);
    }
}
