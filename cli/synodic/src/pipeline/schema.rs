use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::validate;

// ---------------------------------------------------------------------------
// Pipeline YAML schema — 4-type step system per spec 061
// ---------------------------------------------------------------------------

/// Top-level pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub config: HashMap<String, serde_yaml::Value>,
    pub steps: Vec<Step>,
}

/// A single step in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(flatten)]
    pub kind: StepKind,
    // Middleware
    #[serde(default)]
    pub retry: Option<u32>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub log: Option<String>,
    #[serde(default)]
    pub on_fail: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
}

/// The 4 step types (agent, run, branch, fan).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StepKind {
    Agent(AgentStep),
    Run(RunStep),
    Branch(BranchStep),
    Fan(FanStep),
}

/// Agent step: invoke `claude -p` with constrained tools and structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    /// Path to prompt template (Markdown).
    pub prompt: String,
    /// Allowed tools list.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Maximum agent turns.
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// Whether to run in an isolated worktree.
    #[serde(default)]
    pub isolation: Option<String>,
    /// JSON Schema for structured output.
    #[serde(default)]
    pub output_schema: Option<String>,
    /// Context variables injected into the prompt.
    #[serde(default)]
    pub context: HashMap<String, String>,
}

/// Run step: execute commands. Flags enable gate/watch behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStep {
    /// Command to execute.
    #[serde(default)]
    pub command: Option<String>,
    /// Gate group names to check (e.g. ["preflight"]).
    #[serde(default)]
    pub check: Vec<String>,
    /// File-match glob patterns (only run when changed files match).
    #[serde(default, rename = "match")]
    pub match_patterns: Vec<String>,
    /// Polling configuration for watch behavior.
    #[serde(default)]
    pub poll: Option<PollConfig>,
}

/// Polling configuration for run steps with watch behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollConfig {
    /// Polling interval in milliseconds.
    pub interval: u64,
    /// Total timeout in milliseconds.
    pub timeout: u64,
}

/// Branch step: route based on verdict (approve/rework/exhaust).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchStep {
    /// Variable reference for the input value (e.g. "steps.inspect.verdict").
    pub input: String,
    /// Step name to jump to on approval.
    pub approve: String,
    /// Step name to jump to on rework.
    pub rework: String,
    /// Max iterations before exhaust.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Step name or action on exhaust (e.g. "escalate").
    #[serde(default)]
    pub exhaust: Option<String>,
}

fn default_max_iterations() -> u32 {
    3
}

/// Fan step: collection processing (parallel/sequential/loop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanStep {
    /// Processing mode.
    pub mode: FanMode,
    /// Collection to iterate over (variable reference).
    #[serde(default)]
    pub over: Option<String>,
    /// Termination condition for loop mode.
    #[serde(default)]
    pub until: Option<String>,
    /// Max iterations for loop mode.
    #[serde(default)]
    pub max_iterations: Option<u32>,
    /// Termination config for loop mode.
    #[serde(default)]
    pub termination: Option<TerminationConfig>,
    /// Inline step template for fan-out.
    #[serde(default)]
    pub step: Option<Box<Step>>,
    /// Inline steps for loop mode.
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// Fan processing modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FanMode {
    Parallel,
    Sequential,
    Loop,
}

/// Termination configuration for fan loop mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationConfig {
    /// Number of consecutive clean iterations to terminate.
    #[serde(default)]
    pub consecutive_clean: Option<u32>,
    /// Number of plateau rounds before termination.
    #[serde(default)]
    pub plateau_rounds: Option<u32>,
}

// ---------------------------------------------------------------------------
// Pipeline parsing
// ---------------------------------------------------------------------------

impl Pipeline {
    /// Parse a pipeline from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Parse a pipeline from a YAML file.
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {}", path.display(), e))?;
        Self::from_yaml(&content)
            .map_err(|e| anyhow::anyhow!("parsing {}: {}", path.display(), e))
    }

    /// Find a step by name.
    pub fn find_step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// Validate structural correctness (see validate module for details).
    pub fn validate(&self) -> Vec<String> {
        validate::validate_pipeline(self)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_step() {
        let yaml = r#"
name: test-pipeline
description: A test pipeline
steps:
  - name: build
    type: agent
    prompt: prompts/build.md
    tools: [Read, Edit, Write, Bash]
    max_turns: 50
    isolation: worktree
    output_schema: schemas/build-report.json
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.name, "test-pipeline");
        assert_eq!(pipeline.steps.len(), 1);
        match &pipeline.steps[0].kind {
            StepKind::Agent(a) => {
                assert_eq!(a.prompt, "prompts/build.md");
                assert_eq!(a.tools.len(), 4);
                assert_eq!(a.max_turns, Some(50));
                assert_eq!(a.isolation.as_deref(), Some("worktree"));
            }
            _ => panic!("expected agent step"),
        }
    }

    #[test]
    fn test_parse_run_step() {
        let yaml = r#"
name: test
steps:
  - name: gates
    type: run
    check: [preflight]
    match: ["*.rs", "*.ts"]
    retry: 2
    on_fail: rework(build)
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        let step = &pipeline.steps[0];
        assert_eq!(step.retry, Some(2));
        assert_eq!(step.on_fail.as_deref(), Some("rework(build)"));
        match &step.kind {
            StepKind::Run(r) => {
                assert_eq!(r.check, vec!["preflight"]);
                assert_eq!(r.match_patterns, vec!["*.rs", "*.ts"]);
            }
            _ => panic!("expected run step"),
        }
    }

    #[test]
    fn test_parse_branch_step() {
        let yaml = r#"
name: test
steps:
  - name: route
    type: branch
    input: steps.inspect.verdict
    approve: create-pr
    rework: build
    max_iterations: 3
    exhaust: escalate
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        match &pipeline.steps[0].kind {
            StepKind::Branch(b) => {
                assert_eq!(b.input, "steps.inspect.verdict");
                assert_eq!(b.approve, "create-pr");
                assert_eq!(b.rework, "build");
                assert_eq!(b.max_iterations, 3);
                assert_eq!(b.exhaust.as_deref(), Some("escalate"));
            }
            _ => panic!("expected branch step"),
        }
    }

    #[test]
    fn test_parse_fan_step_parallel() {
        let yaml = r#"
name: test
steps:
  - name: solve-leaves
    type: fan
    mode: parallel
    over: pending_nodes
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        match &pipeline.steps[0].kind {
            StepKind::Fan(f) => {
                assert!(matches!(f.mode, FanMode::Parallel));
                assert_eq!(f.over.as_deref(), Some("pending_nodes"));
            }
            _ => panic!("expected fan step"),
        }
    }

    #[test]
    fn test_parse_fan_step_loop() {
        let yaml = r#"
name: test
steps:
  - name: adversarial-loop
    type: fan
    mode: loop
    until: terminated
    max_iterations: 5
    termination:
      consecutive_clean: 2
      plateau_rounds: 3
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        match &pipeline.steps[0].kind {
            StepKind::Fan(f) => {
                assert!(matches!(f.mode, FanMode::Loop));
                assert_eq!(f.until.as_deref(), Some("terminated"));
                assert_eq!(f.max_iterations, Some(5));
                let t = f.termination.as_ref().unwrap();
                assert_eq!(t.consecutive_clean, Some(2));
                assert_eq!(t.plateau_rounds, Some(3));
            }
            _ => panic!("expected fan step"),
        }
    }

    #[test]
    fn test_parse_middleware() {
        let yaml = r#"
name: test
steps:
  - name: gated
    type: run
    command: cargo check
    retry: 3
    timeout: 60000
    log: logs/gate.log
    on_fail: escalate
    condition: config.run_gates
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        let step = &pipeline.steps[0];
        assert_eq!(step.retry, Some(3));
        assert_eq!(step.timeout, Some(60000));
        assert_eq!(step.log.as_deref(), Some("logs/gate.log"));
        assert_eq!(step.on_fail.as_deref(), Some("escalate"));
        assert_eq!(step.condition.as_deref(), Some("config.run_gates"));
    }

    #[test]
    fn test_parse_config() {
        let yaml = r#"
name: test
config:
  critic_modes: [syntax, edge-cases, concurrency]
  max_rework: 3
steps:
  - name: step1
    type: run
    command: echo hello
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        assert_eq!(pipeline.config.len(), 2);
    }

    #[test]
    fn test_find_step() {
        let yaml = r#"
name: test
steps:
  - name: build
    type: agent
    prompt: build.md
  - name: inspect
    type: agent
    prompt: inspect.md
"#;
        let pipeline = Pipeline::from_yaml(yaml).unwrap();
        assert!(pipeline.find_step("build").is_some());
        assert!(pipeline.find_step("inspect").is_some());
        assert!(pipeline.find_step("nonexistent").is_none());
    }
}
