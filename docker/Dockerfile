# Stage 1: Build Rust binaries
FROM rust:1.83-bookworm AS rust-builder
WORKDIR /build
COPY rust/ rust/
RUN cd rust && cargo build --release

# Stage 2: Build dashboard
FROM node:22-bookworm-slim AS ui-builder
RUN corepack enable pnpm
WORKDIR /build
COPY packages/ui/package.json packages/ui/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile 2>/dev/null || pnpm install
COPY packages/ui/ .
RUN npx vite build

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates sqlite3 && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/rust/target/release/synodic /usr/local/bin/synodic
COPY --from=rust-builder /build/rust/target/release/synodic-http /usr/local/bin/synodic-http
COPY --from=ui-builder /build/dist /srv/dashboard

ENV SYNODIC_DASHBOARD_DIR=/srv/dashboard
EXPOSE 3000

ENTRYPOINT ["synodic-http"]
