//! Static analysis and linting tools for vo-engine.
//!
//! Provides linting functionality for workflow definitions and
//! Rust source code analysis.
//!
//! # Crate Overview
//!
//! This crate provides static analysis tools for the Veloxide workflow engine,
//! including linting rules for workflow definitions and Rust source code.
//!
//! # Modules
//!
//! - [`rules`] - Collection of linting rules for workflow validation
//! - [`diagnostic`] - Diagnostic types and lint codes for reporting issues
//!
//! # Rules
//!
//! The linting rules cover:
//! - Workflow structure validation
//! - Step dependency checking
//! - Signal and handler compatibility
//! - Resource quota compliance
//! - Encryption and security checks

mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, LintCode};
pub use rules::Rule;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reexport_diagnostic_from_crate_root() {
        let d = Diagnostic::new(LintCode::L002, "test");
        assert_eq!(d.message(), "test");
    }

    #[test]
    fn test_reexport_lint_code() {
        let code = LintCode::L002;
        let d = Diagnostic::new(code, "lint");
        assert!(matches!(d.code, LintCode::L002));
    }

    #[test]
    fn test_reexport_rule_trait() {
        fn assert_rule<T: Rule>() {}
        assert_rule::<crate::rules::random::RandomRule>();
    }

    #[test]
    fn test_rule_registry_from_crate_root() {
        let registry = rules::RuleRegistry::new();
        assert!(registry.rule_count() > 0);
    }

    #[test]
    fn test_lint_code_debug() {
        let code = LintCode::L002;
        let debug_str = format!("{:?}", code);
        assert!(debug_str.contains("L002"));
    }

    #[test]
    fn test_lint_code_clone() {
        let code1 = LintCode::L002;
        let code2 = code1;
        assert!(matches!(code2, LintCode::L002));
    }

    #[test]
    fn test_lint_code_partial_eq() {
        let c1 = LintCode::L002;
        let c2 = LintCode::L002;
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_diagnostic_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Diagnostic>();
        assert_sync::<Diagnostic>();
    }

    #[test]
    fn test_diagnostic_display_trait_via_debug() {
        let d = Diagnostic::new(LintCode::L002, "debug test");
        let debug_output = format!("{:?}", d);
        assert!(debug_output.contains("Diagnostic") || debug_output.contains("L002"));
    }

    #[test]
    fn test_rules_module_is_pub() {
        let _ = rules::RuleRegistry::new();
    }

    #[test]
    fn test_check_random_in_workflow_reexport() {
        let src = "fn workflow() { let id = Uuid::new_v4(); }";
        let file: syn::File = syn::parse_str(src).expect("parse failed");
        let diags = rules::check_random_in_workflow(&file);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_lint_code_eq_with_same_variants() {
        assert_eq!(LintCode::L002, LintCode::L002);
    }

    #[test]
    fn test_diagnostic_new_with_string() {
        let d = Diagnostic::new(LintCode::L002, String::from("owned string"));
        assert_eq!(d.message(), "owned string");
    }

    #[test]
    fn test_diagnostic_new_with_str() {
        let d = Diagnostic::new(LintCode::L002, "str message");
        assert_eq!(d.message(), "str message");
    }

    #[test]
    fn test_diagnostic_new_with_str_ref() {
        let msg = String::from("borrowed string");
        let d = Diagnostic::new(LintCode::L002, msg.as_str());
        assert_eq!(d.message(), "borrowed string");
    }

    #[test]
    fn test_rule_trait_id_and_name() {
        let registry = rules::RuleRegistry::new();
        for rule in registry.rules() {
            let id = rule.id();
            let name = rule.name();
            assert!(!id.is_empty(), "rule id must not be empty");
            assert!(!name.is_empty(), "rule name must not be empty");
        }
    }

    #[test]
    fn test_rule_execute_empty_file() {
        let registry = rules::RuleRegistry::new();
        let empty_src = "";
        let file: syn::File = syn::parse_str(empty_src).unwrap();
        let diags = registry.execute_all(&file);
        assert!(diags.is_empty(), "empty file should produce no diagnostics");
    }

    #[test]
    fn test_rule_execute_with_multiple_lint_issues() {
        let registry = rules::RuleRegistry::new();
        let src = "fn workflow() { Uuid::new_v4(); rand::random::<u32>(); }";
        let file: syn::File = syn::parse_str(src).unwrap();
        let diags = registry.execute_all(&file);
        assert_eq!(diags.len(), 2, "both random calls should be detected");
    }

    #[test]
    fn test_diagnostic_with_suggestion_chaining() {
        let d = Diagnostic::new(LintCode::L002, "base")
            .with_suggestion("fix it")
            .with_suggestion("overwrite");
        assert_eq!(d.suggestion(), Some("overwrite"));
    }

    #[test]
    fn test_rule_registry_default() {
        let r1 = rules::RuleRegistry::new();
        let r2 = rules::RuleRegistry::default();
        assert_eq!(r1.rule_count(), r2.rule_count());
    }

    #[test]
    fn test_rule_registry_add_rule() {
        let mut registry = rules::RuleRegistry::new();
        let initial_count = registry.rule_count();
        struct TestRule;
        impl Rule for TestRule {
            fn id(&self) -> &'static str { "T001" }
            fn name(&self) -> &'static str { "test rule" }
            fn execute(&self, _node: &syn::File) -> Vec<Diagnostic> {
                vec![]
            }
        }
        registry.add_rule(TestRule);
        assert_eq!(registry.rule_count(), initial_count + 1);
    }

    #[test]
    fn test_rule_registry_execute_all_sorted() {
        let registry = rules::RuleRegistry::new();
        let src = "fn workflow() { let id = Uuid::new_v4(); }";
        let file: syn::File = syn::parse_str(src).unwrap();
        let diags = registry.execute_all(&file);
        assert!(diags.len() >= 1);
        for diag in &diags {
            assert!(!diag.message().is_empty(), "diagnostic message must not be empty");
        }
    }

    #[test]
    fn test_diagnostic_partial_eq() {
        let d1 = Diagnostic::new(LintCode::L002, "same");
        let d2 = Diagnostic::new(LintCode::L002, "same");
        assert_eq!(d1, d2);
    }
}
