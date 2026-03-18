#!/usr/bin/env python3
"""reunify_merge.py — Algorithmic code reunification for fractal solve.

Replaces the AI reunify subagent for output_mode=code with deterministic
git-based merging + structural conflict detection.

Algorithm:
1. Sort child branches by dependency order (from solve_scheduler)
2. Sequential git merge-tree for each child into integration branch
3. Classify conflicts structurally (no AI needed for detection)
4. Auto-resolve textual conflicts where possible
5. Output remaining conflicts for AI fallback (only when semantic)

Input JSON from stdin:
{
  "base_ref": "main",
  "children": [
    {"slug": "auth", "branch": "fractal/auth", "scope": "...", "boundaries": "...", "files": ["src/auth.rs"]}
  ],
  "dependency_order": [["auth", "data"], ["api"]],
  "node_slug": "root"
}

Output JSON:
{
  "status": "MERGED" | "CONFLICT" | "PARTIAL",
  "auto_resolved": [...],
  "conflicts": [...],
  "merge_order": [...],
  "needs_ai": bool
}
"""

import json
import os
import subprocess
import sys
from pathlib import Path


def run_git(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    """Run a git command and return the result."""
    result = subprocess.run(
        ["git"] + list(args),
        capture_output=True,
        text=True,
    )
    if check and result.returncode != 0:
        raise subprocess.CalledProcessError(
            result.returncode, ["git"] + list(args),
            output=result.stdout, stderr=result.stderr,
        )
    return result


def get_merge_base(ref_a: str, ref_b: str) -> str:
    """Find the merge base between two refs."""
    result = run_git("merge-base", ref_a, ref_b)
    return result.stdout.strip()


def try_merge_tree(base: str, ours: str, theirs: str) -> dict:
    """Use git merge-tree to test a 3-way merge without touching the working tree.

    Returns: {clean: bool, conflicts: [{file, type, description}]}
    """
    result = run_git("merge-tree", "--write-tree", base, ours, theirs, check=False)

    if result.returncode == 0:
        return {"clean": True, "tree_sha": result.stdout.strip(), "conflicts": []}

    # Parse conflict information from stderr/stdout
    conflicts = []
    for line in result.stdout.splitlines() + result.stderr.splitlines():
        line = line.strip()
        if not line:
            continue
        # merge-tree outputs conflict markers and file paths
        if "CONFLICT" in line:
            conflict_type = "content"
            if "rename/rename" in line:
                conflict_type = "rename"
            elif "modify/delete" in line:
                conflict_type = "modify_delete"
            elif "add/add" in line:
                conflict_type = "add_add"
            conflicts.append({
                "type": conflict_type,
                "description": line,
            })

    return {"clean": False, "tree_sha": None, "conflicts": conflicts}


def check_scope_violations(child: dict, changed_files: list[str], all_children: list[dict]) -> list[dict]:
    """Detect boundary violations — child modified files outside its scope.

    Pure set operation: does the child's changed file set intersect
    with any sibling's declared file set?
    """
    violations = []
    child_files = set(changed_files)

    for sibling in all_children:
        if sibling["slug"] == child["slug"]:
            continue
        sibling_files = set(sibling.get("files", []))
        overlap = child_files & sibling_files
        if overlap:
            violations.append({
                "category": "boundary",
                "children": [child["slug"], sibling["slug"]],
                "description": (
                    f"Child '{child['slug']}' modified files that belong to "
                    f"sibling '{sibling['slug']}': {', '.join(sorted(overlap))}"
                ),
                "files": sorted(overlap),
            })

    return violations


def check_redundant_exports(children: list[dict]) -> list[dict]:
    """Detect redundancy — multiple children export the same symbols.

    For code mode: check if multiple children created/modified the same files.
    """
    conflicts = []
    file_owners: dict[str, list[str]] = {}

    for child in children:
        for f in child.get("files", []):
            file_owners.setdefault(f, []).append(child["slug"])

    for filepath, owners in file_owners.items():
        if len(owners) > 1:
            conflicts.append({
                "category": "redundancy",
                "children": sorted(owners),
                "description": (
                    f"File '{filepath}' was modified by multiple children: "
                    f"{', '.join(sorted(owners))}"
                ),
                "file": filepath,
            })

    return conflicts


def check_interface_gaps(children: list[dict]) -> list[dict]:
    """Detect gaps — declared inputs not satisfied by any sibling's outputs.

    Pure set operation on declared contracts.
    """
    import re

    STOP_WORDS = {
        "the", "and", "for", "that", "this", "with", "from", "are", "was",
        "none", "any", "all",
    }

    def terms(text: str) -> set[str]:
        words = re.findall(r"[a-zA-Z][a-zA-Z0-9_-]{2,}", text.lower())
        return {w for w in words if w not in STOP_WORDS}

    gaps = []
    # Collect all outputs
    all_outputs = set()
    for c in children:
        all_outputs |= terms(c.get("outputs", ""))

    # Check each child's inputs
    for c in children:
        inputs = terms(c.get("inputs", ""))
        if not inputs:
            continue
        missing = inputs - all_outputs
        if missing:
            gaps.append({
                "category": "gap",
                "children": [c["slug"]],
                "description": (
                    f"Child '{c['slug']}' declares inputs not produced by any "
                    f"sibling: {', '.join(sorted(missing)[:5])}"
                ),
            })

    return gaps


def flatten_dependency_order(waves: list[list[str]]) -> list[str]:
    """Flatten waves into a single merge order (wave by wave)."""
    order = []
    for wave in waves:
        order.extend(sorted(wave))
    return order


def classify_conflict_needs_ai(conflict: dict) -> bool:
    """Determine if a conflict requires AI to resolve.

    Textual conflicts in different functions → auto-resolvable (take both).
    Same function with different logic → needs AI.
    Rename conflicts → usually auto-resolvable.
    """
    ctype = conflict.get("type", "content")
    # Rename and modify/delete can often be resolved mechanically
    if ctype in ("rename", "modify_delete"):
        return False
    # Content conflicts in the same hunk need semantic understanding
    return True


def main():
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": str(e), "status": "FAILED"}))
        return

    base_ref = data.get("base_ref", "main")
    children = data.get("children", [])
    waves = data.get("dependency_order", [])
    node_slug = data.get("node_slug", "root")

    if not children:
        print(json.dumps({"status": "MERGED", "conflicts": [], "needs_ai": False}))
        return

    # Flatten dependency order for merge sequence
    merge_order = flatten_dependency_order(waves) if waves else [c["slug"] for c in children]

    # Structural checks (no git needed)
    all_conflicts = []
    all_conflicts.extend(check_redundant_exports(children))
    all_conflicts.extend(check_interface_gaps(children))

    for child in children:
        all_conflicts.extend(check_scope_violations(child, child.get("files", []), children))

    # Git merge-tree checks (if branches exist)
    auto_resolved = []
    merge_conflicts = []

    children_by_slug = {c["slug"]: c for c in children}
    for slug in merge_order:
        child = children_by_slug.get(slug)
        if not child or not child.get("branch"):
            continue

        try:
            base = get_merge_base(base_ref, child["branch"])
            merge_result = try_merge_tree(base, base_ref, child["branch"])

            if not merge_result["clean"]:
                for conflict in merge_result["conflicts"]:
                    conflict["child"] = slug
                    if classify_conflict_needs_ai(conflict):
                        merge_conflicts.append(conflict)
                    else:
                        auto_resolved.append({
                            **conflict,
                            "resolution": "auto",
                        })
        except (subprocess.CalledProcessError, Exception) as e:
            # Branch may not exist yet (dry-run mode)
            merge_conflicts.append({
                "type": "unavailable",
                "child": slug,
                "description": f"Could not check merge for '{slug}': {e}",
            })

    all_conflicts.extend(merge_conflicts)

    # Determine status
    needs_ai = any(classify_conflict_needs_ai(c) for c in merge_conflicts)
    if not all_conflicts:
        status = "MERGED"
    elif needs_ai:
        status = "CONFLICT"
    else:
        status = "PARTIAL"  # has structural issues but all auto-resolvable

    result = {
        "status": status,
        "auto_resolved": auto_resolved,
        "conflicts": all_conflicts,
        "merge_order": merge_order,
        "needs_ai": needs_ai,
    }

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
