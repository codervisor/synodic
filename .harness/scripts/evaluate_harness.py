#!/usr/bin/env python3
"""evaluate_harness.py — Score the governance harness itself.

Analyzes .harness/*.governance.jsonl over time to measure whether the
governance layer is actually reducing mistakes and improving agent output.

Metrics:
  1. Layer distribution  — % issues caught at L1 vs L2 vs L3 (L1 should grow)
  2. Repeat rate         — Same category reappearing across runs (should shrink)
  3. Rework trend        — Avg rework cycles per run over time (should shrink)
  4. Crystallization health — Candidates identified → rules created → rules effective
  5. Taxonomy coverage   — % feedback fitting existing categories vs uncategorized

Usage:
  python3 evaluate_harness.py [harness-dir] [--window N] [--json]

  harness-dir: path to .harness/ directory (default: .harness/ in repo root)
  --window N:  sliding window size for trend analysis (default: 5 runs)
  --json:      output machine-readable JSON only
"""

import glob
import json
import math
import os
import re
import sys
from collections import Counter, defaultdict
from datetime import datetime


# ---------------------------------------------------------------------------
# Data loading (shared with aggregate_governance.py)
# ---------------------------------------------------------------------------

def find_governance_logs(harness_dir: str) -> list[str]:
    paths = []
    paths.extend(glob.glob(os.path.join(harness_dir, "*.governance.jsonl")))
    repo_root = os.path.dirname(harness_dir) if harness_dir.endswith(".harness") else harness_dir
    for topology in ("factory", "fractal"):
        fallback = os.path.join(repo_root, f".{topology}", "governance.jsonl")
        if os.path.isfile(fallback):
            paths.append(fallback)
    return list(set(paths))


def load_records(paths: list[str]) -> list[dict]:
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


def find_rules(harness_dir: str) -> list[str]:
    """List crystallized rule scripts in .harness/rules/."""
    rules_dir = os.path.join(harness_dir, "rules")
    if not os.path.isdir(rules_dir):
        return []
    return [
        f for f in os.listdir(rules_dir)
        if f != ".gitkeep" and os.path.isfile(os.path.join(rules_dir, f))
    ]


# ---------------------------------------------------------------------------
# Feedback extraction helpers
# ---------------------------------------------------------------------------

KNOWN_CATEGORIES = {
    # Code-producing (§5)
    "[completeness]", "[correctness]", "[security]", "[conformance]", "[quality]",
    # Decomposition (§5)
    "[orthogonality]", "[coverage]", "[granularity]", "[budget]",
    # Integration (§5)
    "[interface]", "[boundary]", "[redundancy]", "[gap]",
}


def extract_feedback(record: dict) -> list[dict]:
    """Extract all feedback items from a governance record."""
    items = []
    for item in record.get("rework_items", []):
        items.append({
            "category": item.get("category", "uncategorized"),
            "description": item.get("description", ""),
            "source": "rework",
        })
    for flag in record.get("decompose_flags", []):
        items.append({
            "category": flag.get("category", "uncategorized"),
            "description": flag.get("description", ""),
            "source": "decompose",
        })
    for conflict in record.get("reunify_conflicts", []):
        items.append({
            "category": conflict.get("category", "uncategorized"),
            "description": conflict.get("description", ""),
            "source": "reunify",
        })
    return items


def get_layer(record: dict) -> str:
    """Infer which layer caught the issue based on record fields."""
    if record.get("static_failures"):
        return "L1"
    if record.get("status") == "escalated":
        return "L3"
    # Default: if there are rework items / flags but no static failures → AI judge
    if (record.get("rework_items") or record.get("decompose_flags")
            or record.get("reunify_conflicts")):
        return "L2"
    return "L1"  # clean pass through Layer 1


def get_timestamp(record: dict) -> float:
    """Extract a sortable timestamp from a record."""
    # Try explicit timestamp field
    ts = record.get("timestamp")
    if ts:
        try:
            return datetime.fromisoformat(ts.replace("Z", "+00:00")).timestamp()
        except (ValueError, AttributeError):
            pass
    # Try work_id pattern: factory-1710600000 or fractal-1710600000
    wid = record.get("work_id", "")
    match = re.search(r"-(\d{10})", wid)
    if match:
        return float(match.group(1))
    return 0.0


# ---------------------------------------------------------------------------
# Metric computations
# ---------------------------------------------------------------------------

def compute_layer_distribution(records: list[dict]) -> dict:
    """What % of runs resolved at each layer? L1 growing = harness working."""
    counts = Counter()
    for rec in records:
        counts[get_layer(rec)] += 1
    total = sum(counts.values()) or 1
    return {
        "counts": dict(counts),
        "rates": {layer: round(c / total, 3) for layer, c in counts.items()},
        "total": total,
    }


def compute_repeat_rate(records: list[dict], window: int) -> dict:
    """Are the same categories showing up in later runs?

    For each sliding window of `window` consecutive runs, compute what fraction
    of feedback categories already appeared in a prior window. A declining
    repeat rate means the harness is catching & preventing recurring issues.
    """
    if len(records) < window:
        return {
            "windows": [],
            "trend": "insufficient_data",
            "message": f"Need at least {window} runs for trend analysis",
        }

    sorted_recs = sorted(records, key=get_timestamp)
    windows = []
    seen_ever = set()

    for i in range(0, len(sorted_recs) - window + 1, max(1, window // 2)):
        chunk = sorted_recs[i:i + window]
        cats_in_window = set()
        for rec in chunk:
            for fb in extract_feedback(rec):
                cats_in_window.add(fb["category"])

        repeats = cats_in_window & seen_ever
        repeat_ratio = len(repeats) / len(cats_in_window) if cats_in_window else 0.0
        windows.append({
            "run_range": [i, i + window - 1],
            "categories_seen": len(cats_in_window),
            "repeat_categories": len(repeats),
            "repeat_ratio": round(repeat_ratio, 3),
        })
        seen_ever |= cats_in_window

    # Trend: compare first half of windows to second half
    if len(windows) >= 2:
        mid = len(windows) // 2
        early = sum(w["repeat_ratio"] for w in windows[:mid]) / mid
        late = sum(w["repeat_ratio"] for w in windows[mid:]) / (len(windows) - mid)
        if late < early - 0.05:
            trend = "improving"
        elif late > early + 0.05:
            trend = "degrading"
        else:
            trend = "stable"
    else:
        trend = "insufficient_data"

    return {"windows": windows, "trend": trend}


def compute_rework_trend(records: list[dict], window: int) -> dict:
    """Average rework attempts per run over time. Declining = harness working."""
    sorted_recs = sorted(records, key=get_timestamp)
    windows = []

    for i in range(0, len(sorted_recs) - window + 1, max(1, window // 2)):
        chunk = sorted_recs[i:i + window]
        attempts = []
        for rec in chunk:
            metrics = rec.get("metrics", {})
            attempts.append(metrics.get("attempt_count", 1))

        avg = sum(attempts) / len(attempts) if attempts else 0
        windows.append({
            "run_range": [i, i + window - 1],
            "avg_attempts": round(avg, 2),
            "max_attempts": max(attempts) if attempts else 0,
        })

    if len(windows) >= 2:
        mid = len(windows) // 2
        early = sum(w["avg_attempts"] for w in windows[:mid]) / mid
        late = sum(w["avg_attempts"] for w in windows[mid:]) / (len(windows) - mid)
        if late < early - 0.1:
            trend = "improving"
        elif late > early + 0.1:
            trend = "degrading"
        else:
            trend = "stable"
    else:
        trend = "insufficient_data"

    return {"windows": windows, "trend": trend}


def compute_crystallization_health(records: list[dict], rules: list[str]) -> dict:
    """Pipeline health: patterns → candidates → rules → effective rules.

    Stages:
      1. pattern_count: categories appearing ≥2 times across runs
      2. candidate_count: categories appearing ≥3 times (crystallization-ready)
      3. rules_created: files in .harness/rules/
      4. rules_effective: rules that correspond to a category whose frequency
         dropped after the rule was presumably added
    """
    category_runs = defaultdict(set)
    for rec in records:
        wid = rec.get("work_id", "unknown")
        for fb in extract_feedback(rec):
            category_runs[fb["category"]].add(wid)

    patterns = {c: len(runs) for c, runs in category_runs.items() if len(runs) >= 2}
    candidates = {c: count for c, count in patterns.items() if count >= 3}

    # Rules effectiveness: for each rule file, check if its name matches any category
    # and whether that category's frequency declined in later records
    rule_names = set(rules)
    rules_matched = []
    for rule in rule_names:
        # Try to match rule name to category: no-secrets.sh → [security],
        # or by exact rule name in static_failures
        for cat in candidates:
            if cat.strip("[]") in rule.replace("-", "").replace("_", ""):
                rules_matched.append({"rule": rule, "covers_category": cat})

    conversion_rate = (
        len(rules) / len(candidates) if candidates
        else 1.0 if not candidates and not patterns  # no issues = healthy
        else 0.0
    )

    return {
        "patterns": len(patterns),
        "candidates": len(candidates),
        "candidate_categories": sorted(candidates.keys()),
        "rules_created": len(rules),
        "rule_files": sorted(rules),
        "rules_matched_to_candidates": rules_matched,
        "conversion_rate": round(conversion_rate, 3),
    }


def compute_taxonomy_coverage(records: list[dict]) -> dict:
    """What % of feedback fits the §5 taxonomy? Uncategorized piling up = gap."""
    category_counts = Counter()
    for rec in records:
        for fb in extract_feedback(rec):
            cat = fb["category"]
            # Normalize: add brackets if missing
            normalized = cat if cat.startswith("[") else f"[{cat}]"
            category_counts[normalized] += 1

    total = sum(category_counts.values()) or 1
    known = sum(c for cat, c in category_counts.items() if cat in KNOWN_CATEGORIES)
    unknown_cats = {
        cat: count for cat, count in category_counts.items()
        if cat not in KNOWN_CATEGORIES
    }

    return {
        "total_feedback_items": sum(category_counts.values()),
        "known_category_count": known,
        "unknown_category_count": sum(unknown_cats.values()),
        "coverage_rate": round(known / total, 3),
        "unknown_categories": dict(unknown_cats) if unknown_cats else {},
        "category_distribution": dict(category_counts.most_common()),
    }


# ---------------------------------------------------------------------------
# Scoring — collapse metrics into a single harness health score
# ---------------------------------------------------------------------------

def score_harness(layer_dist: dict, repeat: dict, rework: dict,
                  crystal: dict, taxonomy: dict, total_runs: int) -> dict:
    """Compute an overall harness health score (0–100).

    Dimensions (each 0–20 points):
      1. Layer efficiency: L1 catching more over time
      2. Repeat reduction: repeat rate trend
      3. Rework efficiency: rework trend improving
      4. Crystallization pipeline: patterns → rules conversion
      5. Taxonomy coverage: feedback classifiable under §5
    """

    scores = {}

    # 1. Layer efficiency (20 pts)
    # Ideal: L1 rate > 50% — most issues caught cheaply
    l1_rate = layer_dist.get("rates", {}).get("L1", 0)
    scores["layer_efficiency"] = min(20, round(l1_rate * 40))  # 50% L1 → full marks

    # 2. Repeat reduction (20 pts)
    trend_map = {"improving": 20, "stable": 12, "degrading": 4, "insufficient_data": 10}
    scores["repeat_reduction"] = trend_map.get(repeat.get("trend", ""), 10)

    # 3. Rework efficiency (20 pts)
    trend_map_rw = {"improving": 20, "stable": 12, "degrading": 4, "insufficient_data": 10}
    scores["rework_efficiency"] = trend_map_rw.get(rework.get("trend", ""), 10)

    # 4. Crystallization pipeline (20 pts)
    conv = crystal.get("conversion_rate", 0)
    # No candidates and no patterns = clean, give full marks
    if crystal["patterns"] == 0 and crystal["candidates"] == 0:
        scores["crystallization"] = 20 if total_runs > 0 else 10
    else:
        scores["crystallization"] = min(20, round(conv * 20))

    # 5. Taxonomy coverage (20 pts)
    cov = taxonomy.get("coverage_rate", 1.0)
    scores["taxonomy_coverage"] = min(20, round(cov * 20))

    total = sum(scores.values())

    # Grade
    if total >= 80:
        grade = "A"
    elif total >= 60:
        grade = "B"
    elif total >= 40:
        grade = "C"
    elif total >= 20:
        grade = "D"
    else:
        grade = "F"

    return {
        "total": total,
        "max": 100,
        "grade": grade,
        "breakdown": scores,
    }


# ---------------------------------------------------------------------------
# Report formatting
# ---------------------------------------------------------------------------

def format_report(report: dict) -> str:
    """Format report as human-readable text."""
    lines = []
    lines.append("=" * 60)
    lines.append("  HARNESS GOVERNANCE EVALUATION REPORT")
    lines.append("=" * 60)
    lines.append("")

    score = report["score"]
    lines.append(f"  Overall Score: {score['total']}/{score['max']}  Grade: {score['grade']}")
    lines.append(f"  Total Runs Analyzed: {report['total_runs']}")
    lines.append("")

    # Score breakdown
    lines.append("  Score Breakdown:")
    for dim, pts in score["breakdown"].items():
        bar = "#" * pts + "." * (20 - pts)
        label = dim.replace("_", " ").title()
        lines.append(f"    {label:<25} [{bar}] {pts}/20")
    lines.append("")

    # Layer distribution
    ld = report["layer_distribution"]
    lines.append("  Layer Distribution:")
    for layer in ("L1", "L2", "L3"):
        count = ld["counts"].get(layer, 0)
        rate = ld["rates"].get(layer, 0)
        lines.append(f"    {layer}: {count:>4} runs ({rate:.1%})")
    lines.append("")

    # Repeat rate trend
    rr = report["repeat_rate"]
    lines.append(f"  Repeat Rate Trend: {rr['trend']}")
    if rr.get("windows"):
        first = rr["windows"][0]["repeat_ratio"]
        last = rr["windows"][-1]["repeat_ratio"]
        lines.append(f"    First window: {first:.1%}  →  Last window: {last:.1%}")
    lines.append("")

    # Rework trend
    rw = report["rework_trend"]
    lines.append(f"  Rework Trend: {rw['trend']}")
    if rw.get("windows"):
        first = rw["windows"][0]["avg_attempts"]
        last = rw["windows"][-1]["avg_attempts"]
        lines.append(f"    First window avg: {first:.1f}  →  Last window avg: {last:.1f}")
    lines.append("")

    # Crystallization
    cr = report["crystallization"]
    lines.append("  Crystallization Pipeline:")
    lines.append(f"    Patterns (≥2 runs): {cr['patterns']}")
    lines.append(f"    Candidates (≥3 runs): {cr['candidates']}")
    lines.append(f"    Rules created: {cr['rules_created']}")
    lines.append(f"    Conversion rate: {cr['conversion_rate']:.0%}")
    if cr["candidate_categories"]:
        lines.append(f"    Candidate categories: {', '.join(cr['candidate_categories'])}")
    lines.append("")

    # Taxonomy
    tx = report["taxonomy_coverage"]
    lines.append(f"  Taxonomy Coverage: {tx['coverage_rate']:.0%}")
    lines.append(f"    Classified: {tx['known_category_count']}  "
                 f"Unclassified: {tx['unknown_category_count']}")
    if tx["unknown_categories"]:
        lines.append(f"    Unknown categories: {tx['unknown_categories']}")
    lines.append("")

    # Actionable insights
    lines.append("-" * 60)
    lines.append("  Actionable Insights:")
    insights = generate_insights(report)
    if insights:
        for insight in insights:
            lines.append(f"    • {insight}")
    else:
        lines.append("    (none — harness is healthy or needs more data)")
    lines.append("")
    lines.append("=" * 60)

    return "\n".join(lines)


def generate_insights(report: dict) -> list[str]:
    """Generate actionable recommendations from the report."""
    insights = []
    score = report["score"]
    bd = score["breakdown"]

    if bd["layer_efficiency"] < 10:
        l2_rate = report["layer_distribution"]["rates"].get("L2", 0)
        insights.append(
            f"Layer 1 is underused ({report['layer_distribution']['rates'].get('L1', 0):.0%}). "
            f"Consider crystallizing frequent L2 findings into static rules."
        )

    if report["repeat_rate"].get("trend") == "degrading":
        insights.append(
            "Repeat rate is increasing — the same categories keep appearing. "
            "Prioritize crystallizing the most frequent categories into rules."
        )

    if report["rework_trend"].get("trend") == "degrading":
        insights.append(
            "Average rework cycles are increasing. Review whether AI judge feedback "
            "is actionable enough for the rework agent."
        )

    cr = report["crystallization"]
    if cr["candidates"] > 0 and cr["rules_created"] == 0:
        insights.append(
            f"{cr['candidates']} crystallization candidate(s) identified but 0 rules created. "
            f"Categories: {', '.join(cr['candidate_categories'])}. Run the crystallization workflow."
        )

    tx = report["taxonomy_coverage"]
    if tx["coverage_rate"] < 0.8 and tx["unknown_categories"]:
        insights.append(
            f"Taxonomy coverage is only {tx['coverage_rate']:.0%}. "
            f"Consider proposing new categories for: {list(tx['unknown_categories'].keys())}"
        )

    if report["total_runs"] < 5:
        insights.append(
            f"Only {report['total_runs']} run(s) recorded. Trends require ≥5 runs "
            f"for meaningful analysis."
        )

    return insights


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Evaluate harness governance effectiveness")
    parser.add_argument("harness_dir", nargs="?", default=".harness",
                        help="Path to .harness/ directory")
    parser.add_argument("--window", type=int, default=5,
                        help="Sliding window size for trend analysis")
    parser.add_argument("--json", action="store_true",
                        help="Output machine-readable JSON only")
    args = parser.parse_args()

    if not os.path.isdir(args.harness_dir):
        print(f"Directory not found: {args.harness_dir}", file=sys.stderr)
        sys.exit(1)

    log_paths = find_governance_logs(args.harness_dir)
    records = load_records(log_paths) if log_paths else []

    if not args.json:
        print(f"Found {len(log_paths)} log file(s), {len(records)} record(s).",
              file=sys.stderr)

    rules = find_rules(args.harness_dir)

    # Compute all metrics
    layer_dist = compute_layer_distribution(records)
    repeat = compute_repeat_rate(records, args.window)
    rework = compute_rework_trend(records, args.window)
    crystal = compute_crystallization_health(records, rules)
    taxonomy = compute_taxonomy_coverage(records)

    report = {
        "total_runs": len(records),
        "log_files": log_paths,
        "window_size": args.window,
        "layer_distribution": layer_dist,
        "repeat_rate": repeat,
        "rework_trend": rework,
        "crystallization": crystal,
        "taxonomy_coverage": taxonomy,
        "score": score_harness(layer_dist, repeat, rework, crystal, taxonomy, len(records)),
    }

    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(format_report(report))


if __name__ == "__main__":
    main()
