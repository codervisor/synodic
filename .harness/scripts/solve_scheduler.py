#!/usr/bin/env python3
"""solve_scheduler.py — DAG-based critical path scheduling for fractal solve.

Reads a fractal manifest.json from stdin (or file path as arg) and computes
optimal solve waves — groups of leaves that can execute concurrently.

Replaces the binary parallel/sequential choice with dependency-aware scheduling:
- All independent leaves → single wave (= fully parallel)
- All dependent leaves → one per wave (= fully sequential)
- Mixed → multiple waves with maximal parallelism per wave

Algorithm: BFS layer decomposition over the dependency DAG (topological sort).
Complexity: O(V + E) where V = leaves, E = dependency edges.

Input: manifest.json with tree nodes containing inputs/outputs declarations.
Output JSON:
{
  "waves": [["leaf-a", "leaf-b"], ["leaf-c"]],
  "critical_path": ["leaf-a", "leaf-c"],
  "critical_path_length": 2,
  "max_parallelism": 2,
  "total_leaves": 3
}
"""

import json
import re
import sys
from collections import defaultdict


STOP_WORDS = {
    "the", "and", "for", "that", "this", "with", "from", "are", "was",
    "were", "been", "have", "has", "had", "not", "but", "its", "can",
    "will", "should", "must", "may", "each", "all", "any", "into",
    "when", "how", "what", "which", "their", "them", "they", "you",
    "your", "about", "also", "does", "using", "used", "use",
}


def extract_terms(text: str) -> set[str]:
    """Extract lowercase alphanumeric terms (>=3 chars) from text."""
    words = re.findall(r"[a-zA-Z][a-zA-Z0-9_-]{2,}", text.lower())
    return {w for w in words if w not in STOP_WORDS}


def collect_leaves(tree: dict) -> list[dict]:
    """Extract all leaf nodes from the manifest tree."""
    leaves = []
    for path, node in tree.items():
        status = node.get("status", "")
        if status in ("leaf", "forced-leaf", "pending") and not node.get("children"):
            leaves.append({**node, "path": path})
    return leaves


def build_dependency_graph(leaves: list[dict]) -> dict[str, set[str]]:
    """Build a dependency DAG from declared inputs/outputs.

    Returns: {slug: set of slugs it depends on}
    """
    slugs = {leaf["slug"] for leaf in leaves}

    # Map output terms to producing leaf
    output_map: dict[str, str] = {}
    for leaf in leaves:
        for term in extract_terms(leaf.get("outputs", "")):
            output_map[term] = leaf["slug"]

    # Build dependencies
    deps: dict[str, set[str]] = {leaf["slug"]: set() for leaf in leaves}
    for leaf in leaves:
        for term in extract_terms(leaf.get("inputs", "")):
            producer = output_map.get(term)
            if producer and producer != leaf["slug"] and producer in slugs:
                deps[leaf["slug"]].add(producer)

    return deps


def compute_waves(deps: dict[str, set[str]]) -> list[list[str]]:
    """Topological sort into parallel execution waves.

    Each wave contains nodes whose dependencies are all resolved
    by previous waves. Nodes within a wave can run concurrently.
    """
    all_nodes = set(deps.keys())
    resolved: set[str] = set()
    waves: list[list[str]] = []

    while len(resolved) < len(all_nodes):
        wave = sorted([
            n for n in all_nodes
            if n not in resolved and deps[n].issubset(resolved)
        ])
        if not wave:
            # Cycle detected — force remaining nodes into final wave
            remaining = sorted(n for n in all_nodes if n not in resolved)
            waves.append(remaining)
            break
        waves.append(wave)
        resolved.update(wave)

    return waves


def compute_critical_path(deps: dict[str, set[str]], waves: list[list[str]]) -> list[str]:
    """Find the longest dependency chain (critical path).

    The critical path determines the minimum number of sequential
    waves required, regardless of parallelism.
    """
    # Assign each node to its wave index
    wave_of: dict[str, int] = {}
    for i, wave in enumerate(waves):
        for slug in wave:
            wave_of[slug] = i

    # Find longest path using dynamic programming
    # longest[n] = length of longest path ending at n
    longest: dict[str, int] = {n: 1 for n in deps}
    predecessor: dict[str, str | None] = {n: None for n in deps}

    # Process in wave order (topological order)
    for wave in waves:
        for node in wave:
            for dep in deps[node]:
                if longest[dep] + 1 > longest[node]:
                    longest[node] = longest[dep] + 1
                    predecessor[node] = dep

    # Trace back from the node with longest path
    if not longest:
        return []

    end_node = max(longest, key=longest.get)
    path = []
    current: str | None = end_node
    while current is not None:
        path.append(current)
        current = predecessor[current]

    return list(reversed(path))


def main():
    # Read manifest from stdin or file arg
    if len(sys.argv) > 1:
        with open(sys.argv[1]) as f:
            manifest = json.load(f)
    else:
        manifest = json.load(sys.stdin)

    tree = manifest.get("tree", {})
    leaves = collect_leaves(tree)

    if not leaves:
        print(json.dumps({
            "waves": [],
            "critical_path": [],
            "critical_path_length": 0,
            "max_parallelism": 0,
            "total_leaves": 0,
        }, indent=2))
        return

    deps = build_dependency_graph(leaves)
    waves = compute_waves(deps)
    critical_path = compute_critical_path(deps, waves)

    result = {
        "waves": waves,
        "critical_path": critical_path,
        "critical_path_length": len(waves),
        "max_parallelism": max(len(w) for w in waves) if waves else 0,
        "total_leaves": len(leaves),
    }

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
