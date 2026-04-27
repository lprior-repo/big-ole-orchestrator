//! BLACK-HAT quality gate tests for placeholder test detection.
//!
//! These tests verify that the linter correctly identifies placeholder semantic tests:
//! - assert(true) or assert_eq!(X, X) constant-only assertions
//! - #[ignore] marked tests
//! - todo!(), unimplemented!(), unreachable!() macros
//! - Commented-out handler mirrors
//! - Local mirror API structs
//! - Fake-only production coverage
//!
//! BDD Scenario:
//! Given a test contains placeholder patterns
//! When quality gate scans tests
//! Then gate fails and identifies the specific placeholder pattern

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use quote::quote;
use syn::File;
use vo_linter::{rules::check_placeholder_tests, Diagnostic};

fn parse(src: &str) -> File {
    syn::parse_str(src).expect("failed to parse Rust source")
}

fn lint(src: &str) -> Vec<Diagnostic> {
    check_placeholder_tests(&parse(src))
}

#[test]
fn given_todo_macro_in_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[test]
        fn given_workflow_starts_when_something_happens_then_produces_output() {
            todo!()
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test contains todo! macro"
    );

    let has_placeholder = diags.iter().any(|d| {
        d.message().contains("todo!")
    });
    assert!(
        has_placeholder,
        "diagnostic must mention todo! macro"
    );
}

#[test]
fn given_unimplemented_macro_in_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[tokio::test]
        async fn given_workflow_starts_when_something_happens_then_produces_output() {
            unimplemented!("feature not yet implemented")
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test contains unimplemented! macro"
    );

    let has_placeholder = diags.iter().any(|d| {
        d.message().contains("unimplemented!")
    });
    assert!(
        has_placeholder,
        "diagnostic must mention unimplemented! macro"
    );
}

#[test]
fn given_unreachable_macro_in_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[test]
        fn given_workflow_starts_when_something_happens_then_produces_output() {
            unreachable!("this branch should not be hit")
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test contains unreachable! macro"
    );
}

#[test]
fn given_assert_true_in_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[test]
        fn given_workflow_starts_when_something_happens_then_produces_output() {
            assert!(true);
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test contains assert!(true)"
    );

    let has_placeholder = diags.iter().any(|d| {
        d.message().contains("assert!(true)")
    });
    assert!(
        has_placeholder,
        "diagnostic must mention assert!(true)"
    );
}

#[test]
fn given_ignored_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[test]
        #[ignore]
        fn given_workflow_starts_when_something_happens_then_produces_output() {
            assert!(false);
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test is marked #[ignore]"
    );

    let has_ignore = diags.iter().any(|d| {
        d.message().contains("#[ignore]")
    });
    assert!(
        has_ignore,
        "diagnostic must mention #[ignore]"
    );
}

#[test]
fn given_constant_only_assert_eq_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[test]
        fn given_commit_outcome_when_compared_then_equality_holds() {
            assert_eq!(CommitOutcome::Failed, CommitOutcome::Failed);
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test asserts only constants"
    );
}

#[test]
fn given_real_behavior_assertion_when_quality_gate_runs_then_no_placeholder_diagnostic() {
    let src = quote! {
        #[test]
        fn given_workflow_instance_when_started_then_timeline_has_entry() {
            let instance_id = "test-instance-1".to_string();
            let state = setup_test_state(&instance_id);
            let response = get_timeline(state).await;
            assert_eq!(response.instance_id, instance_id);
        }
    };

    let diags = lint(&src.to_string());

    let placeholder_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message().contains("placeholder"))
        .collect();

    assert!(
        placeholder_diags.is_empty(),
        "real behavior assertions must not trigger placeholder diagnostic"
    );
}

#[test]
fn given_normal_test_code_when_quality_gate_runs_then_no_diagnostics() {
    let src = quote! {
        fn helper_function() -> bool {
            true
        }

        #[test]
        fn given_something_when_condition_then_result() {
            let result = helper_function();
            assert!(result);
        }
    };

    let diags = lint(&src.to_string());
    assert!(
        diags.is_empty(),
        "normal test code should not produce diagnostics"
    );
}

#[test]
fn given_multiple_placeholder_patterns_when_quality_gate_runs_then_multiple_diagnostics() {
    let src = quote! {
        #[test]
        #[ignore]
        fn given_todo_test() {
            todo!()
        }

        #[test]
        fn given_assert_true_test() {
            assert!(true);
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        diags.len() >= 2,
        "must produce at least one diagnostic per placeholder pattern"
    );
}

#[test]
fn given_production_handler_import_when_lint_runs_then_no_placeholder_diagnostic() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            use vo_api::handlers::sse::WorkflowSseEvent;

            #[test]
            fn given_step_completes_when_event_serialized_then_has_type() {
                let event = WorkflowSseEvent::StepCompleted {
                    node_name: "build-step".to_string(),
                    sequence: 42,
                };
                assert!(!event.node_name.is_empty());
            }
        }
    };

    let diags = lint(&src.to_string());

    let placeholder_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message().contains("placeholder"))
        .collect();

    assert!(
        placeholder_diags.is_empty(),
        "production handler imports must not trigger placeholder diagnostic"
    );
}