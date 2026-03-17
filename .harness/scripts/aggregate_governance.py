#!/usr/bin/env python3
"""aggregate_governance.py — Aggregate governance logs for crystallization analysis.

Reads *.governance.jsonl files from the .harness/ directory (or .factory/ / .fractal/
as fallback), aggregates feedback items by category, and reports patterns that appear
in ≥3 runs as "crystallization candidates."

Usage:
  python3 aggregate_governance.py [harness-dir]

  harness-dir: path to .harness/ directory (default: .harness/ in repo root)
"""

import glob
import json
import os
import sys
from collections import Counter, defaultdict


def find_governance_logs(harness_dir: str) -> list[str]:
    """Find all governance.jsonl files."""
    paths = []

    # Primary: .harness/*.governance.jsonl
    paths.extend(glob.glob(os.path.join(harness_dir, "*.governance.jsonl")))

    # Fallback: .factory/governance.jsonl, .fractal/governance.jsonl
    repo_root = os.path.dirname(harness_dir) if harness_dir.endswith(".harness") else harness_dir
    for topology in ("factory", "fractal"):
        fallback = os.path.join(repo_root, f".{topology}", "governance.jsonl")
        if os.path.isfile(fallback):
            paths.append(fallback)

    return list(set(paths))


def load_records(paths: list[str]) -> list[dict]:
    """Load all JSON lines from governance log files."""
    records = []
    for path in paths:
        with open(path) as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    records.append(json.loads(line))
                except json.JSONDecodeError:
                    print(f"  warning: skipping malformed line {line_num} in {path}",
                          file=sys.stderr)
    return records


def aggregate(records: list[dict]) -> dict:
    """Aggregate governance records into a summary report."""
    total_runs = len(records)
    status_counts = Counter()
    category_counts = Counter()
    description_patterns = defaultdict(list)

    for rec in records:
        source = rec.get("source", rec.get("work_id", "unknown").split("-")[0])
        status_counts[rec.get("status", "unknown")] += 1

        # Factory rework items
        for item in rec.get("rework_items", []):
            cat = item.get("category", "uncategorized")
            desc = item.get("description", "")
            category_counts[cat] += 1
            description_patterns[cat].append({
                "work_id": rec.get("work_id", "?"),
                "source": source,
                "description": desc,
            })

        # Fractal decompose flags
        for flag in rec.get("decompose_flags", []):
            cat = flag.get("category", "uncategorized")
            category_counts[cat] += 1
            description_patterns[cat].append({
                "work_id": rec.get("work_id", "?"),
                "source": source,
                "description": flag.get("description", ""),
            })

        # Fractal reunify conflicts
        for conflict in rec.get("reunify_conflicts", []):
            cat = conflict.get("category", "uncategorized")
            category_counts[cat] += 1
            description_patterns[cat].append({
                "work_id": rec.get("work_id", "?"),
                "source": source,
                "description": conflict.get("description", ""),
            })

        # Static failures (flat strings)
        for failure in rec.get("static_failures", []):
            category_counts["static_failure"] += 1

    # Identify crystallization candidates (>=3 occurrences)
    candidates = []
    for cat, count in category_counts.items():
        if count >= 3:
            unique_runs = len(set(p["work_id"] for p in description_patterns.get(cat, [])))
            candidates.append({
                "category": cat,
                "total_occurrences": count,
                "unique_runs": unique_runs,
                "sample_descriptions": [
                    p["description"] for p in description_patterns.get(cat, [])[:3]
                ],
            })

    candidates.sort(key=lambda c: c["total_occurrences"], reverse=True)

    # Trend analysis: split records into halves, compare category frequency
    trend = {}
    if len(records) >= 4:
        mid = len(records) // 2
        early_cats = Counter()
        late_cats = Counter()
        for rec in records[:mid]:
            for item in rec.get("rework_items", []):
                early_cats[item.get("category", "?")] += 1
            for flag in rec.get("decompose_flags", []):
                early_cats[flag.get("category", "?")] += 1
            for conflict in rec.get("reunify_conflicts", []):
                early_cats[conflict.get("category", "?")] += 1
        for rec in records[mid:]:
            for item in rec.get("rework_items", []):
                late_cats[item.get("category", "?")] += 1
            for flag in rec.get("decompose_flags", []):
                late_cats[flag.get("category", "?")] += 1
            for conflict in rec.get("reunify_conflicts", []):
                late_cats[conflict.get("category", "?")] += 1

        all_cats = set(early_cats) | set(late_cats)
        for cat in all_cats:
            e = early_cats.get(cat, 0) / max(mid, 1)
            l = late_cats.get(cat, 0) / max(len(records) - mid, 1)
            if l < e - 0.1:
                direction = "declining"
            elif l > e + 0.1:
                direction = "rising"
            else:
                direction = "stable"
            trend[cat] = {
                "early_rate": round(e, 3),
                "late_rate": round(l, 3),
                "direction": direction,
            }

    return {
        "total_runs": total_runs,
        "status_distribution": dict(status_counts),
        "category_frequency": dict(category_counts.most_common()),
        "crystallization_candidates": candidates,
        "category_trends": trend,
    }


def main():
    harness_dir = sys.argv[1] if len(sys.argv) > 1 else ".harness"

    if not os.path.isdir(harness_dir):
        print(f"Directory not found: {harness_dir}", file=sys.stderr)
        sys.exit(1)

    log_paths = find_governance_logs(harness_dir)
    if not log_paths:
        print("No governance log files found.", file=sys.stderr)
        print(json.dumps({"total_runs": 0, "message": "No governance logs found."}))
        return

    print(f"Found {len(log_paths)} governance log file(s):", file=sys.stderr)
    for p in log_paths:
        print(f"  - {p}", file=sys.stderr)

    records = load_records(log_paths)
    print(f"Loaded {len(records)} run record(s).", file=sys.stderr)

    report = aggregate(records)
    print(json.dumps(report, indent=2))

    # Print summary to stderr
    if report["crystallization_candidates"]:
        print(f"\n=== Crystallization Candidates ({len(report['crystallization_candidates'])}) ===",
              file=sys.stderr)
        for c in report["crystallization_candidates"]:
            print(f"  [{c['category']}] {c['total_occurrences']} occurrences "
                  f"across {c['unique_runs']} runs", file=sys.stderr)
    else:
        print("\nNo crystallization candidates yet (need ≥3 occurrences).",
              file=sys.stderr)

    if report.get("category_trends"):
        rising = [c for c, t in report["category_trends"].items() if t["direction"] == "rising"]
        declining = [c for c, t in report["category_trends"].items() if t["direction"] == "declining"]
        if rising:
            print(f"\n  Rising categories (getting worse): {', '.join(rising)}",
                  file=sys.stderr)
        if declining:
            print(f"  Declining categories (improving): {', '.join(declining)}",
                  file=sys.stderr)

    # Suggest running harness eval for deeper analysis
    print("\nFor full harness scoring, run: python3 evaluate_harness.py",
          file=sys.stderr)


if __name__ == "__main__":
    main()
