//! Collection of linting rules for workflow validation.
//!
//! Each rule module is independently testable and focuses on a specific
//! category of workflow issues. Rules are stateless and do not share
//! mutable state between invocations.
//!
//! # Available Rules
//!
//! - [`random`] — L002: Detects non-deterministic random calls
//! - [`retry_policy`] — L003-L006: Validates retry policy values are within safe bounds
//!
//! # Rule Architecture
//!
//! All rules implement the [`Rule`] trait which ensures:
//! - Statelessness: Rules do not share mutable state between invocations
//! - Independence: Each rule can be tested in isolation
//! - Composability: Multiple rules can run concurrently without interference

pub mod random;
pub mod retry_policy;

pub use random::check_random_in_workflow;
pub use retry_policy::check_retry_policy_bounds;

/// Trait for all linting rules.
///
/// This trait ensures rules are stateless and can be executed independently
/// without sharing mutable state. Each rule invocation creates fresh state
/// via [`Rule::execute`].
pub trait Rule: Send + Sync {
    /// The unique identifier for this rule.
    fn id(&self) -> &'static str;

    /// The display name of this rule.
    fn name(&self) -> &'static str;

    /// Execute the rule on the given AST.
    ///
    /// This method must be stateless - no shared mutable state is allowed.
    /// Each call creates fresh local state.
    fn execute(&self, node: &syn::File) -> Vec<crate::Diagnostic>;
}

/// Registry for linting rules.
///
/// Provides a centralized way to discover and execute all registered rules.
/// Rules are stored as trait objects for dynamic dispatch.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleRegistry {
    /// Create a new registry with all built-in rules.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self { rules: Vec::new() };
        registry.add_rule(random::RandomRule);
        registry
    }

    /// Add a rule to the registry.
    pub fn add_rule(&mut self, rule: impl Rule + 'static) {
        self.rules.push(Box::new(rule));
    }

    /// Execute all rules on the given AST.
    ///
    /// Returns a vector of diagnostics from all rules.
    #[must_use]
    pub fn execute_all(&self, file: &syn::File) -> Vec<crate::Diagnostic> {
        self.rules
            .iter()
            .flat_map(|rule| rule.execute(file))
            .collect()
    }

    /// Get the number of registered rules.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Get an iterator over all registered rules.
    #[must_use]
    pub fn rules(&self) -> impl Iterator<Item = &dyn Rule> {
        self.rules.iter().map(|r| r.as_ref())
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_has_rules() {
        let registry = RuleRegistry::new();
        assert!(registry.rule_count() > 0);
    }

    #[test]
    fn test_registry_execute_all_collects_diagnostics() {
        let registry = RuleRegistry::new();
        let src = r#"
            fn workflow() {
                let id = Uuid::new_v4();
            }
        "#;
        let file: syn::File = syn::parse_str(src).unwrap();
        let diags = registry.execute_all(&file);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_registry_multiple_rules_no_state_sharing() {
        let registry = RuleRegistry::new();
        let src = r#"
            fn workflow() {
                let id = Uuid::new_v4();
            }
        "#;
        let file: syn::File = syn::parse_str(src).unwrap();

        // Execute multiple times - results should be identical
        let diags1 = registry.execute_all(&file);
        let diags2 = registry.execute_all(&file);
        let diags3 = registry.execute_all(&file);

        assert_eq!(diags1, diags2);
        assert_eq!(diags2, diags3);
    }
}
