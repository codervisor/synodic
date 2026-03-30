use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::process::Command;

use crate::util;

#[derive(Args)]
pub struct InitCmd {
    /// Project directory (default: current repo root)
    #[arg(long)]
    dir: Option<String>,

    /// Skip Claude Code hooks setup
    #[arg(long)]
    no_claude_hooks: bool,

    /// Skip git hooksPath configuration
    #[arg(long)]
    no_git_hooks: bool,
}

impl InitCmd {
    pub fn run(self) -> Result<()> {
        let root = match self.dir {
            Some(d) => PathBuf::from(d),
            None => util::find_repo_root()?,
        };

        // --- .harness/ infrastructure ---
        let harness_dir = root.join(".harness");
        std::fs::create_dir_all(&harness_dir)?;
        std::fs::create_dir_all(harness_dir.join(".runs"))?;

        let db_path = harness_dir.join("synodic.db");
        if !db_path.exists() {
            harness_core::storage::sqlite::SqliteStore::open(&db_path)?;
            eprintln!("Created database: {}", db_path.display());
        } else {
            eprintln!("Database already exists: {}", db_path.display());
        }

        let gates_path = harness_dir.join("gates.yml");
        if !gates_path.exists() {
            std::fs::write(&gates_path, "gates:\n  preflight: []\n")?;
            eprintln!("Created: {}", gates_path.display());
        }

        eprintln!("Initialized .harness/ at {}", harness_dir.display());

        // --- L1: Git hooks ---
        if !self.no_git_hooks {
            setup_git_hooks(&root)?;
        }

        // --- L2: Claude Code hooks ---
        if !self.no_claude_hooks {
            setup_claude_hooks(&root)?;
        }

        Ok(())
    }
}

/// Configure git to use .githooks/ for L1 governance (fmt, clippy, test).
fn setup_git_hooks(root: &PathBuf) -> Result<()> {
    let githooks_dir = root.join(".githooks");
    if !githooks_dir.exists() {
        eprintln!("No .githooks/ directory found, skipping git hooks setup");
        return Ok(());
    }

    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .current_dir(root)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run git config: {e}"))?;

    if status.success() {
        eprintln!("L1: git hooksPath → .githooks/");
    } else {
        eprintln!("Warning: failed to set git core.hooksPath");
    }

    Ok(())
}

/// Create .claude/settings.json and hook scripts for L2 interception.
fn setup_claude_hooks(root: &PathBuf) -> Result<()> {
    let claude_dir = root.join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    // Write intercept.sh
    let intercept_path = hooks_dir.join("intercept.sh");
    if !intercept_path.exists() {
        std::fs::write(&intercept_path, INTERCEPT_HOOK)?;
        #[cfg(unix)]
        set_executable(&intercept_path)?;
        eprintln!("L2: created {}", intercept_path.display());
    } else {
        eprintln!("L2: {} already exists, skipping", intercept_path.display());
    }

    // Write post-tool-log.sh
    let log_path = hooks_dir.join("post-tool-log.sh");
    if !log_path.exists() {
        std::fs::write(&log_path, POST_TOOL_LOG_HOOK)?;
        #[cfg(unix)]
        set_executable(&log_path)?;
        eprintln!("L2: created {}", log_path.display());
    } else {
        eprintln!("L2: {} already exists, skipping", log_path.display());
    }

    // Write settings.json
    let settings_path = claude_dir.join("settings.json");
    if !settings_path.exists() {
        std::fs::write(&settings_path, CLAUDE_SETTINGS)?;
        eprintln!("L2: created {}", settings_path.display());
    } else {
        eprintln!("L2: {} already exists, skipping", settings_path.display());
    }

    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

const INTERCEPT_HOOK: &str = r##"#!/usr/bin/env bash
# L2 Interception hook for Claude Code PreToolUse events.
#
# Reads tool call JSON from stdin, evaluates against Synodic's intercept
# rules, and returns the appropriate exit code + output for Claude Code.
#
# Exit 0 = allow, Exit 2 = block (with reason on stderr).

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
SYNODIC_BIN="${SYNODIC_BIN:-${PROJECT_DIR}/rust/target/release/synodic}"

# Fall back to debug build if release doesn't exist
if [[ ! -x "$SYNODIC_BIN" ]]; then
  SYNODIC_BIN="${PROJECT_DIR}/rust/target/debug/synodic"
fi

# If no binary, allow (don't block the agent on missing build)
if [[ ! -x "$SYNODIC_BIN" ]]; then
  exit 0
fi

# Read hook input from stdin
INPUT="$(cat)"

# Extract tool_name and tool_input from the hook's JSON payload
TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // empty')"
TOOL_INPUT="$(echo "$INPUT" | jq -c '.tool_input // {}')"

# If we couldn't parse the input, allow
if [[ -z "$TOOL_NAME" ]]; then
  exit 0
fi

# Call synodic intercept
RESULT="$("$SYNODIC_BIN" intercept --tool "$TOOL_NAME" --input "$TOOL_INPUT" 2>/dev/null)" || {
  # If the command fails, allow (fail-open)
  exit 0
}

DECISION="$(echo "$RESULT" | jq -r '.decision // "allow"')"

if [[ "$DECISION" == "block" ]]; then
  REASON="$(echo "$RESULT" | jq -r '.reason // "Blocked by Synodic governance rule"')"
  RULE="$(echo "$RESULT" | jq -r '.rule // "unknown"')"
  echo "Synodic L2 interception [$RULE]: $REASON" >&2
  exit 2
fi

exit 0
"##;

const POST_TOOL_LOG_HOOK: &str = r##"#!/usr/bin/env bash
# PostToolUse event logger for Synodic governance.
#
# Logs tool usage events to .harness/events.jsonl for post-session
# analysis and pattern detection (L2 audit trail).
#
# Runs async — does not block the agent.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
HARNESS_DIR="${PROJECT_DIR}/.harness"
LOG_FILE="${HARNESS_DIR}/events.jsonl"

# Ensure .harness directory exists
mkdir -p "$HARNESS_DIR"

# Read hook input from stdin
INPUT="$(cat)"

TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // empty')"

# Skip if we can't parse
if [[ -z "$TOOL_NAME" ]]; then
  exit 0
fi

# Build a lightweight log entry (no tool_result to keep logs small)
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // "unknown"')"

jq -n \
  --arg ts "$TIMESTAMP" \
  --arg tool "$TOOL_NAME" \
  --arg session "$SESSION_ID" \
  --arg event "tool_use" \
  '{timestamp: $ts, event: $event, tool: $tool, session_id: $session}' \
  >> "$LOG_FILE"

exit 0
"##;

const CLAUDE_SETTINGS: &str = r##"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/intercept.sh",
            "timeout": 5,
            "statusMessage": "Synodic L2 intercept check..."
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Bash|Write|Edit|Read",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post-tool-log.sh",
            "async": true,
            "timeout": 5
          }
        ]
      }
    ]
  }
}
"##;
