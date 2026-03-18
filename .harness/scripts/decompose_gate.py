#!/usr/bin/env python3
"""decompose_gate.py — Validate a fractal decomposition structurally.

Reads JSON from stdin with the shape:
{
  "parent_spec": "<parent spec text>",
  "children": [{"slug": "...", "scope": "...", "inputs": "...", "outputs": "..."}],
  "current_depth": N,
  "max_depth": N,
  "total_nodes": N,
  "max_total_nodes": N
}

Outputs JSON with:
  "flags": [{category, description}]         — advisory warnings
  "complexity_score": float                  — 0.0-1.0 complexity estimate
  "budget_allocation": {slug: int}           — per-child node budget
  "dependency_order": [[slug, ...], ...]     — solve waves (parallel groups)

Exit 0 always (flags are advisory, not errors).
"""

import json
import math
import re
import sys
from collections import Counter
from itertools import combinations


# ---------------------------------------------------------------------------
# Term extraction (shared)
# ---------------------------------------------------------------------------

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


def extract_term_list(text: str) -> list[str]:
    """Extract terms preserving duplicates (for TF calculation)."""
    words = re.findall(r"[a-zA-Z][a-zA-Z0-9_-]{2,}", text.lower())
    return [w for w in words if w not in STOP_WORDS]


# ---------------------------------------------------------------------------
# 1. TF-IDF Cosine Similarity (replaces raw Jaccard for orthogonality)
# ---------------------------------------------------------------------------

def tfidf_cosine_similarity(text_a: str, text_b: str, corpus: list[str]) -> float:
    """TF-IDF weighted cosine similarity between two texts.

    More accurate than Jaccard because it down-weights common terms
    (e.g., "implement", "system") that appear in every child scope,
    and up-weights discriminative terms that indicate real overlap.
    """
    if not corpus:
        return 0.0

    # Document frequency across the corpus
    df = Counter()
    for doc in corpus:
        df.update(set(extract_terms(doc)))

    n_docs = len(corpus)

    def tfidf_vec(text: str) -> dict[str, float]:
        terms = extract_term_list(text)
        if not terms:
            return {}
        tf = Counter(terms)
        return {
            t: (tf[t] / len(terms)) * math.log((n_docs + 1) / (df.get(t, 0) + 1))
            for t in set(terms)
        }

    vec_a = tfidf_vec(text_a)
    vec_b = tfidf_vec(text_b)
    common = set(vec_a) & set(vec_b)
    if not common:
        return 0.0

    dot = sum(vec_a[t] * vec_b[t] for t in common)
    mag_a = math.sqrt(sum(v ** 2 for v in vec_a.values()))
    mag_b = math.sqrt(sum(v ** 2 for v in vec_b.values()))
    return dot / (mag_a * mag_b) if mag_a and mag_b else 0.0


# ---------------------------------------------------------------------------
# 2. Dependency Cycle Detection (Kahn's algorithm / topological sort)
# ---------------------------------------------------------------------------

def detect_cycles(children: list[dict]) -> list[dict]:
    """Detect circular dependencies among children using Kahn's algorithm.

    Builds a DAG from declared inputs→outputs. If topological sort
    cannot visit all nodes, a cycle exists.
    """
    flags = []
    slugs = {c["slug"] for c in children}

    # Map output terms to producing child
    output_map: dict[str, str] = {}
    for c in children:
        for term in extract_terms(c.get("outputs", "")):
            output_map[term] = c["slug"]

    # Build adjacency: child depends on whoever produces its input terms
    graph: dict[str, list[str]] = {c["slug"]: [] for c in children}
    for c in children:
        for term in extract_terms(c.get("inputs", "")):
            producer = output_map.get(term)
            if producer and producer != c["slug"] and producer in slugs:
                if producer not in graph[c["slug"]]:
                    graph[c["slug"]].append(producer)

    # Kahn's algorithm
    in_degree = {s: 0 for s in slugs}
    for node, deps in graph.items():
        for dep in deps:
            if dep in in_degree:
                in_degree[dep] += 1

    queue = [n for n, d in in_degree.items() if d == 0]
    visited = 0
    while queue:
        node = queue.pop(0)
        visited += 1
        for dependent, deps in graph.items():
            if node in deps:
                in_degree[dependent] -= 1
                if in_degree[dependent] == 0:
                    queue.append(dependent)

    if visited < len(slugs):
        cycle_nodes = [s for s, d in in_degree.items() if d > 0]
        flags.append({
            "category": "cycle",
            "description": (
                f"Circular dependency detected among children: "
                f"{', '.join(sorted(cycle_nodes))}. "
                f"These children have mutual input/output dependencies "
                f"that cannot be resolved by sequential execution."
            ),
        })

    return flags


# ---------------------------------------------------------------------------
# 3. Solve Wave Scheduling (topological sort into parallel waves)
# ---------------------------------------------------------------------------

def compute_solve_waves(children: list[dict]) -> list[list[str]]:
    """Schedule children into parallel execution waves using topological sort.

    Returns a list of waves. Each wave is a list of slugs that can
    execute concurrently (no dependencies between them).

    Like MapReduce shuffle: group independent tasks for parallel execution.
    """
    slugs = {c["slug"] for c in children}

    # Map output terms to producing child
    output_map: dict[str, str] = {}
    for c in children:
        for term in extract_terms(c.get("outputs", "")):
            output_map[term] = c["slug"]

    # Build dependency graph: slug → set of slugs it depends on
    deps: dict[str, set[str]] = {c["slug"]: set() for c in children}
    for c in children:
        for term in extract_terms(c.get("inputs", "")):
            producer = output_map.get(term)
            if producer and producer != c["slug"] and producer in slugs:
                deps[c["slug"]].add(producer)

    # BFS layer decomposition (topological sort into waves)
    resolved: set[str] = set()
    waves: list[list[str]] = []

    while len(resolved) < len(slugs):
        # Find all nodes whose dependencies are fully resolved
        wave = [
            s for s in slugs
            if s not in resolved and deps[s].issubset(resolved)
        ]
        if not wave:
            # Remaining nodes have unresolvable deps (cycle) — force them
            remaining = [s for s in slugs if s not in resolved]
            waves.append(sorted(remaining))
            break
        waves.append(sorted(wave))
        resolved.update(wave)

    return waves


# ---------------------------------------------------------------------------
# 4. Complexity Scoring (decision tree feature scoring)
# ---------------------------------------------------------------------------

CROSS_CUTTING_TERMS = {
    "authentication", "authorization", "logging", "monitoring", "caching",
    "error-handling", "validation", "security", "testing", "deployment",
    "configuration", "middleware", "database", "migration", "api",
}


def complexity_score(spec_text: str) -> float:
    """Compute a complexity score from 0.0 (trivial) to 1.0 (very complex).

    Uses weighted feature scoring — same principle as decision tree
    split criteria. Each signal contributes proportionally.
    """
    terms = extract_terms(spec_text)
    lines = spec_text.strip().splitlines()

    signals = {
        # Length signal: longer specs tend to describe more complex tasks
        "line_count": min(1.0, len(lines) / 200),
        # Term diversity: more unique concepts = more complex
        "term_diversity": min(1.0, len(terms) / 80),
        # Cross-cutting concerns: these terms indicate architectural complexity
        "cross_cutting": min(1.0, len(terms & CROSS_CUTTING_TERMS) / 5),
        # Enumeration signal: bullet points / numbered items suggest multiple parts
        "enumeration": min(1.0, sum(1 for l in lines if re.match(r"\s*[-*•]\s|^\s*\d+\.", l)) / 10),
    }

    # Weighted average (cross-cutting is most predictive of needing decomposition)
    weights = {
        "line_count": 0.15,
        "term_diversity": 0.25,
        "cross_cutting": 0.35,
        "enumeration": 0.25,
    }

    score = sum(signals[k] * weights[k] for k in signals)
    return round(min(1.0, score), 3)


# ---------------------------------------------------------------------------
# 5. Budget Allocation (proportional distribution)
# ---------------------------------------------------------------------------

def allocate_budget(
    children: list[dict],
    remaining_budget: int,
) -> dict[str, int]:
    """Allocate remaining node budget to children proportional to complexity.

    Each child gets at least 1. Remaining budget is distributed
    proportional to each child's complexity score.
    """
    n = len(children)
    if n == 0:
        return {}

    if remaining_budget <= n:
        # Not enough budget — everyone gets 1 (or 0 if truly exhausted)
        return {c["slug"]: min(1, remaining_budget // n) for c in children}

    # Score each child's scope
    scores = [complexity_score(c.get("scope", "")) for c in children]
    total_score = sum(scores)

    # Base allocation: 1 per child
    allocation: dict[str, int] = {}
    leftover = remaining_budget - n

    for i, c in enumerate(children):
        base = 1
        if total_score > 0 and leftover > 0:
            extra = int(leftover * (scores[i] / total_score))
        else:
            extra = leftover // n
        allocation[c["slug"]] = base + extra

    return allocation


# ---------------------------------------------------------------------------
# Original checks (preserved, enhanced)
# ---------------------------------------------------------------------------

def check_orthogonality(children: list[dict]) -> list[dict]:
    """Flag pairs of children with high scope overlap.

    Uses TF-IDF cosine similarity (more accurate than raw Jaccard)
    with Jaccard as a fast pre-filter.
    """
    flags = []
    corpus = [c.get("scope", "") for c in children]
    child_terms = [(c["slug"], extract_terms(c.get("scope", ""))) for c in children]

    for (slug_a, terms_a), (slug_b, terms_b) in combinations(child_terms, 2):
        # Fast pre-filter: skip pairs with zero Jaccard overlap
        jaccard = jaccard_similarity(terms_a, terms_b)
        if jaccard < 0.10:
            continue

        # Precise check: TF-IDF cosine similarity
        scope_a = next(c.get("scope", "") for c in children if c["slug"] == slug_a)
        scope_b = next(c.get("scope", "") for c in children if c["slug"] == slug_b)
        cosine = tfidf_cosine_similarity(scope_a, scope_b, corpus)

        if cosine > 0.30:
            shared = sorted(terms_a & terms_b)[:5]
            flags.append({
                "category": "orthogonality",
                "description": (
                    f"Children '{slug_a}' and '{slug_b}' have {cosine:.0%} "
                    f"TF-IDF cosine similarity (Jaccard: {jaccard:.0%}). "
                    f"Shared terms: {', '.join(shared)}"
                ),
            })
    return flags


def jaccard_similarity(a: set[str], b: set[str]) -> float:
    """Compute Jaccard similarity between two term sets."""
    if not a and not b:
        return 0.0
    intersection = len(a & b)
    union = len(a | b)
    return intersection / union if union > 0 else 0.0


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


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": f"Invalid JSON input: {e}", "flags": []}))
        return

    children = data.get("children", [])
    parent_spec = data.get("parent_spec", "")
    remaining_budget = data.get("max_total_nodes", 20) - data.get("total_nodes", 0)

    # Flags (advisory)
    flags = []
    flags.extend(check_orthogonality(children))
    flags.extend(check_coverage(parent_spec, children))
    flags.extend(check_budget(data, len(children)))
    flags.extend(detect_cycles(children))

    # Algorithmic outputs (deterministic, used by orchestrator)
    parent_complexity = complexity_score(parent_spec)
    budget = allocate_budget(children, remaining_budget)
    waves = compute_solve_waves(children)

    result = {
        "flags": flags,
        "complexity_score": parent_complexity,
        "budget_allocation": budget,
        "dependency_order": waves,
    }

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
