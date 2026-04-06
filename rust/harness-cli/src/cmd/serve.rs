//! HTTP server for the Synodic governance dashboard.
//!
//! Serves the React dashboard as static files and exposes a JSON API
//! for governance events, rules, and health checks.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, patch};
use axum::{Json, Router};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

use harness_core::storage::pool::{create_storage, resolve_database_url};
use harness_core::storage::{
    CreateGovernanceEvent, GovernanceEvent, GovernanceEventFilters, Rule, Storage,
};

/// Run the Synodic web server (dashboard + API)
#[derive(Parser)]
pub struct ServeCmd {
    /// Port to listen on (defaults to $PORT or 3000)
    #[arg(long, env = "PORT", default_value = "3000")]
    port: u16,

    /// Directory containing the built dashboard files
    #[arg(long, env = "SYNODIC_DASHBOARD_DIR")]
    dashboard_dir: Option<String>,
}

type AppState = Arc<dyn Storage>;

impl ServeCmd {
    pub async fn run(self) -> Result<()> {
        let db_url = resolve_database_url();
        eprintln!("Connecting to database...");
        let storage = create_storage(&db_url).await?;
        let state: AppState = Arc::from(storage);

        let api = Router::new()
            .route("/health", get(health))
            .route("/events", get(list_events).post(create_event))
            .route("/events/{id}", get(get_event))
            .route("/events/{id}/resolve", patch(resolve_event))
            .route("/stats", get(get_stats))
            .route("/rules", get(list_rules));

        let mut app = Router::new().nest("/api", api).with_state(state);

        // Serve dashboard static files if directory is configured and exists
        if let Some(ref dir) = self.dashboard_dir {
            let path = std::path::Path::new(dir);
            if path.is_dir() {
                eprintln!("Serving dashboard from {dir}");
                app = app.fallback_service(ServeDir::new(dir));
            } else {
                eprintln!("Dashboard directory {dir} not found, skipping static files");
            }
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        eprintln!("Synodic server listening on http://{addr}");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// API handlers
// ---------------------------------------------------------------------------

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn list_events(
    State(store): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<GovernanceEvent>>, AppError> {
    let filters = GovernanceEventFilters {
        event_type: params.get("type").cloned(),
    };
    let events = store.get_governance_events(filters).await?;
    Ok(Json(events))
}

async fn get_event(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GovernanceEvent>, AppError> {
    let event = store
        .get_governance_event(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("event not found".into()))?;
    Ok(Json(event))
}

async fn create_event(
    State(store): State<AppState>,
    Json(body): Json<CreateGovernanceEvent>,
) -> Result<(StatusCode, Json<GovernanceEvent>), AppError> {
    let event = store.create_governance_event(body).await?;
    Ok((StatusCode::CREATED, Json(event)))
}

#[derive(Deserialize)]
struct ResolveBody {
    notes: Option<String>,
}

async fn resolve_event(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> Result<StatusCode, AppError> {
    store.resolve_governance_event(&id, body.notes).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct Stats {
    total: usize,
    unresolved: usize,
    by_type: HashMap<String, usize>,
    by_severity: HashMap<String, usize>,
}

async fn get_stats(State(store): State<AppState>) -> Result<Json<Stats>, AppError> {
    let events = store
        .get_governance_events(GovernanceEventFilters::default())
        .await?;

    let total = events.len();
    let unresolved = events.iter().filter(|e| !e.resolved).count();

    let mut by_type: HashMap<String, usize> = HashMap::new();
    let mut by_severity: HashMap<String, usize> = HashMap::new();

    for e in &events {
        *by_type.entry(e.event_type.clone()).or_default() += 1;
        *by_severity.entry(e.severity.clone()).or_default() += 1;
    }

    Ok(Json(Stats {
        total,
        unresolved,
        by_type,
        by_severity,
    }))
}

#[derive(Serialize)]
struct ApiRule {
    name: String,
    description: String,
    pattern: String,
    event_type: String,
    severity: String,
    enabled: bool,
}

impl From<Rule> for ApiRule {
    fn from(r: Rule) -> Self {
        Self {
            name: r.id,
            description: r.description,
            pattern: r.condition_value,
            event_type: r.condition_type,
            severity: r.category_id,
            enabled: r.enabled,
        }
    }
}

async fn list_rules(State(store): State<AppState>) -> Result<Json<Vec<ApiRule>>, AppError> {
    let rules = store.get_rules(false).await?;
    let api_rules: Vec<ApiRule> = rules.into_iter().map(ApiRule::from).collect();
    Ok(Json(api_rules))
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

enum AppError {
    Internal(anyhow::Error),
    NotFound(String),
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Internal(err) => {
                eprintln!("Internal error: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "internal server error" })),
                )
                    .into_response()
            }
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
        }
    }
}
