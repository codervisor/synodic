use serde::Serialize;

use super::{jaccard_similarity, SwarmManifest};

// ---------------------------------------------------------------------------
// Swarm prune — convergence detection per spec 064
// ---------------------------------------------------------------------------

/// Prune output: branches to remove and branches that survive.
#[derive(Debug, Clone, Serialize)]
pub struct PruneOutput {
    pub pruned: Vec<String>,
    pub surviving: Vec<String>,
}

/// Input for the prune command.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PruneInput {
    #[serde(flatten)]
    pub manifest: SwarmManifest,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_threshold() -> f64 {
    0.8
}

/// Prune branches with similarity > threshold.
/// Enforces minimum 2 survivors per spec 064.
pub fn run(input: &PruneInput) -> PruneOutput {
    let branches = &input.manifest.branches;
    let threshold = input.threshold;

    if branches.len() <= 2 {
        return PruneOutput {
            pruned: vec![],
            surviving: branches.iter().map(|b| b.id.clone()).collect(),
        };
    }

    // Find branches to prune: if two branches are too similar, prune the later one.
    let mut pruned_set = std::collections::HashSet::new();

    for i in 0..branches.len() {
        if pruned_set.contains(&i) {
            continue;
        }
        for j in (i + 1)..branches.len() {
            if pruned_set.contains(&j) {
                continue;
            }
            let sim = jaccard_similarity(&branches[i].files, &branches[j].files);
            if sim > threshold {
                pruned_set.insert(j);
            }
        }
    }

    // Enforce minimum 2 survivors.
    let surviving_count = branches.len() - pruned_set.len();
    if surviving_count < 2 {
        // Remove pruned entries until we have at least 2 survivors.
        let mut to_restore: Vec<usize> = pruned_set.iter().copied().collect();
        to_restore.sort();
        while branches.len() - pruned_set.len() < 2 && !to_restore.is_empty() {
            let restore = to_restore.pop().unwrap();
            pruned_set.remove(&restore);
        }
    }

    let pruned: Vec<String> = pruned_set
        .iter()
        .map(|&i| branches[i].id.clone())
        .collect();
    let surviving: Vec<String> = branches
        .iter()
        .enumerate()
        .filter(|(i, _)| !pruned_set.contains(i))
        .map(|(_, b)| b.id.clone())
        .collect();

    PruneOutput { pruned, surviving }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::SwarmBranch;

    #[test]
    fn test_prune_convergent() {
        let input = PruneInput {
            manifest: super::SwarmManifest {
                id: "test".to_string(),
                branches: vec![
                    SwarmBranch {
                        id: "a".to_string(),
                        strategy: "s1".to_string(),
                        files: vec!["main.rs".to_string(), "lib.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "b".to_string(),
                        strategy: "s2".to_string(),
                        files: vec!["main.rs".to_string(), "lib.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "c".to_string(),
                        strategy: "s3".to_string(),
                        files: vec!["other.rs".to_string()],
                        status: "active".to_string(),
                    },
                ],
            },
            threshold: 0.8,
        };
        let result = run(&input);
        // a and b are identical (similarity=1.0 > 0.8), one should be pruned.
        assert_eq!(result.pruned.len(), 1);
        assert!(result.surviving.len() >= 2);
    }

    #[test]
    fn test_prune_min_survivors() {
        let input = PruneInput {
            manifest: super::SwarmManifest {
                id: "test".to_string(),
                branches: vec![
                    SwarmBranch {
                        id: "a".to_string(),
                        strategy: "s1".to_string(),
                        files: vec!["main.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "b".to_string(),
                        strategy: "s2".to_string(),
                        files: vec!["main.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "c".to_string(),
                        strategy: "s3".to_string(),
                        files: vec!["main.rs".to_string()],
                        status: "active".to_string(),
                    },
                ],
            },
            threshold: 0.8,
        };
        let result = run(&input);
        // All converge, but min 2 survivors enforced.
        assert!(result.surviving.len() >= 2);
    }

    #[test]
    fn test_prune_disjoint() {
        let input = PruneInput {
            manifest: super::SwarmManifest {
                id: "test".to_string(),
                branches: vec![
                    SwarmBranch {
                        id: "a".to_string(),
                        strategy: "s1".to_string(),
                        files: vec!["a.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "b".to_string(),
                        strategy: "s2".to_string(),
                        files: vec!["b.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "c".to_string(),
                        strategy: "s3".to_string(),
                        files: vec!["c.rs".to_string()],
                        status: "active".to_string(),
                    },
                ],
            },
            threshold: 0.8,
        };
        let result = run(&input);
        // All disjoint, none pruned.
        assert!(result.pruned.is_empty());
        assert_eq!(result.surviving.len(), 3);
    }

    #[test]
    fn test_prune_malformed_input() {
        // Only 2 branches — should never prune below 2.
        let input = PruneInput {
            manifest: super::SwarmManifest {
                id: "test".to_string(),
                branches: vec![
                    SwarmBranch {
                        id: "a".to_string(),
                        strategy: "s1".to_string(),
                        files: vec!["main.rs".to_string()],
                        status: "active".to_string(),
                    },
                    SwarmBranch {
                        id: "b".to_string(),
                        strategy: "s2".to_string(),
                        files: vec!["main.rs".to_string()],
                        status: "active".to_string(),
                    },
                ],
            },
            threshold: 0.8,
        };
        let result = run(&input);
        assert!(result.pruned.is_empty());
        assert_eq!(result.surviving.len(), 2);
    }
}
