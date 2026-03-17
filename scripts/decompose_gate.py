#!/usr/bin/env python3
"""Decompose Gate — deterministic structural checks on fractal decompositions.

Takes child scopes as JSON input, returns flags as JSON output.
No LLM calls — pure heuristic checks.

Usage:
    echo '{"parent_spec": "...", "children": [...], "current_depth": 1, "max_depth": 3, "total_nodes": 5, "max_total_nodes": 20}' | python3 scripts/decompose_gate.py

Input JSON schema:
    {
        "parent_spec": "full text of the parent spec",
        "children": [
            {"slug": "auth", "scope": "handle authentication and authorization"},
            {"slug": "data", "scope": "manage data persistence and queries"}
        ],
        "current_depth": <int>,
        "max_depth": <int>,
        "total_nodes": <int>,
        "max_total_nodes": <int>
    }

Output JSON schema:
    {
        "flags": [
            {"category": "orthogonality", "description": "..."},
            {"category": "coverage", "description": "..."},
            {"category": "budget", "description": "..."}
        ]
    }
"""

import json
import re
import sys
from itertools import combinations


def extract_terms(text: str) -> set[str]:
    """Extract meaningful terms from text for comparison.

    Lowercases, splits on non-alphanumeric, filters short/stop words.
    """
    stop_words = {
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "with", "by", "from", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would",
        "could", "should", "may", "might", "shall", "can", "need", "must",
        "this", "that", "these", "those", "it", "its", "not", "no", "nor",
        "as", "if", "then", "else", "when", "up", "out", "so", "than",
        "too", "very", "just", "about", "above", "after", "again", "all",
        "also", "any", "each", "how", "into", "more", "most", "other",
        "over", "own", "same", "some", "such", "only", "both", "through",
    }
    words = re.findall(r"[a-z0-9]+", text.lower())
    return {w for w in words if len(w) > 2 and w not in stop_words}


def jaccard_similarity(set_a: set, set_b: set) -> float:
    """Compute Jaccard similarity between two sets."""
    if not set_a and not set_b:
        return 0.0
    intersection = set_a & set_b
    union = set_a | set_b
    return len(intersection) / len(union)


def check_orthogonality(children: list[dict]) -> list[dict]:
    """Flag pairs of children with >30% keyword overlap in scope."""
    flags = []
    for (i, child_a), (j, child_b) in combinations(enumerate(children), 2):
        terms_a = extract_terms(child_a["scope"])
        terms_b = extract_terms(child_b["scope"])
        similarity = jaccard_similarity(terms_a, terms_b)
        if similarity > 0.3:
            overlap = terms_a & terms_b
            flags.append({
                "category": "orthogonality",
                "description": (
                    f"Children '{child_a['slug']}' and '{child_b['slug']}' "
                    f"have {similarity:.0%} scope overlap. "
                    f"Shared terms: {', '.join(sorted(overlap))}"
                ),
            })
    return flags


def check_coverage(parent_spec: str, children: list[dict]) -> list[dict]:
    """Flag parent terms not covered by any child scope."""
    parent_terms = extract_terms(parent_spec)
    child_terms = set()
    for child in children:
        child_terms |= extract_terms(child["scope"])

    uncovered = parent_terms - child_terms
    # Filter to only flag if significant terms are missing (>20% of parent terms)
    if parent_terms and len(uncovered) / len(parent_terms) > 0.2:
        # Only report up to 10 most relevant uncovered terms
        sample = sorted(uncovered)[:10]
        return [{
            "category": "coverage",
            "description": (
                f"Parent spec mentions terms not covered by any child: "
                f"{', '.join(sample)}"
                f"{' (and more)' if len(uncovered) > 10 else ''}. "
                f"{len(uncovered)}/{len(parent_terms)} parent terms uncovered."
            ),
        }]
    return []


def check_budget(
    total_nodes: int,
    max_total_nodes: int,
    num_children: int,
    current_depth: int,
    max_depth: int,
) -> list[dict]:
    """Warn if budget is tight after this split."""
    projected = total_nodes + num_children
    threshold = max_total_nodes * 0.8

    if projected >= threshold and current_depth < max_depth - 1:
        return [{
            "category": "budget",
            "description": (
                f"After this split: {projected}/{max_total_nodes} nodes used "
                f"({projected / max_total_nodes:.0%} of budget) "
                f"with {max_depth - current_depth - 1} depth levels remaining. "
                f"Further splitting may be constrained."
            ),
        }]
    return []


def main() -> None:
    raw = sys.stdin.read().strip()
    if not raw:
        print(json.dumps({"error": "No input provided"}))
        sys.exit(1)

    try:
        data = json.loads(raw)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": f"Invalid JSON: {e}"}))
        sys.exit(1)

    parent_spec = data.get("parent_spec", "")
    children = data.get("children", [])
    current_depth = data.get("current_depth", 0)
    max_depth = data.get("max_depth", 3)
    total_nodes = data.get("total_nodes", 0)
    max_total_nodes = data.get("max_total_nodes", 20)

    flags = []
    flags.extend(check_orthogonality(children))
    flags.extend(check_coverage(parent_spec, children))
    flags.extend(check_budget(
        total_nodes, max_total_nodes, len(children), current_depth, max_depth,
    ))

    print(json.dumps({"flags": flags}, indent=2))


if __name__ == "__main__":
    main()
