mod routes;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_path = resolve_db_path()?;
    let app_state = state::AppState::new(&db_path)?;

    let app = Router::new()
        .nest("/api", routes::api_router())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    eprintln!("Synodic governance dashboard: http://{addr}");
    eprintln!("API: http://{addr}/api");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_db_path() -> anyhow::Result<PathBuf> {
    // Check DATABASE_URL env var first
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Ok(PathBuf::from(url));
    }
    // Check SYNODIC_ROOT
    if let Ok(root) = std::env::var("SYNODIC_ROOT") {
        let p = PathBuf::from(root).join(".harness").join("synodic.db");
        if p.exists() {
            return Ok(p);
        }
    }
    // Walk up from CWD
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".harness").join("synodic.db");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("synodic.db not found. Run `synodic init` first.")
}
