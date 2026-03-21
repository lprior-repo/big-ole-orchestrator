use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LintError {
    #[error("Failed to read spec file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse YAML: {0}")]
    ParseError(#[from] serde_yaml::Error),
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Unknown rule id: {rule_id}")]
    UnknownRuleId { rule_id: String },
    #[error("Invalid severity '{severity}' for rule {rule_id}")]
    InvalidSeverity { rule_id: String, severity: String },
    #[error("Missing required field '{field}' for rule {rule_id}")]
    MissingRequiredField { rule_id: String, field: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecIdentity {
    pub id: String,
    pub version: String,
    pub status: String,
    pub author: String,
    pub created: String,
    pub updated: Option<String>,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecIntent {
    pub problem_statement: String,
    pub success_criteria: Vec<String>,
    pub non_goals: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDependency {
    pub service: String,
    pub purpose: String,
    #[serde(rename = "twin_available")]
    pub twin_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecContext {
    #[serde(rename = "system_dependencies")]
    pub system_dependencies: Vec<SystemDependency>,
    #[serde(rename = "existing_behaviors")]
    pub existing_behaviors: Option<Vec<String>>,
    pub constraints: Option<Vec<String>>,
    pub invariants: Vec<String>,
    pub glossary: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Behavior {
    pub id: String,
    pub description: String,
    pub given: Option<Vec<String>>,
    pub r#when: Option<String>,
    pub then: Vec<String>,
    #[serde(rename = "edge_cases")]
    pub edge_cases: Option<Vec<EdgeCase>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCase {
    pub id: String,
    pub r#when: String,
    pub then: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataModelEntity {
    pub name: String,
    pub fields: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataModel {
    pub entities: Option<Vec<DataModelEntity>>,
    #[serde(rename = "state_transitions")]
    pub state_transitions: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub method: String,
    pub path: String,
    #[serde(rename = "authentication")]
    pub authentication: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContract {
    pub endpoints: Option<Vec<ApiEndpoint>>,
    #[serde(rename = "events_emitted")]
    pub events_emitted: Option<Vec<serde_json::Value>>,
    #[serde(rename = "events_consumed")]
    pub events_consumed: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    #[serde(rename = "behavior_ref")]
    pub behavior_ref: Option<String>,
    pub criterion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Specification {
    pub identity: SpecIdentity,
    pub intent: SpecIntent,
    pub context: SpecContext,
    pub behaviors: Vec<Behavior>,
    #[serde(rename = "data_model")]
    pub data_model: Option<DataModel>,
    #[serde(rename = "api_contract")]
    pub api_contract: Option<ApiContract>,
    #[serde(rename = "acceptance_criteria")]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub specification: Specification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintRule {
    pub id: String,
    pub name: String,
    pub severity: String,
    pub description: String,
    #[serde(rename = "banned_phrases")]
    pub banned_phrases: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintRules {
    pub rules: Vec<LintRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    pub message: String,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    pub spec_id: String,
    pub spec_version: String,
    pub overall_score: u32,
    pub passed: bool,
    pub categories: HashMap<String, CategoryScore>,
    pub errors: Vec<LintIssue>,
    pub warnings: Vec<LintIssue>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub score: u32,
    pub details: String,
}

impl LintReport {
    #[must_use]
    pub fn new(spec_id: String, spec_version: String) -> Self {
        Self {
            spec_id,
            spec_version,
            overall_score: 0,
            passed: false,
            categories: HashMap::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn calculate_score(&mut self) {
        let (total, count) = self
            .categories
            .values()
            .fold((0u32, 0u32), |(total, count), category| {
                (total + category.score, count + 1)
            });

        self.overall_score = if count > 0 { total / count } else { 0 };
        self.passed = self.errors.is_empty() && self.overall_score >= 80;
    }
}
