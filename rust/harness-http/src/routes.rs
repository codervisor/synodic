use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, patch};
use axum::Router;
use serde::Deserialize;

use harness_core::events::{Event, EventFilter, EventType, Severity};
use harness_core::rules;
use harness_core::storage::EventStore;

use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/events", get(list_events).post(submit_event))
        .route("/events/{id}", get(get_event))
        .route("/events/{id}/resolve", patch(resolve_event))
        .route("/rules", get(list_rules))
        .route("/stats", get(get_stats))
        .route("/health", get(health))
}

// ── Health ──────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ── Events ──────────────────────────────────────────────

#[derive(Deserialize)]
struct ListEventsQuery {
    r#type: Option<String>,
    severity: Option<String>,
    unresolved: Option<bool>,
    source: Option<String>,
    limit: Option<usize>,
}

async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<ListEventsQuery>,
) -> Result<Json<Vec<Event>>, (StatusCode, String)> {
    let filter = EventFilter {
        event_type: q
            .r#type
            .as_deref()
            .map(|s| s.parse::<EventType>())
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?,
        severity: q
            .severity
            .as_deref()
            .map(|s| s.parse::<Severity>())
            .transpose()
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?,
        unresolved_only: q.unresolved.unwrap_or(false),
        source: q.source,
        limit: Some(q.limit.unwrap_or(100)),
        ..Default::default()
    };

    let store = state.store.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("lock error: {e}"))
    })?;
    let events = store.list(&filter).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("query error: {e}"))
    })?;
    Ok(Json(events))
}

#[derive(Deserialize)]
struct SubmitEventBody {
    r#type: String,
    title: String,
    severity: Option<String>,
    source: Option<String>,
    metadata: Option<serde_json::Value>,
}

async fn submit_event(
    State(state): State<AppState>,
    Json(body): Json<SubmitEventBody>,
) -> Result<(StatusCode, Json<Event>), (StatusCode, String)> {
    let event_type: EventType = body
        .r#type
        .parse()
        .map_err(|e: String| (StatusCode::BAD_REQUEST, e))?;
    let severity: Severity = body
        .severity
        .as_deref()
        .unwrap_or("medium")
        .parse()
        .map_err(|e: String| (StatusCode::BAD_REQUEST, e))?;

    let event = Event::new(
        event_type,
        body.title,
        severity,
        body.source.unwrap_or_else(|| "api".to_string()),
        body.metadata.unwrap_or(serde_json::json!({})),
    );

    let store = state.store.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("lock error: {e}"))
    })?;
    store.insert(&event).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("insert error: {e}"))
    })?;

    Ok((StatusCode::CREATED, Json(event)))
}

async fn get_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Event>, (StatusCode, String)> {
    let store = state.store.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("lock error: {e}"))
    })?;
    match store.get(&id) {
        Ok(Some(event)) => Ok(Json(event)),
        Ok(None) => Err((StatusCode::NOT_FOUND, format!("event not found: {id}"))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("query error: {e}"))),
    }
}

#[derive(Deserialize)]
struct ResolveBody {
    notes: Option<String>,
}

async fn resolve_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let store = state.store.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("lock error: {e}"))
    })?;
    store
        .resolve(&id, body.notes.as_deref().unwrap_or(""))
        .map_err(|e| (StatusCode::NOT_FOUND, format!("{e}")))?;

    Ok(Json(serde_json::json!({ "resolved": true, "id": id })))
}

// ── Rules ───────────────────────────────────────────────

async fn list_rules() -> Json<Vec<rules::Rule>> {
    Json(rules::default_rules())
}

// ── Stats ───────────────────────────────────────────────

async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<harness_core::events::Stats>, (StatusCode, String)> {
    let store = state.store.lock().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("lock error: {e}"))
    })?;
    let stats = store.stats(&EventFilter::default()).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("stats error: {e}"))
    })?;
    Ok(Json(stats))
}
