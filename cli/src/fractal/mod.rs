pub mod decompose;
pub mod prune;
pub mod reunify;
pub mod schedule;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Shared types for the fractal algorithmic spine
// ---------------------------------------------------------------------------

/// A child node produced by decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Child {
    pub slug: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub boundaries: String,
    #[serde(default)]
    pub inputs: String,
    #[serde(default)]
    pub outputs: String,
}

/// Input to the decompose gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposeInput {
    pub parent_spec: String,
    pub children: Vec<Child>,
    #[serde(default = "default_depth")]
    pub current_depth: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub total_nodes: usize,
    #[serde(default = "default_max_nodes")]
    pub max_total_nodes: usize,
}

fn default_depth() -> usize {
    0
}
fn default_max_depth() -> usize {
    3
}
fn default_max_nodes() -> usize {
    20
}

/// An advisory flag from structural validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    pub category: String,
    pub description: String,
}

/// Full output of the decompose gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposeOutput {
    pub flags: Vec<Flag>,
    pub complexity_score: f64,
    pub budget_allocation: HashMap<String, usize>,
    pub dependency_order: Vec<Vec<String>>,
}

/// A node in the fractal manifest tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub slug: String,
    #[serde(default)]
    pub depth: usize,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub boundaries: String,
    #[serde(default)]
    pub inputs: String,
    #[serde(default)]
    pub outputs: String,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub branch: String,
}

/// Fractal manifest (subset needed by algorithmic spine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tree: HashMap<String, TreeNode>,
}

// ---------------------------------------------------------------------------
// Shared NLP utilities
// ---------------------------------------------------------------------------

/// Stop words filtered during term extraction.
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "from", "are", "was", "were",
    "been", "have", "has", "had", "not", "but", "its", "can", "will", "should",
    "must", "may", "each", "all", "any", "into", "when", "how", "what", "which",
    "their", "them", "they", "you", "your", "about", "also", "does", "using",
    "used", "use", "none",
];

/// Extract lowercase alphanumeric terms (>=3 chars), filtering stop words.
pub fn extract_terms(text: &str) -> HashSet<String> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    let re = regex::Regex::new(r"[a-zA-Z][a-zA-Z0-9_-]{2,}").unwrap();
    re.find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .filter(|w| !stop.contains(w.as_str()))
        .collect()
}

/// Extract terms preserving duplicates (for TF calculation).
pub fn extract_term_list(text: &str) -> Vec<String> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    let re = regex::Regex::new(r"[a-zA-Z][a-zA-Z0-9_-]{2,}").unwrap();
    re.find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .filter(|w| !stop.contains(w.as_str()))
        .collect()
}

/// Jaccard similarity between two term sets.
pub fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_terms() {
        let terms = extract_terms("Build an authentication system with OAuth2");
        assert!(terms.contains("build"));
        assert!(terms.contains("authentication"));
        assert!(terms.contains("system"));
        assert!(terms.contains("oauth2"));
        // Stop words filtered
        assert!(!terms.contains("with"));
        // Short words filtered
        assert!(!terms.contains("an"));
    }

    #[test]
    fn test_jaccard_similarity() {
        let a: HashSet<String> = ["auth", "system", "oauth"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["auth", "system", "data"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let sim = jaccard_similarity(&a, &b);
        // intersection=2, union=4 → 0.5
        assert!((sim - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_jaccard_empty() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert_eq!(jaccard_similarity(&a, &b), 0.0);
    }
}
