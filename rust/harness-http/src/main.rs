mod routes;
mod state;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = resolve_database_url()?;
    let app_state = state::AppState::new(&database_url)?;

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let mut app = Router::new()
        .nest("/api", routes::api_router())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Serve dashboard static files if available
    if let Some(dashboard_dir) = resolve_dashboard_dir() {
        let index = dashboard_dir.join("index.html");
        if index.exists() {
            app =
                app.fallback_service(ServeDir::new(&dashboard_dir).fallback(ServeFile::new(index)));
            eprintln!("Dashboard: serving from {}", dashboard_dir.display());
        }
    }

    let host: [u8; 4] = if std::env::var("SYNODIC_BIND_ALL").is_ok()
        || std::env::var("RAILWAY_ENVIRONMENT").is_ok()
        || std::env::var("FLY_APP_NAME").is_ok()
        || std::env::var("RENDER").is_ok()
    {
        [0, 0, 0, 0]
    } else {
        [127, 0, 0, 1]
    };
    let addr = SocketAddr::from((host, port));
    eprintln!("Synodic governance API: http://{addr}/api");
    eprintln!("Dashboard: http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_database_url() -> anyhow::Result<String> {
    // Explicit DATABASE_URL — supports both postgres:// and file paths
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Ok(url);
    }
    // Search for local SQLite database
    if let Ok(root) = std::env::var("SYNODIC_ROOT") {
        let p = PathBuf::from(root).join(".harness").join("synodic.db");
        if p.exists() {
            return Ok(p.to_string_lossy().to_string());
        }
    }
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".harness").join("synodic.db");
        if candidate.exists() {
            return Ok(candidate.to_string_lossy().to_string());
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("DATABASE_URL not set and synodic.db not found. Run `synodic init` or set DATABASE_URL=postgres://...")
}

/// Resolve path to dashboard dist/ directory.
fn resolve_dashboard_dir() -> Option<PathBuf> {
    // Explicit env var (used in Docker)
    if let Ok(dir) = std::env::var("SYNODIC_DASHBOARD_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }
    // Relative to binary location (dev/install)
    if let Ok(exe) = std::env::current_exe() {
        // dev: rust/target/debug/synodic-http → ../../packages/ui/dist
        if let Some(root) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let ui_dist = root.join("packages").join("ui").join("dist");
            if ui_dist.is_dir() {
                return Some(ui_dist);
            }
        }
    }
    // CWD-relative
    let cwd_dist = PathBuf::from("packages/ui/dist");
    if cwd_dist.is_dir() {
        return Some(cwd_dist);
    }
    None
}
