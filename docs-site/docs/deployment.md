---
sidebar_position: 8
---

# Deployment

Synodic can be self-hosted in several ways.

## Local development

```bash
# Build from source
cd rust && cargo build --release

# Initialize and start
./target/release/synodic init
./target/release/synodic serve
```

## Docker

```bash
# Build the image
docker build -f docker/Dockerfile -t synodic .

# Run with a persistent volume
docker run -p 3000:3000 -v synodic-data:/data synodic
```

The Docker image uses a multi-stage build:
1. **Build stage** — compiles the Rust binary
2. **Runtime stage** — minimal image with just the binary and dashboard

### Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `3000` |
| `DATABASE_URL` | PostgreSQL connection URL (optional) | SQLite |
| `SYNODIC_ROOT` | Project root directory | auto-detect |
| `SYNODIC_DASHBOARD_DIR` | Path to dashboard static files | auto-detect |

## PostgreSQL

By default, Synodic uses SQLite (zero-config). For team/org deployments, switch to PostgreSQL:

```bash
# Build with PostgreSQL support
cd rust && cargo build --release --features postgres

# Set the connection URL
export DATABASE_URL="postgres://user:pass@localhost:5432/synodic"

# Start the server
synodic serve
```

The PostgreSQL backend uses the same schema as SQLite with full-text search via `tsvector`.

## Fly.io

```bash
cd deploy
fly launch --config fly.toml
fly deploy
```

## Railway

Deploy from the repository using the `deploy/railway.json` configuration.

## Render

Use `deploy/render.yaml` as a Blueprint:

```bash
# Or deploy manually
render deploy --config deploy/render.yaml
```

## npm distribution

Synodic is distributed as an npm package with platform-specific Rust binaries:

```bash
npm install -g @codervisor/synodic
```

The npm wrapper handles:
- Downloading the correct binary for your platform
- Making the `synodic` command available globally
- Version management
