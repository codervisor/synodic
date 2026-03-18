#!/usr/bin/env bash
# .claude/setup-env.sh — SessionStart hook for Claude Code Cloud
#
# Installs tools not present in the default cloud container image.
# Runs once at the start of each session.
#
# Pre-installed in the image (no action needed):
#   git, docker, docker-compose, docker-buildx, curl, wget, jq
#   node 22, pnpm, npm, bun, cargo/rustc, python 3.10-3.13, go, ruby, java
#
# This script installs:
#   gh (GitHub CLI) — for actions, PRs, issues, releases, job logs

set -euo pipefail

# --- GitHub CLI ---
if ! command -v gh &>/dev/null; then
  echo "[setup-env] Installing GitHub CLI..."
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list > /dev/null
  apt-get update -qq && apt-get install -y -qq gh > /dev/null 2>&1
  echo "[setup-env] gh installed: $(gh --version | head -1)"
else
  echo "[setup-env] gh already installed: $(gh --version | head -1)"
fi

# --- gh auth ---
# GH_TOKEN must be set in your Claude Code env vars (settings or org config).
# Without it, gh works for public repos only (unauthenticated, 60 req/hr).
# With it, you get 5000 req/hr + private repo access + write operations.
if [ -n "${GH_TOKEN:-}" ]; then
  echo "[setup-env] GH_TOKEN is set — gh is authenticated"
else
  echo "[setup-env] WARNING: GH_TOKEN not set — gh limited to public/unauthenticated access"
  echo "[setup-env]   Set GH_TOKEN in Claude Code settings for full GitHub API access"
fi

echo "[setup-env] Environment ready."
