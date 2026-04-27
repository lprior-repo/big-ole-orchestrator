//! BLACK-HAT quality gate tests for mirror-only test detection.
//!
//! These tests verify that the linter correctly identifies API tests that define
//! local mirror types instead of using production handlers.
//!
//! BDD Scenario:
//! Given an API test defines local mirror broadcaster/handler instead of production handler
//! When quality gate scans tests
//! Then gate fails or marks test as non-production coverage

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use quote::quote;
use syn::File;
use vo_linter::{rules::check_mirror_types_in_tests, Diagnostic};

fn parse(src: &str) -> File {
    syn::parse_str(src).expect("failed to parse Rust source")
}

fn lint(src: &str) -> Vec<Diagnostic> {
    check_mirror_types_in_tests(&parse(src))
}

#[test]
fn given_api_mirror_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Debug, Clone)]
            pub enum WorkflowSseEvent {
                StepCompleted {
                    node_name: String,
                    sequence: u64,
                },
                StepFailed {
                    node_name: String,
                    sequence: u64,
                    error: String,
                },
            }

            #[test]
            fn given_step_completes_when_event_serialized_then_has_type() {
                let event = WorkflowSseEvent::StepCompleted {
                    node_name: "build-step".to_string(),
                    sequence: 42,
                };
                // test serialization
            }
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test defines mirror WorkflowSseEvent type"
    );

    let has_mirror_diagnostic = diags.iter().any(|d| {
        d.message()
            .contains("mirror type")
            && d.message().contains("WorkflowSseEvent")
    });
    assert!(
        has_mirror_diagnostic,
        "diagnostic must mention mirror type WorkflowSseEvent"
    );
}

#[test]
fn given_production_handler_import_when_lint_runs_then_no_mirror_diagnostic() {
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
            }
        }
    };

    let diags = lint(&src.to_string());

    let mirror_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message().contains("mirror type"))
        .collect();

    assert!(
        mirror_diags.is_empty(),
        "using production handler import must not trigger mirror diagnostic"
    );
}

#[test]
fn given_ws_mirror_event_type_when_lint_runs_then_gate_fails() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Debug, Clone)]
            pub enum WorkflowWsEvent {
                StepCompleted {
                    node_name: String,
                    sequence: u64,
                },
            }
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test defines mirror WorkflowWsEvent type"
    );
}

#[test]
fn given_sse_broadcaster_mirror_when_lint_runs_then_gate_fails() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Clone)]
            pub struct SseBroadcaster {
                tx: tokio::sync::broadcast::Sender<WorkflowSseEvent>,
            }
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test defines mirror SseBroadcaster type"
    );
}

#[test]
fn given_ws_broadcaster_mirror_when_lint_runs_then_gate_fails() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Clone)]
            pub struct WsBroadcaster {
                tx: tokio::sync::broadcast::Sender<WorkflowWsEvent>,
            }
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test defines mirror WsBroadcaster type"
    );
}

#[test]
fn given_unknown_type_in_test_when_lint_runs_then_no_mirror_diagnostic() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Debug)]
            pub struct MyCustomEvent {
                name: String,
            }
        }
    };

    let diags = lint(&src.to_string());

    let mirror_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.message().contains("mirror type"))
        .collect();

    assert!(
        mirror_diags.is_empty(),
        "unknown custom types must not trigger mirror diagnostic"
    );
}

#[test]
fn given_ws_connection_count_mirror_when_lint_runs_then_gate_fails() {
    let src = quote! {
        #[derive(Clone)]
        pub struct WsConnectionCount {
            active_connections: std::sync::atomic::AtomicUsize,
        }
    };

    let diags = lint(&src.to_string());

    assert!(
        !diags.is_empty(),
        "quality gate must fail when test defines mirror WsConnectionCount type"
    );
}

#[test]
fn given_multiple_mirror_types_when_lint_runs_then_multiple_diagnostics() {
    let src = quote! {
        #[cfg(test)]
        mod tests {
            #[derive(Debug, Clone)]
            pub enum WorkflowSseEvent {
                StepCompleted { node_name: String, sequence: u64 },
            }

            #[derive(Debug, Clone)]
            pub enum WorkflowWsEvent {
                StepCompleted { node_name: String, sequence: u64 },
            }
        }
    };

    let diags = lint(&src.to_string());

    assert_eq!(
        diags.len(),
        2,
        "must produce one diagnostic per mirror type"
    );
}