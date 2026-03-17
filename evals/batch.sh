#!/usr/bin/env bash
# batch.sh — Run evals across multiple tasks and skills, collect results
#
# Evaluates the harness as a whole by running benchmark tasks through
# different skills (fractal, factory, baseline) and aggregating results.
#
# Usage:
#   ./batch.sh [options]
#
# Options:
#   --tasks <list>       Comma-separated task aliases (default: all e2e tasks from evals.json)
#   --skills <list>      Comma-separated skills to compare (default: fractal,factory,baseline)
#   --benchmark <name>   Filter to one benchmark: swebench, featurebench, devbench
#   --split <split>      SWE-bench split (default: pro)
#   --output-dir <path>  Directory for results (default: evals/results/batch-<timestamp>)
#   --agent-cmd <cmd>    Agent command (default: claude)
#   --dry-run            Print what would run without executing
#   --skip-setup         Skip testbed setup (assume already done)
#   --parallel <n>       Max parallel runs (default: 1 — sequential)
#   --resume             Resume from last incomplete batch in output-dir
#
# Examples:
#   # Full harness eval: all tasks × all skills
#   ./batch.sh
#
#   # Compare skills on SWE-bench Pro only
#   ./batch.sh --benchmark swebench --split pro
#
#   # Specific tasks, specific skills
#   ./batch.sh --tasks "swe:django-16379,fb:mlflow-tracing" --skills "fractal,baseline"
#
#   # Dry run to see what would execute
#   ./batch.sh --dry-run
#
#   # Resume an interrupted batch
#   ./batch.sh --output-dir evals/results/batch-20260317-1430 --resume

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Defaults ---
TASKS=""
SKILLS="fractal,factory,baseline"
BENCHMARK_FILTER=""
SWE_SPLIT="pro"
OUTPUT_DIR=""
AGENT_CMD="claude"
DRY_RUN=false
SKIP_SETUP=false
PARALLEL=1
RESUME=false

# --- Argument parsing ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tasks) TASKS="$2"; shift 2 ;;
    --skills) SKILLS="$2"; shift 2 ;;
    --benchmark) BENCHMARK_FILTER="$2"; shift 2 ;;
    --split) SWE_SPLIT="$2"; shift 2 ;;
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    --agent-cmd) AGENT_CMD="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --skip-setup) SKIP_SETUP=true; shift ;;
    --parallel) PARALLEL="$2"; shift 2 ;;
    --resume) RESUME=true; shift ;;
    -h|--help)
      head -40 "$0" | grep '^#' | sed 's/^# \?//'
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# --- Output directory ---
if [[ -z "$OUTPUT_DIR" ]]; then
  TIMESTAMP=$(date +%Y%m%d-%H%M)
  OUTPUT_DIR="${SCRIPT_DIR}/results/batch-${TIMESTAMP}"
fi
mkdir -p "$OUTPUT_DIR"

# --- Resolve task list ---
resolve_tasks() {
  if [[ -n "$TASKS" ]]; then
    # User-specified task list
    echo "$TASKS" | tr ',' '\n'
    return
  fi

  # Extract e2e-capable tasks from evals.json
  python3 -c "
import json, sys

with open('${SCRIPT_DIR}/evals.json') as f:
    data = json.load(f)

benchmark_filter = '${BENCHMARK_FILTER}'

for ev in data['evals']:
    # Only tasks with e2e runner config
    if 'e2e' not in ev:
        continue

    alias = ev['e2e'].get('alias', '')
    if not alias:
        continue

    # Apply benchmark filter
    if benchmark_filter:
        tags = ev.get('tags', [])
        if benchmark_filter == 'swebench' and 'swebench' not in tags:
            continue
        if benchmark_filter == 'featurebench' and 'featurebench' not in tags:
            continue
        if benchmark_filter == 'devbench' and 'devbench' not in tags:
            continue

    print(alias)
"
}

TASK_LIST=()
while IFS= read -r task; do
  [[ -n "$task" ]] && TASK_LIST+=("$task")
done < <(resolve_tasks)

SKILL_LIST=()
IFS=',' read -ra SKILL_LIST <<< "$SKILLS"

TOTAL_RUNS=$(( ${#TASK_LIST[@]} * ${#SKILL_LIST[@]} ))

# --- Header ---
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║          Batch Eval — Synodic Harness                       ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Tasks:      ${#TASK_LIST[@]}"
echo "Skills:     ${SKILL_LIST[*]}"
echo "Total runs: ${TOTAL_RUNS}"
echo "Output:     ${OUTPUT_DIR}"
echo "Agent:      ${AGENT_CMD}"
echo ""

if [[ ${#TASK_LIST[@]} -eq 0 ]]; then
  echo "ERROR: No tasks matched. Check --tasks or --benchmark filter." >&2
  exit 1
fi

# --- Print run matrix ---
echo "┌─────────────────────────────────────────┬──────────┬──────────┐"
printf "│ %-39s │ %-8s │ %-8s │\n" "Task" "Skill" "Status"
echo "├─────────────────────────────────────────┼──────────┼──────────┤"
for task in "${TASK_LIST[@]}"; do
  for skill in "${SKILL_LIST[@]}"; do
    # Check if result already exists (for --resume)
    RESULT_FILE="${OUTPUT_DIR}/${task//[:\/]/_}_${skill}.json"
    if [[ "$RESUME" == "true" ]] && [[ -f "$RESULT_FILE" ]]; then
      STATUS="done"
    else
      STATUS="pending"
    fi
    printf "│ %-39s │ %-8s │ %-8s │\n" "$task" "$skill" "$STATUS"
  done
done
echo "└─────────────────────────────────────────┴──────────┴──────────┘"
echo ""

if [[ "$DRY_RUN" == "true" ]]; then
  echo "DRY RUN — commands that would execute:"
  echo ""
  for task in "${TASK_LIST[@]}"; do
    for skill in "${SKILL_LIST[@]}"; do
      EXTRA_ARGS=""
      if [[ "$task" == swe:* ]]; then
        EXTRA_ARGS="--split ${SWE_SPLIT}"
      fi
      if [[ "$SKIP_SETUP" == "true" ]]; then
        EXTRA_ARGS="${EXTRA_ARGS} --skip-setup"
      fi
      echo "  ${SCRIPT_DIR}/run.sh ${task} --skill ${skill} --agent-cmd ${AGENT_CMD} ${EXTRA_ARGS}"
    done
  done
  echo ""
  echo "To execute: re-run without --dry-run"
  exit 0
fi

# --- Run matrix ---
RUN_INDEX=0
PASSED=0
FAILED=0
SKIPPED=0
ERRORS=0

# Manifest tracks all runs
MANIFEST="${OUTPUT_DIR}/manifest.json"

# Initialize manifest
python3 -c "
import json, datetime
manifest = {
    'batch_id': '$(basename "$OUTPUT_DIR")',
    'started': datetime.datetime.utcnow().isoformat() + 'Z',
    'config': {
        'tasks': $(python3 -c "import json; print(json.dumps([$(printf '"%s",' "${TASK_LIST[@]}" | sed 's/,$//')]))" ),
        'skills': $(python3 -c "import json; print(json.dumps([$(printf '"%s",' "${SKILL_LIST[@]}" | sed 's/,$//')]))" ),
        'swe_split': '${SWE_SPLIT}',
        'agent_cmd': '${AGENT_CMD}'
    },
    'runs': [],
    'summary': None
}
with open('${MANIFEST}', 'w') as f:
    json.dump(manifest, f, indent=2)
"

run_one() {
  local task="$1"
  local skill="$2"
  local run_id="${task//[:\/]/_}_${skill}"
  local result_file="${OUTPUT_DIR}/${run_id}.json"
  local log_file="${OUTPUT_DIR}/${run_id}.log"

  # Skip if resuming and result exists
  if [[ "$RESUME" == "true" ]] && [[ -f "$result_file" ]]; then
    echo "  SKIP (already completed)"
    return 0
  fi

  local extra_args=()
  if [[ "$task" == swe:* ]]; then
    extra_args+=(--split "$SWE_SPLIT")
  fi
  if [[ "$SKIP_SETUP" == "true" ]]; then
    extra_args+=(--skip-setup)
  fi

  # Run and capture output
  local start_time
  start_time=$(date +%s)

  local exit_code=0
  "${SCRIPT_DIR}/run.sh" "$task" \
    --skill "$skill" \
    --agent-cmd "$AGENT_CMD" \
    --output "$result_file" \
    "${extra_args[@]}" \
    > "$log_file" 2>&1 || exit_code=$?

  local end_time
  end_time=$(date +%s)
  local duration=$(( end_time - start_time ))

  # Extract resolved status from score report
  local resolved="false"
  if [[ -f "$result_file" ]]; then
    resolved=$(python3 -c "import json; print(str(json.load(open('${result_file}')).get('resolved', False)).lower())" 2>/dev/null || echo "false")
  fi

  # Append to manifest
  python3 -c "
import json, datetime
with open('${MANIFEST}') as f:
    manifest = json.load(f)
manifest['runs'].append({
    'run_id': '${run_id}',
    'task': '${task}',
    'skill': '${skill}',
    'resolved': ${resolved},
    'exit_code': ${exit_code},
    'duration_s': ${duration},
    'result_file': '${result_file}',
    'log_file': '${log_file}',
    'timestamp': datetime.datetime.utcnow().isoformat() + 'Z'
})
with open('${MANIFEST}', 'w') as f:
    json.dump(manifest, f, indent=2)
"

  return $exit_code
}

for task in "${TASK_LIST[@]}"; do
  for skill in "${SKILL_LIST[@]}"; do
    RUN_INDEX=$((RUN_INDEX + 1))
    echo "━━━ Run ${RUN_INDEX}/${TOTAL_RUNS}: ${task} × ${skill} ━━━"

    RESULT_FILE="${OUTPUT_DIR}/${task//[:\/]/_}_${skill}.json"

    if [[ "$RESUME" == "true" ]] && [[ -f "$RESULT_FILE" ]]; then
      echo "  SKIP (already completed)"
      SKIPPED=$((SKIPPED + 1))
      echo ""
      continue
    fi

    if run_one "$task" "$skill"; then
      # Check if resolved
      if [[ -f "$RESULT_FILE" ]]; then
        RESOLVED=$(python3 -c "import json; print(json.load(open('${RESULT_FILE}')).get('resolved', False))" 2>/dev/null || echo "False")
        if [[ "$RESOLVED" == "True" ]]; then
          echo "  RESOLVED"
          PASSED=$((PASSED + 1))
        else
          echo "  NOT RESOLVED"
          FAILED=$((FAILED + 1))
        fi
      else
        echo "  NO SCORE REPORT"
        ERRORS=$((ERRORS + 1))
      fi
    else
      echo "  ERROR (exit code $?)"
      ERRORS=$((ERRORS + 1))
    fi
    echo ""
  done
done

# --- Finalize manifest with summary ---
python3 -c "
import json, datetime

with open('${MANIFEST}') as f:
    manifest = json.load(f)

runs = manifest['runs']
skills = list(set(r['skill'] for r in runs))
tasks = list(set(r['task'] for r in runs))

# Per-skill resolve rates
skill_stats = {}
for skill in skills:
    skill_runs = [r for r in runs if r['skill'] == skill]
    resolved = sum(1 for r in skill_runs if r.get('resolved'))
    total = len(skill_runs)
    skill_stats[skill] = {
        'resolved': resolved,
        'total': total,
        'resolve_rate': round(resolved / total, 3) if total > 0 else 0,
        'avg_duration_s': round(sum(r.get('duration_s', 0) for r in skill_runs) / max(len(skill_runs), 1), 1)
    }

# Skill deltas (vs baseline)
baseline_resolved = set()
if 'baseline' in skill_stats:
    baseline_resolved = set(r['task'] for r in runs if r['skill'] == 'baseline' and r.get('resolved'))

skill_deltas = {}
for skill in skills:
    if skill == 'baseline':
        continue
    skill_resolved = set(r['task'] for r in runs if r['skill'] == skill and r.get('resolved'))
    # Tasks this skill resolved that baseline didn't
    uplift = skill_resolved - baseline_resolved
    # Tasks baseline resolved that this skill didn't (regressions)
    regressions = baseline_resolved - skill_resolved
    skill_deltas[skill] = {
        'uplift_tasks': sorted(list(uplift)),
        'regression_tasks': sorted(list(regressions)),
        'net_delta': len(uplift) - len(regressions)
    }

manifest['completed'] = datetime.datetime.utcnow().isoformat() + 'Z'
manifest['summary'] = {
    'total_runs': len(runs),
    'resolved': sum(1 for r in runs if r.get('resolved')),
    'failed': sum(1 for r in runs if not r.get('resolved') and r.get('exit_code', 1) == 0),
    'errors': sum(1 for r in runs if r.get('exit_code', 0) != 0),
    'per_skill': skill_stats,
    'skill_deltas': skill_deltas
}

with open('${MANIFEST}', 'w') as f:
    json.dump(manifest, f, indent=2)
"

# --- Print summary ---
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║          Batch Results                                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

python3 -c "
import json

with open('${MANIFEST}') as f:
    manifest = json.load(f)

summary = manifest['summary']
print(f'Total runs:  {summary[\"total_runs\"]}')
print(f'Resolved:    {summary[\"resolved\"]}')
print(f'Failed:      {summary[\"failed\"]}')
print(f'Errors:      {summary[\"errors\"]}')
print()

# Per-skill table
print('┌──────────┬──────────┬──────────┬───────────┬──────────────┐')
print(f'│ {\"Skill\":<8} │ {\"Resolved\":<8} │ {\"Total\":<8} │ {\"Rate\":<9} │ {\"Avg Time (s)\":<12} │')
print('├──────────┼──────────┼──────────┼───────────┼──────────────┤')
for skill, stats in summary['per_skill'].items():
    rate_pct = f'{stats[\"resolve_rate\"]*100:.1f}%'
    print(f'│ {skill:<8} │ {stats[\"resolved\"]:>8} │ {stats[\"total\"]:>8} │ {rate_pct:>9} │ {stats[\"avg_duration_s\"]:>12} │')
print('└──────────┴──────────┴──────────┴───────────┴──────────────┘')
print()

# Skill deltas
if summary.get('skill_deltas'):
    print('Skill deltas (vs baseline):')
    for skill, delta in summary['skill_deltas'].items():
        sign = '+' if delta['net_delta'] >= 0 else ''
        print(f'  {skill}: {sign}{delta[\"net_delta\"]} net ({len(delta[\"uplift_tasks\"])} uplift, {len(delta[\"regression_tasks\"])} regressions)')
        if delta['uplift_tasks']:
            for t in delta['uplift_tasks']:
                print(f'    + {t}')
        if delta['regression_tasks']:
            for t in delta['regression_tasks']:
                print(f'    - {t}')
    print()
"

echo ""
echo "Full results: ${OUTPUT_DIR}/"
echo "Manifest:     ${MANIFEST}"
echo ""
echo "To generate a report from these results:"
echo "  ./evals/report.sh ${OUTPUT_DIR}"
