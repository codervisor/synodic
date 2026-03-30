use anyhow::{Context, Result};
use postgres::{Client, NoTls};
use std::cell::RefCell;
use std::collections::HashMap;

use crate::events::{Event, EventFilter, Stats};
use crate::storage::EventStore;

/// PostgreSQL-backed event store for team/org deployments.
pub struct PostgresStore {
    client: RefCell<Client>,
}

impl PostgresStore {
    /// Connect to a PostgreSQL database using the given connection URL.
    ///
    /// URL format: `postgres://user:pass@host:port/dbname`
    pub fn connect(url: &str) -> Result<Self> {
        let mut client = Client::connect(url, NoTls)
            .with_context(|| format!("connecting to PostgreSQL: {url}"))?;
        migrate(&mut client)?;
        Ok(Self {
            client: RefCell::new(client),
        })
    }
}

fn migrate(client: &mut Client) -> Result<()> {
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS events (
                    id TEXT PRIMARY KEY,
                    event_type TEXT NOT NULL,
                    title TEXT NOT NULL,
                    severity TEXT NOT NULL,
                    source TEXT NOT NULL,
                    metadata JSONB NOT NULL DEFAULT '{}',
                    resolved BOOLEAN NOT NULL DEFAULT FALSE,
                    resolution_notes TEXT,
                    created_at TIMESTAMPTZ NOT NULL,
                    resolved_at TIMESTAMPTZ
                );

                CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
                CREATE INDEX IF NOT EXISTS idx_events_severity ON events(severity);
                CREATE INDEX IF NOT EXISTS idx_events_resolved ON events(resolved);
                CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

                -- GIN index on title for full-text search
                CREATE INDEX IF NOT EXISTS idx_events_title_gin ON events USING gin(to_tsvector('english', title));",
        )
        .context("running PostgreSQL migrations")?;
    Ok(())
}

impl EventStore for PostgresStore {
    fn insert(&self, event: &Event) -> Result<()> {
        let mut client = self.client.borrow_mut();
        client.execute(
            "INSERT INTO events (id, event_type, title, severity, source, metadata, resolved, resolution_notes, created_at, resolved_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &event.id,
                &event.event_type.as_str(),
                &event.title,
                &event.severity.as_str(),
                &event.source,
                &serde_json::to_string(&event.metadata)?,
                &event.resolved,
                &event.resolution_notes,
                &event.created_at,
                &event.resolved_at,
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<Event>> {
        let mut client = self.client.borrow_mut();
        let rows = client.query("SELECT * FROM events WHERE id = $1", &[&id])?;
        match rows.first() {
            Some(row) => Ok(Some(row_to_event(row)?)),
            None => Ok(None),
        }
    }

    fn list(&self, filter: &EventFilter) -> Result<Vec<Event>> {
        let mut sql = String::from("SELECT * FROM events WHERE TRUE");
        let mut param_idx = 1u32;
        let mut params: Vec<String> = Vec::new();

        if let Some(ref et) = filter.event_type {
            params.push(et.as_str().to_string());
            sql.push_str(&format!(" AND event_type = ${param_idx}"));
            param_idx += 1;
        }
        if let Some(ref sev) = filter.severity {
            params.push(sev.as_str().to_string());
            sql.push_str(&format!(" AND severity = ${param_idx}"));
            param_idx += 1;
        }
        if filter.unresolved_only {
            sql.push_str(" AND resolved = FALSE");
        }
        if let Some(ref src) = filter.source {
            params.push(src.clone());
            sql.push_str(&format!(" AND source = ${param_idx}"));
            param_idx += 1;
        }
        if let Some(since) = filter.since {
            params.push(since.to_rfc3339());
            sql.push_str(&format!(" AND created_at >= ${param_idx}::timestamptz"));
            param_idx += 1;
        }

        let _ = param_idx; // suppress unused warning

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut client = self.client.borrow_mut();
        let param_refs: Vec<&(dyn postgres::types::ToSql + Sync)> = params
            .iter()
            .map(|s| s as &(dyn postgres::types::ToSql + Sync))
            .collect();
        let rows = client.query(&sql as &str, &param_refs)?;

        let mut events = Vec::new();
        for row in &rows {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    }

    fn resolve(&self, id: &str, notes: &str) -> Result<()> {
        let mut client = self.client.borrow_mut();
        let updated = client.execute(
            "UPDATE events SET resolved = TRUE, resolution_notes = $1, resolved_at = $2 WHERE id = $3",
            &[&notes, &chrono::Utc::now(), &id],
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
            *by_type
                .entry(e.event_type.as_str().to_string())
                .or_default() += 1;
            *by_severity
                .entry(e.severity.as_str().to_string())
                .or_default() += 1;
        }

        Ok(Stats {
            total,
            unresolved,
            by_type,
            by_severity,
        })
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        let mut client = self.client.borrow_mut();
        let rows = client.query(
            "SELECT * FROM events
             WHERE to_tsvector('english', title) @@ plainto_tsquery('english', $1)
             ORDER BY created_at DESC
             LIMIT $2",
            &[&query, &(limit as i64)],
        )?;

        let mut events = Vec::new();
        for row in &rows {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    }

    fn ping(&self) -> Result<()> {
        let mut client = self.client.borrow_mut();
        client
            .execute("SELECT 1", &[])
            .context("PostgreSQL ping failed")?;
        Ok(())
    }
}

fn row_to_event(row: &postgres::Row) -> Result<Event> {
    let event_type_str: String = row.get("event_type");
    let severity_str: String = row.get("severity");
    let metadata_str: String = row.get("metadata");
    let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
    let resolved_at: Option<chrono::DateTime<chrono::Utc>> = row.get("resolved_at");

    Ok(Event {
        id: row.get("id"),
        event_type: event_type_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?,
        title: row.get("title"),
        severity: severity_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?,
        source: row.get("source"),
        metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({})),
        resolved: row.get("resolved"),
        resolution_notes: row.get("resolution_notes"),
        created_at,
        resolved_at,
    })
}
