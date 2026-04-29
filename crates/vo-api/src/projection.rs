//! Operator projection service for ADR-025 dual-representation privacy model.
//!
//! Provides redaction of workflow payloads in API/SSE query paths.
//! Default view is the operator projection (redacted); privileged callers
//! can request the canonical view via `?view=canonical`.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use vo_types::{
    apply_redaction, OperatorProjection, RedactionKind, RedactionPolicy, RedactionRule,
};

use std::sync::Arc;

/// Requested view mode for API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// Redacted operator projection (default per ADR-025).
    #[default]
    Projected,
    /// Full canonical data for privileged forensic access.
    Canonical,
}

impl std::str::FromStr for ViewMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "projected" | "" => Ok(ViewMode::Projected),
            "canonical" => Ok(ViewMode::Canonical),
            _ => Err(format!(
                "invalid view mode: {s}, expected 'projected' or 'canonical'"
            )),
        }
    }
}

/// Result of applying an operator projection to an event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedPayload {
    /// The (possibly redacted) payload JSON.
    pub payload: serde_json::Value,
    /// Fields that were redacted during projection.
    pub redacted_fields: Vec<Vec<String>>,
}

/// Projection service holding per-workflow-type redaction policies.
///
/// Thread-safe via `RwLock`. Lookups are fast (read path).
/// Policy registration is infrequent (configuration/startup).
#[derive(Debug, Clone)]
pub struct ProjectionService {
    policies: Arc<RwLock<HashMap<String, RedactionPolicy>>>,
}

impl ProjectionService {
    /// Create a new projection service with no policies registered.
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a redaction policy for a workflow type.
    ///
    /// Replaces any existing policy for the same workflow type.
    pub fn register_policy(&self, policy: RedactionPolicy) {
        let wt = policy.workflow_type.clone();
        let mut guard = self.policies.write().expect("projection policies lock poisoned");
        guard.insert(wt, policy);
    }

    /// Look up the redaction policy for a workflow type.
    pub fn get_policy(&self, workflow_type: &str) -> Option<RedactionPolicy> {
        let guard = self.policies.read().expect("projection policies lock poisoned");
        guard.get(workflow_type).cloned()
    }

    /// Apply the operator projection to a payload for a given workflow type.
    ///
    /// If no policy is registered for the workflow type, the payload passes
    /// through unchanged with an empty redacted_fields list.
    pub fn project_payload(
        &self,
        workflow_type: &str,
        payload: &serde_json::Value,
    ) -> ProjectedPayload {
        match self.get_policy(workflow_type) {
            Some(policy) => {
                let (projected, redacted_fields) =
                    apply_redaction(payload, &policy.redaction_rules);
                ProjectedPayload {
                    payload: projected,
                    redacted_fields,
                }
            }
            None => ProjectedPayload {
                payload: payload.clone(),
                redacted_fields: Vec::new(),
            },
        }
    }

    /// Apply the operator projection, producing an `OperatorProjection` struct.
    pub fn project(
        &self,
        workflow_id: &str,
        workflow_type: &str,
        payload: &serde_json::Value,
    ) -> OperatorProjection {
        let projected = self.project_payload(workflow_type, payload);
        OperatorProjection::new(
            workflow_id.to_string(),
            workflow_type.to_string(),
            projected.payload,
            projected.redacted_fields,
        )
    }
}

impl Default for ProjectionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: build a default redaction policy that removes common PII fields.
pub fn default_pii_policy(workflow_type: &str) -> RedactionPolicy {
    RedactionPolicy::new(
        workflow_type.to_string(),
        vec![
            RedactionRule::new(vec!["ssn".to_string()], RedactionKind::Remove),
            RedactionRule::new(vec!["email".to_string()], RedactionKind::Hash),
            RedactionRule::new(vec!["phone".to_string()], RedactionKind::Hash),
            RedactionRule::new(vec!["credit_card".to_string()], RedactionKind::Remove),
            RedactionRule::new(vec!["password".to_string()], RedactionKind::Remove),
            RedactionRule::new(vec!["api_key".to_string()], RedactionKind::Remove),
            RedactionRule::new(vec!["token".to_string()], RedactionKind::Remove),
            RedactionRule::new(vec!["secret".to_string()], RedactionKind::Remove),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_mode_default_is_projected() {
        assert_eq!(ViewMode::default(), ViewMode::Projected);
    }

    #[test]
    fn view_mode_parses_projected() {
        assert_eq!("projected".parse::<ViewMode>().unwrap(), ViewMode::Projected);
    }

    #[test]
    fn view_mode_parses_empty_as_projected() {
        assert_eq!("".parse::<ViewMode>().unwrap(), ViewMode::Projected);
    }

    #[test]
    fn view_mode_parses_canonical() {
        assert_eq!("canonical".parse::<ViewMode>().unwrap(), ViewMode::Canonical);
    }

    #[test]
    fn view_mode_rejects_invalid() {
        assert!("raw".parse::<ViewMode>().is_err());
    }

    #[test]
    fn projection_service_no_policy_passes_through() {
        let svc = ProjectionService::new();
        let payload = serde_json::json!({"ssn": "123-45-6789"});
        let result = svc.project_payload("payments", &payload);
        assert_eq!(result.payload, payload);
        assert!(result.redacted_fields.is_empty());
    }

    #[test]
    fn projection_service_applies_policy() {
        let svc = ProjectionService::new();
        let policy = RedactionPolicy::new(
            "payments".to_string(),
            vec![RedactionRule::new(
                vec!["ssn".to_string()],
                RedactionKind::Remove,
            )],
        );
        svc.register_policy(policy);

        let payload = serde_json::json!({"ssn": "123-45-6789", "name": "Alice"});
        let result = svc.project_payload("payments", &payload);

        assert!(result.payload.get("ssn").is_none(), "ssn should be removed");
        assert_eq!(result.payload["name"], "Alice");
        assert_eq!(result.redacted_fields.len(), 1);
    }

    #[test]
    fn projection_service_replaces_policy() {
        let svc = ProjectionService::new();
        svc.register_policy(RedactionPolicy::new(
            "payments".to_string(),
            vec![RedactionRule::new(
                vec!["email".to_string()],
                RedactionKind::Remove,
            )],
        ));
        svc.register_policy(RedactionPolicy::new(
            "payments".to_string(),
            vec![RedactionRule::new(
                vec!["email".to_string()],
                RedactionKind::ReplaceWith("[REDACTED]".to_string()),
            )],
        ));

        let payload = serde_json::json!({"email": "a@b.com"});
        let result = svc.project_payload("payments", &payload);
        assert_eq!(result.payload["email"], "[REDACTED]");
    }

    #[test]
    fn default_pii_policy_removes_ssn() {
        let policy = default_pii_policy("test");
        let payload = serde_json::json!({"ssn": "123", "name": "ok"});
        let (result, redacted) = apply_redaction(&payload, &policy.redaction_rules);
        assert!(result.get("ssn").is_none());
        assert_eq!(result["name"], "ok");
        assert_eq!(redacted.len(), 1);
    }

    #[test]
    fn default_pii_policy_hashes_email() {
        let policy = default_pii_policy("test");
        let payload = serde_json::json!({"email": "user@example.com"});
        let (result, _) = apply_redaction(&payload, &policy.redaction_rules);
        let email = result["email"].as_str().unwrap();
        assert!(email.starts_with("HASH"), "email should be hashed: {email}");
    }

    #[test]
    fn projection_service_project_returns_operator_projection() {
        let svc = ProjectionService::new();
        svc.register_policy(RedactionPolicy::new(
            "wf".to_string(),
            vec![RedactionRule::new(
                vec!["secret".to_string()],
                RedactionKind::Remove,
            )],
        ));

        let payload = serde_json::json!({"secret": "abc", "public": "ok"});
        let proj = svc.project("inst-1", "wf", &payload);

        assert_eq!(proj.workflow_id(), "inst-1");
        assert_eq!(proj.workflow_type(), "wf");
        assert!(proj.projection_json().get("secret").is_none());
        assert_eq!(proj.projection_json()["public"], "ok");
        assert_eq!(proj.redacted_fields().len(), 1);
    }
}
