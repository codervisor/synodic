use serde::Serialize;
use std::collections::HashMap;

use super::{jaccard_similarity, SwarmManifest};

// ---------------------------------------------------------------------------
// Swarm checkpoint — Jaccard similarity on changed file sets per spec 064
// ---------------------------------------------------------------------------

/// Checkpoint output: pairwise similarities + cross-pollination suggestions.
#[derive(Debug, Clone, Serialize)]
pub struct CheckpointOutput {
    pub similarities: HashMap<String, f64>,
    pub cross_pollination: HashMap<String, Vec<String>>,
}

/// Compute pairwise Jaccard similarities between all branches.
pub fn run(manifest: &SwarmManifest) -> CheckpointOutput {
    let mut similarities = HashMap::new();
    let mut cross_pollination: HashMap<String, Vec<String>> = HashMap::new();

    let branches = &manifest.branches;
    for i in 0..branches.len() {
        for j in (i + 1)..branches.len() {
            let sim = jaccard_similarity(&branches[i].files, &branches[j].files);
            let key = format!("{}:{}", branches[i].id, branches[j].id);
            similarities.insert(key, sim);

            // Cross-pollination: suggest unique files from each branch to the other.
            if sim > 0.0 && sim < 1.0 {
                let unique_i: Vec<String> = branches[i]
                    .files
                    .iter()
                    .filter(|f| !branches[j].files.contains(f))
                    .cloned()
                    .collect();
                let unique_j: Vec<String> = branches[j]
                    .files
                    .iter()
                    .filter(|f| !branches[i].files.contains(f))
                    .cloned()
                    .collect();

                if !unique_i.is_empty() {
                    cross_pollination
                        .entry(branches[j].id.clone())
                        .or_default()
                        .extend(unique_i);
                }
                if !unique_j.is_empty() {
                    cross_pollination
                        .entry(branches[i].id.clone())
                        .or_default()
                        .extend(unique_j);
                }
            }
        }
    }

    CheckpointOutput {
        similarities,
        cross_pollination,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::SwarmBranch;

    #[test]
    fn test_checkpoint_identical_branches() {
        let manifest = SwarmManifest {
            id: "test".to_string(),
            branches: vec![
                SwarmBranch {
                    id: "a".to_string(),
                    strategy: "strategy-1".to_string(),
                    files: vec!["src/main.rs".to_string()],
                    status: "active".to_string(),
                },
                SwarmBranch {
                    id: "b".to_string(),
                    strategy: "strategy-2".to_string(),
                    files: vec!["src/main.rs".to_string()],
                    status: "active".to_string(),
                },
            ],
        };
        let result = run(&manifest);
        assert!((result.similarities["a:b"] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_checkpoint_disjoint_branches() {
        let manifest = SwarmManifest {
            id: "test".to_string(),
            branches: vec![
                SwarmBranch {
                    id: "a".to_string(),
                    strategy: "s1".to_string(),
                    files: vec!["src/a.rs".to_string()],
                    status: "active".to_string(),
                },
                SwarmBranch {
                    id: "b".to_string(),
                    strategy: "s2".to_string(),
                    files: vec!["src/b.rs".to_string()],
                    status: "active".to_string(),
                },
            ],
        };
        let result = run(&manifest);
        assert!((result.similarities["a:b"]).abs() < 1e-10);
    }

    #[test]
    fn test_checkpoint_cross_pollination() {
        let manifest = SwarmManifest {
            id: "test".to_string(),
            branches: vec![
                SwarmBranch {
                    id: "a".to_string(),
                    strategy: "s1".to_string(),
                    files: vec!["shared.rs".to_string(), "unique-a.rs".to_string()],
                    status: "active".to_string(),
                },
                SwarmBranch {
                    id: "b".to_string(),
                    strategy: "s2".to_string(),
                    files: vec!["shared.rs".to_string(), "unique-b.rs".to_string()],
                    status: "active".to_string(),
                },
            ],
        };
        let result = run(&manifest);
        assert!(result.cross_pollination.contains_key("a"));
        assert!(result.cross_pollination.contains_key("b"));
        assert!(result.cross_pollination["a"].contains(&"unique-b.rs".to_string()));
        assert!(result.cross_pollination["b"].contains(&"unique-a.rs".to_string()));
    }
}
