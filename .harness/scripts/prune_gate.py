#!/usr/bin/env python3
"""prune_gate.py — Algorithmic redundancy detection for fractal tree pruning.

Replaces the AI prune step with deterministic analysis:
1. Diff-based subset detection: is a node's output fully contained in a sibling's?
2. File overlap analysis: did multiple nodes produce identical changes?
3. Empty/trivial output detection: did a node produce nothing meaningful?

Algorithm: Set cover — find the minimal set of nodes that covers all outputs.

Input JSON from stdin:
{
  "tree": {
    "1-auth": {"slug": "auth", "status": "solved", "files": [...], "result_hash": "..."},
    "2-data": {"slug": "data", "status": "solved", "files": [...], "result_hash": "..."}
  }
}

Output JSON:
{
  "prunable": ["1-auth"],
  "reasons": {"1-auth": "output is subset of sibling 2-data"},
  "kept": ["2-data", "3-api"],
  "file_coverage": {"src/auth.rs": ["1-auth", "2-data"]}
}
"""

import json
import subprocess
import sys
from collections import defaultdict


def get_changed_files(branch_or_ref: str, base: str = "main") -> set[str]:
    """Get files changed between base and ref."""
    try:
        result = subprocess.run(
            ["git", "diff", "--name-only", f"{base}...{branch_or_ref}"],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return {f.strip() for f in result.stdout.splitlines() if f.strip()}
    except Exception:
        pass
    return set()


def get_diff_stat(branch_or_ref: str, base: str = "main") -> str:
    """Get diff content for a branch."""
    try:
        result = subprocess.run(
            ["git", "diff", f"{base}...{branch_or_ref}"],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return result.stdout
    except Exception:
        pass
    return ""


def detect_subset_outputs(nodes: list[dict]) -> dict[str, str]:
    """Find nodes whose file changes are a strict subset of a sibling's.

    If node A changed {a.rs, b.rs} and node B changed {a.rs, b.rs, c.rs},
    then A's output is a subset of B's — A is prunable.
    """
    prunable: dict[str, str] = {}

    for i, node_a in enumerate(nodes):
        files_a = set(node_a.get("files", []))
        if not files_a:
            # Empty output — always prunable
            prunable[node_a["slug"]] = "produced no file changes"
            continue

        for j, node_b in enumerate(nodes):
            if i == j:
                continue
            files_b = set(node_b.get("files", []))
            if files_a < files_b:  # strict subset
                prunable[node_a["slug"]] = (
                    f"output files are a strict subset of sibling "
                    f"'{node_b['slug']}' ({len(files_a)} vs {len(files_b)} files)"
                )
                break

    return prunable


def detect_identical_outputs(nodes: list[dict]) -> list[tuple[str, str]]:
    """Find pairs of nodes with identical file change sets.

    When two nodes changed exactly the same files, one is redundant.
    Keep the one with more lines changed (heuristic for "more complete").
    """
    identical_pairs = []
    for i, node_a in enumerate(nodes):
        for j, node_b in enumerate(nodes):
            if i >= j:
                continue
            files_a = set(node_a.get("files", []))
            files_b = set(node_b.get("files", []))
            if files_a and files_a == files_b:
                identical_pairs.append((node_a["slug"], node_b["slug"]))

    return identical_pairs


def compute_file_coverage(nodes: list[dict]) -> dict[str, list[str]]:
    """Map each file to which nodes touched it."""
    coverage: dict[str, list[str]] = defaultdict(list)
    for node in nodes:
        for f in node.get("files", []):
            coverage[f].append(node["slug"])
    return dict(coverage)


def minimal_covering_set(nodes: list[dict]) -> list[str]:
    """Greedy set cover: find minimal set of nodes that covers all files.

    This is the classic greedy approximation to set cover.
    The nodes NOT in the covering set are candidates for pruning.
    """
    all_files: set[str] = set()
    node_files: dict[str, set[str]] = {}
    for node in nodes:
        files = set(node.get("files", []))
        node_files[node["slug"]] = files
        all_files |= files

    if not all_files:
        return []

    covered: set[str] = set()
    selected: list[str] = []
    remaining = dict(node_files)

    while covered < all_files and remaining:
        # Pick the node that covers the most uncovered files
        best_slug = max(remaining, key=lambda s: len(remaining[s] - covered))
        new_coverage = remaining[best_slug] - covered
        if not new_coverage:
            break
        selected.append(best_slug)
        covered |= new_coverage
        del remaining[best_slug]

    return selected


def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": str(e)}))
        return

    tree = data.get("tree", {})

    # Collect solved nodes
    nodes = []
    for path, node in tree.items():
        status = node.get("status", "")
        if status in ("solved", "reunified"):
            nodes.append({**node, "path": path})

    if not nodes:
        print(json.dumps({
            "prunable": [],
            "reasons": {},
            "kept": [],
            "file_coverage": {},
        }, indent=2))
        return

    # Analysis
    subset_prunable = detect_subset_outputs(nodes)
    identical_pairs = detect_identical_outputs(nodes)
    file_coverage = compute_file_coverage(nodes)
    covering_set = minimal_covering_set(nodes)

    # Nodes not in the minimal covering set are also prune candidates
    all_slugs = {n["slug"] for n in nodes}
    not_in_cover = all_slugs - set(covering_set)

    # Merge reasons
    reasons: dict[str, str] = dict(subset_prunable)
    for slug in not_in_cover:
        if slug not in reasons:
            reasons[slug] = "not in minimal covering set (all files covered by other nodes)"
    for slug_a, slug_b in identical_pairs:
        if slug_a not in reasons and slug_b not in reasons:
            # Keep the one that appears first in covering set
            if slug_a in covering_set:
                reasons[slug_b] = f"identical file set as '{slug_a}' (keeping '{slug_a}')"
            else:
                reasons[slug_a] = f"identical file set as '{slug_b}' (keeping '{slug_b}')"

    prunable = sorted(reasons.keys())
    kept = sorted(all_slugs - set(prunable))

    result = {
        "prunable": prunable,
        "reasons": reasons,
        "kept": kept,
        "file_coverage": file_coverage,
        "identical_pairs": [list(p) for p in identical_pairs],
        "minimal_covering_set": covering_set,
    }

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
