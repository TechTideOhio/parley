use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseSeverity {
    Blocker,
    Graded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteMeta {
    pub id: String,
    pub version: String,
    pub description: String,
    pub max_turns_default: u32,
    pub min_suite_goal_score: f64,
    pub min_suite_efficiency: f64,
    pub min_suite_memory_retention: f64,
    #[serde(default)]
    pub fail_on_policy_violation: bool,
    #[serde(default)]
    pub fail_on_sycophancy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithheldConstraint {
    pub id: String,
    pub description: String,
    pub unlock_on_assistant_regex: String,
    pub reveal_utterance: String,
    pub must_hold_regex: String,
    pub violation_regex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdversarialPushback {
    pub inject_at_turn: u32,
    pub utterance: String,
    pub forbidden_capitulation_regex: String,
    pub required_boundary_regex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCriterion {
    pub id: String,
    pub description: String,
    pub required_regex: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyGuard {
    pub id: String,
    pub forbidden_regex: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSpec {
    pub id: String,
    pub role: String,
    pub tone: String,
    pub hidden_goal: String,
    pub opening_prompt: String,
    #[serde(default)]
    pub withheld_constraints: Vec<WithheldConstraint>,
    #[serde(default)]
    pub adversarial_pushbacks: Vec<AdversarialPushback>,
    #[serde(default)]
    pub goal_criteria: Vec<GoalCriterion>,
    #[serde(default)]
    pub policy_guards: Vec<PolicyGuard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSpec {
    pub id: String,
    pub title: String,
    pub stratum: String,
    pub severity: CaseSeverity,
    pub optimal_turns: u32,
    pub max_turns: u32,
    pub min_goal_score: f64,
    pub min_memory_retention: f64,
    pub transcript_path: String,
    pub persona: PersonaSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteSpec {
    pub suite: SuiteMeta,
    pub strata: HashMap<String, f64>,
    pub cases: Vec<CaseSpec>,
}

impl SuiteSpec {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let suite: Self = toml::from_str(&content)?;
        Ok(suite)
    }

    pub fn validate(&self) -> Result<(), anyhow::Error> {
        if self.cases.is_empty() {
            anyhow::bail!("suite must declare at least one evaluation case");
        }
        let total_weight: f64 = self.strata.values().sum();
        if (total_weight - 1.0).abs() > 0.001 {
            anyhow::bail!(
                "strata weights must sum to 1.0 (found sum {:.4})",
                total_weight
            );
        }
        for case in &self.cases {
            if !self.strata.contains_key(&case.stratum) {
                anyhow::bail!(
                    "case '{}' refers to undeclared stratum '{}'",
                    case.id,
                    case.stratum
                );
            }
            if case.optimal_turns > case.max_turns {
                anyhow::bail!(
                    "case '{}' optimal_turns ({}) cannot exceed max_turns ({})",
                    case.id,
                    case.optimal_turns,
                    case.max_turns
                );
            }
        }
        Ok(())
    }
}
