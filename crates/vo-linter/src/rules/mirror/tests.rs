//! Tests for the mirror rule (L003).

use super::check_mirror_types_in_api_test;
use quote::quote;
use syn::File;

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
fn lint(src: &str) -> usize {
    let file: File = syn::parse_str(src).unwrap();
    check_mirror_types_in_api_test(&file).len()
}

#[test]
fn given_mirror_sse_event_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        //! Test module
        #[derive(Debug, Clone)]
        pub enum MirrorSseEvent {
            StepCompleted { node_name: String, sequence: u64 },
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "mirror SseEvent type must trigger L003 diagnostic"
    );
}

#[test]
fn given_fake_handler_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        pub struct FakeHandler {
            pub name: String,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "fake Handler type must trigger L003 diagnostic"
    );
}

#[test]
fn given_mock_broadcaster_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        pub struct MockBroadcaster {
            pub events: Vec<String>,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "mock Broadcaster type must trigger L003 diagnostic"
    );
}

#[test]
fn given_production_event_type_when_quality_gate_runs_then_gate_passes() {
    let src = quote! {
        pub struct ProductionEvent {
            pub payload: Vec<u8>,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        0,
        "production Event type must NOT trigger L003 diagnostic"
    );
}

#[test]
fn given_regular_struct_when_quality_gate_runs_then_gate_passes() {
    let src = quote! {
        pub struct Config {
            pub port: u16,
            pub host: String,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        0,
        "regular struct must NOT trigger L003 diagnostic"
    );
}

#[test]
fn given_mirror_event_in_impl_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        pub enum MirrorEvent {
            StepCompleted,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "Mirror Event type in impl must trigger L003 diagnostic"
    );
}

#[test]
fn given_mirror_attribute_on_struct_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        #[mirror_of = "handlers/sse.rs"]
        pub struct SomeEvent {
            pub data: String,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "struct with mirror_of attribute must trigger L003 diagnostic"
    );
}

#[test]
fn given_api_mirror_test_when_quality_gate_runs_then_gate_fails() {
    let src = quote! {
        pub struct MirrorApiHandler {
            pub events: Vec<String>,
        }
    };
    assert_eq!(
        lint(&src.to_string()),
        1,
        "API test defining local mirror handler must trigger L003 diagnostic"
    );
}