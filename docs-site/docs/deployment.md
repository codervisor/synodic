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

Railway deploys from the repository using `deploy/railway.json`.

### Prerequisites

- A [Railway](https://railway.app) account
- The Railway CLI installed: `npm install -g @railway/cli`

### SQLite deployment (single instance)

1. **Create the project:**

   ```bash
   railway login
   railway init
   ```

2. **Add a persistent volume** (required — data is lost on redeploy without this):

   ```bash
   railway volume add --mount /data
   ```

3. **Set environment variables:**

   ```bash
   railway variables set DATABASE_URL=/data/synodic.db
   ```

4. **Deploy:**

   ```bash
   cd deploy
   railway up --config railway.json
   ```

5. **Verify** the health endpoint:

   ```bash
   curl https://<your-app>.up.railway.app/api/health
   # {"status":"ok"}
   ```

### PostgreSQL deployment

For team deployments, use Railway's managed PostgreSQL instead of SQLite:

1. **Create the project and add a PostgreSQL plugin:**

   ```bash
   railway login
   railway init
   railway add --plugin postgresql
   ```

2. **Link the DATABASE_URL** — Railway automatically provides `DATABASE_URL` when a PostgreSQL plugin is attached. No manual variable configuration needed.

3. **Deploy:**

   ```bash
   cd deploy
   railway up --config railway.json
   ```

   The entrypoint script auto-detects the `postgres://` URL and uses the PostgreSQL backend.

### Important notes

- **Volumes:** Railway volumes are configured via the dashboard or CLI, not in `railway.json`. Without a volume, SQLite data is ephemeral and will be lost on each deploy.
- **Single replica:** The config sets `numReplicas: 1`. Do not increase this when using SQLite, as concurrent writes from multiple replicas will corrupt the database. PostgreSQL deployments can safely scale to multiple replicas.
- **Port:** Railway injects the `PORT` environment variable automatically. The Synodic HTTP server reads `PORT` and defaults to 3000 if unset.
- **HTTPS:** Railway provides HTTPS termination by default on all public domains.

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
