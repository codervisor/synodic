#!/usr/bin/env python3
"""simulate_governance.py — Generate synthetic governance logs for testing.

Produces realistic .governance.jsonl records that model harness evolution
over time. Useful for testing evaluate_harness.py and validating that the
scoring system responds correctly to improving/degrading governance.

Scenarios:
  healthy   — L1 catches more over time, repeat rate drops, rules get created
  degrading — L2/L3 load grows, same issues keep appearing, no crystallization
  cold-start — early runs with no rules, gradual improvement
  plateau   — initial improvement then stagnation

Usage:
  python3 simulate_governance.py <scenario> [--runs N] [--output PATH]

  scenario: healthy | degrading | cold-start | plateau
  --runs N: number of synthetic runs (default: 20)
  --output PATH: output file (default: stdout)
"""

import argparse
import json
import random
import sys
import time

# Feedback taxonomy from HARNESS.md §5
CODE_CATEGORIES = ["[completeness]", "[correctness]", "[security]", "[conformance]", "[quality]"]
DECOMPOSE_CATEGORIES = ["[orthogonality]", "[coverage]", "[granularity]", "[budget]"]
REUNIFY_CATEGORIES = ["[interface]", "[boundary]", "[redundancy]", "[gap]"]

DESCRIPTIONS = {
    "[completeness]": [
        "Missing error handling for empty input",
        "Endpoint does not return pagination metadata",
        "Config parser ignores unknown keys silently",
    ],
    "[correctness]": [
        "Off-by-one in loop boundary",
        "Race condition in concurrent map access",
        "Null dereference when optional field absent",
    ],
    "[security]": [
        "SQL query uses string interpolation instead of parameterized query",
        "API key hardcoded in source file",
        "User input passed to shell exec without sanitization",
    ],
    "[conformance]": [
        "Returns JSON but spec requires protobuf",
        "Uses polling when spec requires websocket push",
        "Implements sync API but spec says async",
    ],
    "[quality]": [
        "Function exceeds 200 lines, should be decomposed",
        "Naming inconsistency: mix of camelCase and snake_case",
        "Dead code: unreachable branch after early return",
    ],
    "[orthogonality]": [
        "Children 'auth' and 'permissions' have 45% scope overlap",
        "Children 'api' and 'routes' are essentially the same concern",
    ],
    "[coverage]": [
        "Parent spec mentions caching but no child covers it",
        "Logging requirement not allocated to any child",
    ],
    "[granularity]": [
        "Single child covers 80% of parent scope — decomposition adds no value",
        "7 children for a 20-line spec is too fine-grained",
    ],
    "[budget]": [
        "15/20 nodes used with 2 depth levels remaining",
    ],
    "[interface]": [
        "Component A expects JSON, component B emits CSV",
        "Return type mismatch: Promise<string> vs string",
    ],
    "[boundary]": [
        "Auth component also implements rate limiting (out of scope)",
    ],
    "[redundancy]": [
        "Both components implement their own HTTP client wrapper",
    ],
    "[gap]": [
        "No component handles the handoff between parsing and validation",
    ],
}


def pick_description(category: str) -> str:
    options = DESCRIPTIONS.get(category, [f"Issue in {category}"])
    return random.choice(options)


def generate_run(run_index: int, total_runs: int, scenario: str,
                 base_timestamp: float) -> dict:
    """Generate a single governance record based on scenario and position."""
    progress = run_index / max(total_runs - 1, 1)  # 0.0 → 1.0
    source = random.choice(["factory", "fractal"])
    ts = base_timestamp + run_index * 3600  # 1 hour apart
    work_id = f"{source}-{int(ts)}"

    record = {
        "work_id": work_id,
        "source": source,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(ts)),
        "rework_items": [],
        "decompose_flags": [],
        "reunify_conflicts": [],
        "static_failures": [],
        "metrics": {},
    }

    if scenario == "healthy":
        # L1 catches more over time, fewer L2 issues, rework drops
        l1_prob = 0.2 + progress * 0.6        # 20% → 80% L1 resolution
        issue_count = max(0, int(3 - progress * 2.5 + random.gauss(0, 0.5)))
        attempt_count = max(1, int(3 - progress * 2 + random.gauss(0, 0.3)))

        if random.random() < l1_prob:
            # Caught at L1
            record["static_failures"] = [f"rule-{random.randint(1,5)}"]
            record["status"] = "passed"
        elif issue_count > 0:
            _add_feedback(record, source, issue_count, progress)
            record["status"] = "passed" if random.random() < 0.7 + progress * 0.3 else "failed"
        else:
            record["status"] = "passed"

        record["metrics"]["attempt_count"] = attempt_count

    elif scenario == "degrading":
        # L2/L3 load grows, repeat issues, rework increases
        issue_count = int(1 + progress * 3 + random.gauss(0, 0.5))
        attempt_count = max(1, int(1 + progress * 2.5 + random.gauss(0, 0.3)))

        # Keep hitting the same categories
        _add_feedback(record, source, max(1, issue_count), progress,
                      repeat_bias=True)

        if random.random() < progress * 0.3:
            record["status"] = "escalated"
        elif random.random() < 0.4 + progress * 0.3:
            record["status"] = "failed"
        else:
            record["status"] = "passed"

        record["metrics"]["attempt_count"] = attempt_count

    elif scenario == "cold-start":
        # No rules early, gradual improvement as rules get added
        if progress < 0.3:
            # Early: lots of L2 issues, high rework
            issue_count = random.randint(2, 4)
            attempt_count = random.randint(2, 3)
            _add_feedback(record, source, issue_count, progress)
            record["status"] = "passed" if random.random() < 0.5 else "failed"
        elif progress < 0.6:
            # Mid: rules being created, L1 starts catching
            if random.random() < 0.4:
                record["static_failures"] = [f"rule-{random.randint(1,3)}"]
            issue_count = random.randint(1, 2)
            attempt_count = random.randint(1, 2)
            _add_feedback(record, source, issue_count, progress)
            record["status"] = "passed" if random.random() < 0.7 else "failed"
        else:
            # Late: mostly L1, few L2
            if random.random() < 0.7:
                record["static_failures"] = [f"rule-{random.randint(1,5)}"]
                record["status"] = "passed"
            else:
                issue_count = random.randint(0, 1)
                attempt_count = 1
                if issue_count:
                    _add_feedback(record, source, issue_count, progress)
                record["status"] = "passed"

            attempt_count = 1

        record["metrics"]["attempt_count"] = attempt_count

    elif scenario == "plateau":
        # Improves initially then flatlines
        effective_progress = min(progress * 2, 0.5)  # caps at 0.5
        l1_prob = 0.2 + effective_progress * 0.4
        issue_count = max(0, int(2 - effective_progress * 2 + random.gauss(0, 0.5)))
        attempt_count = max(1, int(2 - effective_progress + random.gauss(0, 0.3)))

        if random.random() < l1_prob:
            record["static_failures"] = [f"rule-{random.randint(1,3)}"]
            record["status"] = "passed"
        elif issue_count > 0:
            _add_feedback(record, source, issue_count, progress)
            record["status"] = "passed" if random.random() < 0.6 else "failed"
        else:
            record["status"] = "passed"

        record["metrics"]["attempt_count"] = attempt_count

    # Add timing metric
    record["metrics"]["time_to_pass_s"] = random.randint(60, 600)

    return record


def _add_feedback(record: dict, source: str, count: int, progress: float,
                  repeat_bias: bool = False):
    """Add feedback items to a record."""
    if source == "fractal" and random.random() < 0.3:
        # Decomposition feedback
        cats = DECOMPOSE_CATEGORIES
        for _ in range(min(count, 2)):
            cat = cats[0] if repeat_bias else random.choice(cats)
            record["decompose_flags"].append({
                "category": cat,
                "description": pick_description(cat),
            })
        if random.random() < 0.2:
            cat = random.choice(REUNIFY_CATEGORIES)
            record["reunify_conflicts"].append({
                "category": cat,
                "description": pick_description(cat),
            })
    else:
        # Code-producing feedback
        cats = CODE_CATEGORIES
        for _ in range(count):
            if repeat_bias:
                # Bias toward first 2 categories (simulating repeat issues)
                cat = random.choice(cats[:2])
            else:
                cat = random.choice(cats)
            record["rework_items"].append({
                "category": cat,
                "description": pick_description(cat),
            })


def main():
    parser = argparse.ArgumentParser(description="Generate synthetic governance logs")
    parser.add_argument("scenario", choices=["healthy", "degrading", "cold-start", "plateau"],
                        help="Evolution scenario")
    parser.add_argument("--runs", type=int, default=20, help="Number of runs")
    parser.add_argument("--output", type=str, default=None, help="Output file (default: stdout)")
    parser.add_argument("--seed", type=int, default=None, help="Random seed for reproducibility")
    args = parser.parse_args()

    if args.seed is not None:
        random.seed(args.seed)

    base_ts = 1710600000  # Approx 2024-03-16

    records = []
    for i in range(args.runs):
        rec = generate_run(i, args.runs, args.scenario, base_ts)
        records.append(rec)

    out = open(args.output, "w") if args.output else sys.stdout
    for rec in records:
        out.write(json.dumps(rec) + "\n")

    if args.output:
        out.close()
        print(f"Wrote {len(records)} records to {args.output}", file=sys.stderr)
    else:
        print(f"Generated {len(records)} records ({args.scenario} scenario)",
              file=sys.stderr)


if __name__ == "__main__":
    main()
