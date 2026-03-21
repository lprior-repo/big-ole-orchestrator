#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

mod code_generator;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::{ValidationResult, Workflow};
use crate::wtf_client::client::WtfClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCode {
    pub source: String,
    pub paradigm: WorkflowParadigm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub paradigm: WorkflowParadigm,
    pub graph_json: String,
    pub generated_code: GeneratedCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintError {
    pub code: String,
    pub message: String,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowParadigm {
    Fsm,
    Dag,
    Procedural,
}

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Lint errors: {0:?}")]
    LintErrors(Vec<LintError>),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Codegen error: {0}")]
    CodegenError(String),
}

#[derive(Debug)]
pub enum DeployResult {
    Success { generated_code: String },
    ValidationErrors { errors: Vec<LintError> },
    Error { message: String },
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    errors: Vec<LintError>,
}

#[derive(Debug, Deserialize)]
struct ApiSuccessResponse {
    #[serde(rename = "generated_code")]
    generated_code: String,
}

fn validate_before_deploy(workflow: &Workflow) -> ValidationResult {
    crate::graph::validate_workflow(workflow)
}

fn generate_code(
    workflow: &Workflow,
    paradigm: WorkflowParadigm,
) -> Result<GeneratedCode, DeployError> {
    code_generator::generate(workflow, paradigm).map_err(DeployError::CodegenError)
}

async fn post_definition(
    client: &WtfClient,
    definition: WorkflowDefinition,
) -> Result<String, DeployError> {
    let json = serde_json::to_string(&definition)
        .map_err(|e| DeployError::SerializationError(e.to_string()))?;

    let response = client
        .post_json("/definitions", json)
        .await
        .map_err(|e| DeployError::NetworkError(e.to_string()))?;

    match response.status().as_u16() {
        201 => {
            let body = response
                .text()
                .await
                .map_err(|e| DeployError::NetworkError(e.to_string()))?;
            let parsed: ApiSuccessResponse = serde_json::from_str(&body)
                .map_err(|e| DeployError::SerializationError(e.to_string()))?;
            Ok(parsed.generated_code)
        }
        422 => {
            let body = response
                .text()
                .await
                .map_err(|e| DeployError::NetworkError(e.to_string()))?;
            let parsed: ApiErrorResponse = serde_json::from_str(&body)
                .map_err(|e| DeployError::SerializationError(e.to_string()))?;
            Err(DeployError::LintErrors(parsed.errors))
        }
        status => Err(DeployError::NetworkError(format!("HTTP {status}"))),
    }
}

pub async fn deploy_handler(
    workflow: &Workflow,
    paradigm: WorkflowParadigm,
    client: &WtfClient,
) -> DeployResult {
    let validation_result = validate_before_deploy(workflow);
    if validation_result.has_errors() {
        let errors: Vec<LintError> = validation_result
            .issues
            .iter()
            .map(|issue| LintError {
                code: format!("WTF-L{:03}", 0),
                message: issue.message.clone(),
                node_id: issue.node_id.map(|n| n.to_string()),
            })
            .collect();
        return DeployResult::ValidationErrors { errors };
    }

    let generated_code = match generate_code(workflow, paradigm) {
        Ok(code) => code,
        Err(e) => return DeployResult::Error { message: e.to_string() },
    };

    let definition = WorkflowDefinition {
        paradigm,
        graph_json: match serde_json::to_string(workflow) {
            Ok(json) => json,
            Err(e) => {
                return DeployResult::Error {
                    message: format!("Failed to serialize workflow: {}", e)
                }
            }
        },
        generated_code,
    };

    match post_definition(client, definition).await {
        Ok(code) => DeployResult::Success { generated_code: code },
        Err(e) => DeployResult::Error {
            message: e.to_string(),
        },
    }
}

mod code_generator {
    use super::{DeployError, GeneratedCode, WorkflowParadigm, Workflow};

    pub fn generate(
        _workflow: &Workflow,
        paradigm: WorkflowParadigm,
    ) -> Result<GeneratedCode, String> {
        let source = match paradigm {
            WorkflowParadigm::Fsm => {
                r#"// Generated FSM code
struct FsmState {
    current: String,
}

impl FsmState {
    fn new() -> Self {
        Self { current: "Initial".to_string() }
    }

    fn transition(&mut self, event: &str) -> Result<(), String> {
        match (self.current.as_str(), event) {
            ("Initial", "NEXT") => { self.current = "Processing".to_string(); Ok(()) }
            ("Processing", "DONE") => { self.current = "Final".to_string(); Ok(()) }
            _ => Err(format!("Invalid transition from {} on {}", self.current, event)),
        }
    }
}
"#
            }
            WorkflowParadigm::Dag => {
                r#"// Generated DAG code
struct DagNode {
    id: String,
    dependencies: Vec<String>,
}

impl DagNode {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            dependencies: Vec::new(),
        }
    }

    fn with_deps(mut self, deps: Vec<&str>) -> Self {
        self.dependencies = deps.into_iter().map(String::from).collect();
        self
    }
}
"#
            }
            WorkflowParadigm::Procedural => {
                r#"// Generated Procedural code
struct ProceduralWorkflow {
    steps: Vec<Step>,
}

#[derive(Debug, Clone)]
struct Step {
    name: String,
    action: Box<dyn Fn() -> Result<(), String>>,
}

impl ProceduralWorkflow {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    fn add_step(&mut self, name: &str) {
        self.steps.push(Step {
            name: name.to_string(),
            action: Box::new(|| Ok(())),
        });
    }

    async fn run(&self) -> Result<(), String> {
        for step in &self.steps {
            (step.action)()?;
        }
        Ok(())
    }
}
"#
            }
        };

        Ok(GeneratedCode {
            source: source.to_string(),
            paradigm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_workflow() -> Workflow {
        Workflow::default()
    }

    fn valid_fsm_workflow() -> Workflow {
        let mut workflow = Workflow::default();
        let _ = workflow.add_node("fsm-entry", 0.0, 0.0);
        let _ = workflow.add_node("fsm-state", 100.0, 0.0);
        workflow
    }

    mod validate_before_deploy {
        use super::*;

        #[test]
        fn given_empty_workflow_when_validated_then_has_errors() {
            let workflow = empty_workflow();
            let result = validate_before_deploy(&workflow);
            assert!(result.has_errors());
        }

        #[test]
        fn given_valid_fsm_workflow_when_validated_then_no_errors() {
            let workflow = valid_fsm_workflow();
            let result = validate_before_deploy(&workflow);
            assert!(!result.has_errors());
        }
    }

    mod generate_code {
        use super::*;

        #[test]
        fn given_fsm_paradigm_generates_fsm_code() {
            let workflow = valid_fsm_workflow();
            let result = generate_code(&workflow, WorkflowParadigm::Fsm);
            assert!(result.is_ok());
            let code = result.unwrap();
            assert!(code.source.contains("FsmState"));
            assert_eq!(code.paradigm, WorkflowParadigm::Fsm);
        }

        #[test]
        fn given_dag_paradigm_generates_dag_code() {
            let workflow = valid_fsm_workflow();
            let result = generate_code(&workflow, WorkflowParadigm::Dag);
            assert!(result.is_ok());
            let code = result.unwrap();
            assert!(code.source.contains("DagNode"));
            assert_eq!(code.paradigm, WorkflowParadigm::Dag);
        }

        #[test]
        fn given_procedural_paradigm_generates_procedural_code() {
            let workflow = valid_fsm_workflow();
            let result = generate_code(&workflow, WorkflowParadigm::Procedural);
            assert!(result.is_ok());
            let code = result.unwrap();
            assert!(code.source.contains("ProceduralWorkflow"));
            assert_eq!(code.paradigm, WorkflowParadigm::Procedural);
        }
    }

    mod deploy_handler {
        use super::*;

        #[tokio::test]
        async fn given_empty_workflow_blocks_deploy() {
            let workflow = empty_workflow();
            let client = WtfClient::new("http://localhost:8080");
            let result = deploy_handler(&workflow, WorkflowParadigm::Fsm, &client).await;
            match result {
                DeployResult::ValidationErrors { .. } => {}
                other => panic!("Expected ValidationErrors, got {:?}", other),
            }
        }
    }
}
