#!/usr/bin/env python3
"""decompose_gate.py — Validate a fractal decomposition structurally.

Reads JSON from stdin with the shape:
{
  "parent_spec": "<parent spec text>",
  "children": [{"slug": "...", "scope": "..."}],
  "current_depth": N,
  "max_depth": N,
  "total_nodes": N,
  "max_total_nodes": N
}

Outputs JSON with a "flags" array. Each flag has "category" and "description".
Exit 0 always (flags are advisory, not errors).
"""

import json
import re
import sys
from itertools import combinations


def extract_terms(text: str) -> set[str]:
    """Extract lowercase alphanumeric terms (>=3 chars) from text."""
    words = re.findall(r"[a-zA-Z][a-zA-Z0-9_-]{2,}", text.lower())
    # Filter out common stop words
    stop = {
        "the", "and", "for", "that", "this", "with", "from", "are", "was",
        "were", "been", "have", "has", "had", "not", "but", "its", "can",
        "will", "should", "must", "may", "each", "all", "any", "into",
        "when", "how", "what", "which", "their", "them", "they", "you",
        "your", "about", "also", "does", "using", "used", "use",
    }
    return {w for w in words if w not in stop}


def jaccard_similarity(a: set[str], b: set[str]) -> float:
    """Compute Jaccard similarity between two term sets."""
    if not a and not b:
        return 0.0
    intersection = len(a & b)
    union = len(a | b)
    return intersection / union if union > 0 else 0.0


def check_orthogonality(children: list[dict]) -> list[dict]:
    """Flag pairs of children with >30% keyword overlap."""
    flags = []
    child_terms = [(c["slug"], extract_terms(c.get("scope", ""))) for c in children]
    for (slug_a, terms_a), (slug_b, terms_b) in combinations(child_terms, 2):
        sim = jaccard_similarity(terms_a, terms_b)
        if sim > 0.30:
            flags.append({
                "category": "orthogonality",
                "description": (
                    f"Children '{slug_a}' and '{slug_b}' have {sim:.0%} "
                    f"keyword overlap in their scopes. "
                    f"Shared terms: {', '.join(sorted(terms_a & terms_b)[:5])}"
                ),
            })
    return flags


def check_coverage(parent_spec: str, children: list[dict]) -> list[dict]:
    """Flag if parent terms are not covered by any child."""
    flags = []
    parent_terms = extract_terms(parent_spec)
    if not parent_terms:
        return flags

    child_terms = set()
    for c in children:
        child_terms |= extract_terms(c.get("scope", ""))
        child_terms |= extract_terms(c.get("slug", ""))

    uncovered = parent_terms - child_terms
    coverage_ratio = 1 - (len(uncovered) / len(parent_terms)) if parent_terms else 1.0

    if coverage_ratio < 0.80:
        sample = sorted(uncovered)[:5]
        flags.append({
            "category": "coverage",
            "description": (
                f"Only {coverage_ratio:.0%} of parent spec terms are covered "
                f"by children. Uncovered terms (sample): {', '.join(sample)}"
            ),
        })
    return flags


def check_budget(data: dict, num_children: int) -> list[dict]:
    """Flag if node budget is under pressure."""
    flags = []
    total = data.get("total_nodes", 0)
    max_total = data.get("max_total_nodes", 20)
    current_depth = data.get("current_depth", 0)
    max_depth = data.get("max_depth", 3)

    projected = total + num_children
    if max_total > 0 and projected / max_total > 0.80 and current_depth < max_depth - 1:
        flags.append({
            "category": "budget",
            "description": (
                f"After this split, {projected}/{max_total} nodes used "
                f"({projected/max_total:.0%}) with {max_depth - current_depth - 1} "
                f"depth levels remaining. Budget is tight."
            ),
        })
    return flags


def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": f"Invalid JSON input: {e}", "flags": []}))
        return

    children = data.get("children", [])
    parent_spec = data.get("parent_spec", "")

    flags = []
    flags.extend(check_orthogonality(children))
    flags.extend(check_coverage(parent_spec, children))
    flags.extend(check_budget(data, len(children)))

    print(json.dumps({"flags": flags}, indent=2))


if __name__ == "__main__":
    main()
