#!/usr/bin/env bash
# report.sh — Generate a summary report from batch eval results
#
# Reads results from a batch run directory (produced by batch.sh) or
# from individual score reports in evals/results/, and outputs a
# formatted comparison table.
#
# Usage:
#   ./report.sh [batch-dir]           # Report from a specific batch
#   ./report.sh --all                 # Report across all batches
#   ./report.sh --latest              # Report from most recent batch
#   ./report.sh --compare a b         # Compare two batch runs side-by-side
#
# Output formats:
#   --format table    (default) Terminal table
#   --format json     Machine-readable JSON
#   --format csv      CSV for spreadsheet import
#
# Examples:
#   ./report.sh evals/results/batch-20260317-1430
#   ./report.sh --latest --format json > report.json
#   ./report.sh --compare batch-20260317-1430 batch-20260318-0900

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="${SCRIPT_DIR}/results"

# --- Argument parsing ---
MODE="single"
BATCH_DIR=""
FORMAT="table"
COMPARE_A=""
COMPARE_B=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all) MODE="all"; shift ;;
    --latest) MODE="latest"; shift ;;
    --compare) MODE="compare"; COMPARE_A="$2"; COMPARE_B="$3"; shift 3 ;;
    --format) FORMAT="$2"; shift 2 ;;
    -h|--help)
      head -25 "$0" | grep '^#' | sed 's/^# \?//'
      exit 0
      ;;
    *)
      if [[ -z "$BATCH_DIR" ]]; then
        BATCH_DIR="$1"
      else
        echo "Unknown option: $1" >&2; exit 1
      fi
      shift
      ;;
  esac
done

# Resolve batch directory
case "$MODE" in
  single)
    if [[ -z "$BATCH_DIR" ]]; then
      echo "Usage: report.sh <batch-dir> | --latest | --all" >&2
      exit 1
    fi
    # Allow relative names like "batch-20260317-1430"
    if [[ ! -d "$BATCH_DIR" ]] && [[ -d "${RESULTS_DIR}/${BATCH_DIR}" ]]; then
      BATCH_DIR="${RESULTS_DIR}/${BATCH_DIR}"
    fi
    if [[ ! -f "${BATCH_DIR}/manifest.json" ]]; then
      echo "ERROR: No manifest.json in ${BATCH_DIR}" >&2
      exit 1
    fi
    ;;
  latest)
    BATCH_DIR=$(ls -dt "${RESULTS_DIR}"/batch-* 2>/dev/null | head -1)
    if [[ -z "$BATCH_DIR" ]] || [[ ! -f "${BATCH_DIR}/manifest.json" ]]; then
      echo "ERROR: No batch results found in ${RESULTS_DIR}" >&2
      exit 1
    fi
    ;;
esac

# --- Report generation ---
python3 << 'PYEOF'
import json, sys, os, glob
from pathlib import Path

mode = os.environ.get("REPORT_MODE", "single")
fmt = os.environ.get("REPORT_FORMAT", "table")
batch_dir = os.environ.get("BATCH_DIR", "")
results_dir = os.environ.get("RESULTS_DIR", "")
compare_a = os.environ.get("COMPARE_A", "")
compare_b = os.environ.get("COMPARE_B", "")


def load_manifest(path):
    with open(os.path.join(path, "manifest.json")) as f:
        return json.load(f)


def format_table(manifest):
    """Print a terminal-formatted report."""
    summary = manifest.get("summary")
    if not summary:
        print("ERROR: Batch has no summary (still running?)")
        return

    config = manifest.get("config", {})
    batch_id = manifest.get("batch_id", "unknown")

    print(f"Batch:     {batch_id}")
    print(f"Started:   {manifest.get('started', '?')}")
    print(f"Completed: {manifest.get('completed', '?')}")
    print()

    # Overall stats
    print(f"Total runs:  {summary['total_runs']}")
    print(f"Resolved:    {summary['resolved']}")
    print(f"Failed:      {summary['failed']}")
    print(f"Errors:      {summary['errors']}")
    print()

    # Per-skill table
    skills = summary.get("per_skill", {})
    if skills:
        hdr = f"{'Skill':<10} {'Resolved':>8} {'Total':>6} {'Rate':>8} {'Avg Time':>10}"
        print(hdr)
        print("─" * len(hdr))
        for skill, stats in sorted(skills.items()):
            rate = f"{stats['resolve_rate']*100:.1f}%"
            avg_t = f"{stats['avg_duration_s']:.0f}s"
            print(f"{skill:<10} {stats['resolved']:>8} {stats['total']:>6} {rate:>8} {avg_t:>10}")
        print()

    # Per-task × skill matrix
    runs = manifest.get("runs", [])
    if runs:
        tasks = sorted(set(r["task"] for r in runs))
        skill_names = sorted(set(r["skill"] for r in runs))

        # Build lookup
        lookup = {}
        for r in runs:
            lookup[(r["task"], r["skill"])] = r

        # Header
        col_w = max(10, max(len(s) for s in skill_names) + 2)
        task_w = max(30, max(len(t) for t in tasks) + 2)
        header = f"{'Task':<{task_w}}" + "".join(f"{s:>{col_w}}" for s in skill_names)
        print(header)
        print("─" * len(header))

        for task in tasks:
            row = f"{task:<{task_w}}"
            for skill in skill_names:
                r = lookup.get((task, skill))
                if r is None:
                    cell = "—"
                elif r.get("resolved"):
                    dur = r.get("duration_s", 0)
                    cell = f"OK ({dur}s)"
                elif r.get("exit_code", 0) != 0:
                    cell = "ERR"
                else:
                    cell = "FAIL"
                row += f"{cell:>{col_w}}"
            print(row)
        print()

    # Skill deltas
    deltas = summary.get("skill_deltas", {})
    if deltas:
        print("Harness Impact (vs baseline):")
        print()
        for skill, delta in sorted(deltas.items()):
            sign = "+" if delta["net_delta"] >= 0 else ""
            print(f"  {skill}: {sign}{delta['net_delta']} net")
            if delta["uplift_tasks"]:
                print(f"    Uplift (+): {', '.join(delta['uplift_tasks'])}")
            if delta["regression_tasks"]:
                print(f"    Regress (-): {', '.join(delta['regression_tasks'])}")
        print()


def format_json(manifest):
    """Print machine-readable JSON."""
    json.dump(manifest.get("summary", manifest), sys.stdout, indent=2)
    print()


def format_csv(manifest):
    """Print CSV for spreadsheet import."""
    runs = manifest.get("runs", [])
    print("task,skill,resolved,duration_s,exit_code")
    for r in runs:
        print(f"{r['task']},{r['skill']},{r.get('resolved', False)},{r.get('duration_s', 0)},{r.get('exit_code', 0)}")


def format_compare(manifest_a, manifest_b):
    """Compare two batch runs side-by-side."""
    id_a = manifest_a.get("batch_id", "A")
    id_b = manifest_b.get("batch_id", "B")

    print(f"Comparing: {id_a} vs {id_b}")
    print()

    sum_a = manifest_a.get("summary", {})
    sum_b = manifest_b.get("summary", {})

    skills_a = sum_a.get("per_skill", {})
    skills_b = sum_b.get("per_skill", {})

    all_skills = sorted(set(list(skills_a.keys()) + list(skills_b.keys())))

    hdr = f"{'Skill':<10} {'Rate (A)':>10} {'Rate (B)':>10} {'Delta':>8}"
    print(hdr)
    print("─" * len(hdr))
    for skill in all_skills:
        rate_a = skills_a.get(skill, {}).get("resolve_rate", 0) * 100
        rate_b = skills_b.get(skill, {}).get("resolve_rate", 0) * 100
        delta = rate_b - rate_a
        sign = "+" if delta >= 0 else ""
        print(f"{skill:<10} {rate_a:>9.1f}% {rate_b:>9.1f}% {sign}{delta:>6.1f}%")
    print()

    # Per-task comparison
    runs_a = {(r["task"], r["skill"]): r for r in manifest_a.get("runs", [])}
    runs_b = {(r["task"], r["skill"]): r for r in manifest_b.get("runs", [])}
    all_keys = sorted(set(list(runs_a.keys()) + list(runs_b.keys())))

    changes = []
    for key in all_keys:
        ra = runs_a.get(key, {}).get("resolved", False)
        rb = runs_b.get(key, {}).get("resolved", False)
        if ra != rb:
            direction = "GAINED" if rb and not ra else "LOST"
            changes.append((key[0], key[1], direction))

    if changes:
        print("Changes:")
        for task, skill, direction in changes:
            marker = "+" if direction == "GAINED" else "-"
            print(f"  {marker} {task} ({skill}): {direction}")
    else:
        print("No changes in resolve status between runs.")
    print()


# --- Main ---
if mode == "single" or mode == "latest":
    manifest = load_manifest(batch_dir)
    if fmt == "table":
        format_table(manifest)
    elif fmt == "json":
        format_json(manifest)
    elif fmt == "csv":
        format_csv(manifest)

elif mode == "all":
    batch_dirs = sorted(glob.glob(os.path.join(results_dir, "batch-*")))
    for bd in batch_dirs:
        mf = os.path.join(bd, "manifest.json")
        if os.path.exists(mf):
            manifest = load_manifest(bd)
            summary = manifest.get("summary", {})
            per_skill = summary.get("per_skill", {})
            rates = " | ".join(
                f"{s}: {st['resolve_rate']*100:.0f}%"
                for s, st in sorted(per_skill.items())
            )
            print(f"{manifest.get('batch_id', bd):<30} {rates}")
    print()

elif mode == "compare":
    dir_a = compare_a if os.path.isdir(compare_a) else os.path.join(results_dir, compare_a)
    dir_b = compare_b if os.path.isdir(compare_b) else os.path.join(results_dir, compare_b)
    manifest_a = load_manifest(dir_a)
    manifest_b = load_manifest(dir_b)
    format_compare(manifest_a, manifest_b)

PYEOF
