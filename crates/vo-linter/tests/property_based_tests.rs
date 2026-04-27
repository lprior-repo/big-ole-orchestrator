#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

//! Property-Based Tests for vo-linter using proptest.
//!
//! Tests that verify linter behavior across randomly generated valid and invalid
//! Rust source code ASTs.
//!
//! # Coverage Areas
//!
//! - Random expression generation
//! - Random statement sequences
//! - Random function definitions
//! - Random type annotations
//! - Edge case fuzzing
//! - Invariant preservation

use proptest::prelude::*;
use quote::quote;
use syn::parse_str;
use vo_linter::rules::check_random_in_workflow;

// ─────────────────────────────────────────────────────────────────────────────
// Invariant: Empty and whitespace inputs produce no diagnostics
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn empty_source_produces_no_diagnostics(src in " *") {
        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            0,
            "Empty/whitespace source should produce no diagnostics, got {} for: {:?}",
            diags.len(),
            src
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant: Deterministic code is never flagged
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn ctx_random_never_flagged(count in 0u32..10) {
        let mut src = String::from("fn workflow() { ");
        for i in 0..count {
            src.push_str(&format!("let x{} = ctx.random_u64(); ", i));
        }
        src.push_str(" }");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            0,
            "ctx.random_u64() should never be flagged, got {} diagnostics",
            diags.len()
        );
    }
}

proptest! {
    #[test]
    fn deterministic_fn_never_flagged(name in "[a-z][a-z0-9_]*", call_count in 0u32..5) {
        let mut src = format!("fn workflow() {{ ");
        for i in 0..call_count {
            src.push_str(&format!("{}({}); ", name, i));
        }
        src.push_str(" }");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            0,
            "Deterministic function {} should not be flagged",
            name
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Uuid::new_v4() is always detected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn uuid_new_v4_always_detected() {
    let srcs = vec![
        "fn workflow() { let id = uuid::Uuid::new_v4(); }",
        "fn workflow() { let id = Uuid::new_v4(); }",
    ];

    for src in srcs {
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Uuid::new_v4() should always be detected"
        );
    }
}

#[test]
fn fully_qualified_uuid_detected() {
    let src = "fn workflow() { let id = uuid::Uuid::new_v4(); }";
    let file: syn::File = parse_str(src).expect("parse failed");
    let diags = check_random_in_workflow(&file);
    assert_eq!(diags.len(), 1, "Fully qualified Uuid::new_v4() should be detected");
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: rand::random() is always detected
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn rand_random_always_detected(ty in "u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|isize") {
        let src = format!(
            "fn workflow() {{ let x: {} = rand::random::<{}>(); }}",
            ty, ty
        );

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "rand::random::<{}>() should always be detected",
            ty
        );
    }
}

#[test]
fn rand_random_without_type_annotation_detected() {
    let src = "fn workflow() { let x = rand::random::<u32>(); }";
    let file: syn::File = parse_str(src).expect("parse failed");
    let diags = check_random_in_workflow(&file);
    assert_eq!(diags.len(), 1, "rand::random without type annotation should be detected");
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Count correctness - each random call produces one diagnostic
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn count_matches_random_call_count(count in 1u32..20) {
        let mut src = String::from("fn workflow() { ");
        for _ in 0..count {
            src.push_str("Uuid::new_v4(); ");
        }
        src.push_str(" }");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len() as u32,
            count,
            "Each Uuid::new_v4() should produce one diagnostic"
        );
    }
}

proptest! {
    #[test]
    fn mixed_random_calls_count_correctly(uuid_count in 0u32..5, rand_count in 0u32..5) {
        let mut src = String::from("fn workflow() { ");
        for _ in 0..uuid_count {
            src.push_str("Uuid::new_v4(); ");
        }
        for _ in 0..rand_count {
            src.push_str("rand::random::<u32>(); ");
        }
        src.push_str(" }");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let expected = uuid_count + rand_count;
        prop_assert_eq!(
            diags.len() as u32,
            expected,
            "Mixed {} Uuid and {} rand::random should produce {} diagnostics",
            uuid_count,
            rand_count,
            expected
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Diagnostic always has non-empty message
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn diagnostic_message_never_empty() {
    let srcs = vec![
        "fn workflow() { Uuid::new_v4(); }",
        "fn workflow() { rand::random::<u32>(); }",
    ];

    for src in srcs {
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(!diags.is_empty(), "Should have at least one diagnostic");

        for (i, diag) in diags.iter().enumerate() {
            assert!(
                !diag.message().is_empty(),
                "Diagnostic {} should have non-empty message",
                i
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Diagnostic always has suggestion
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn diagnostic_always_has_suggestion() {
    let srcs = vec![
        "fn workflow() { Uuid::new_v4(); }",
        "fn workflow() { rand::random::<u32>(); }",
    ];

    for src in srcs {
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(!diags.is_empty(), "Should have at least one diagnostic for: {}", src);

        for (i, diag) in diags.iter().enumerate() {
            assert!(
                diag.suggestion().is_some(),
                "Diagnostic {} should have a suggestion",
                i
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Deeply nested code is handled correctly
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn deeply_nested_random_detected(depth in 1u32..10) {
        let mut src = String::from("fn workflow() { ");
        for _ in 0..depth {
            src.push_str("if true { ");
        }
        src.push_str("Uuid::new_v4(); ");
        for _ in 0..depth {
            src.push_str("} ");
        }
        src.push_str("}");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "Random at depth {} should be detected",
            depth
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Different line positions are handled
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn random_on_various_lines(line_count in 1u32..10, random_line in 0u32..10) {
        let mut src = String::from("fn workflow() {\n");
        for i in 0..line_count {
            if i == random_line % line_count {
                src.push_str("    Uuid::new_v4();\n");
            } else {
                src.push_str("    let x = 1;\n");
            }
        }
        src.push_str("}");

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "Random on line {} should be detected",
            random_line % line_count
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Module nesting doesn't affect detection
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn nested_module_random_detected(depth in 1u32..5) {
        let mut src = String::new();
        for i in 0..depth {
            src.push_str(&format!("mod level{} {{ ", i));
        }
        src.push_str("fn workflow() { Uuid::new_v4(); } ");
        for _ in 0..depth {
            src.push_str("} ");
        }

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "Random in {} levels of nesting should be detected",
            depth
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Closure contexts are handled
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn random_in_closure_context(context in "iter|option|result|vec") {
        let src: &str = match context.as_str() {
            "iter" => "fn workflow() { [1,2,3].iter().map(|x| { Uuid::new_v4(); *x }); }",
            "option" => "fn workflow() { Some(1).map(|x| { Uuid::new_v4(); x }); }",
            "result" => "fn workflow() { Ok::<_, ()>(1).map(|x| { Uuid::new_v4(); x }); }",
            "vec" => "fn workflow() { vec![1,2,3].into_iter().map(|x| { Uuid::new_v4(); x }); }",
            _ => unreachable!(),
        };

        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "Random in {} closure context should be detected",
            context
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property: Loop contexts are handled
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn random_in_loop_context(context in "for|while|loop") {
        let src: &str = match context.as_str() {
            "for" => "fn workflow() { for i in 0..10 { let _ = Uuid::new_v4(); } }",
            "while" => "fn workflow() { while true { let _ = Uuid::new_v4(); break; } }",
            "loop" => "fn workflow() { loop { let _ = Uuid::new_v4(); break; } }",
            _ => unreachable!(),
        };

        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            1,
            "Random in {} loop context should be detected",
            context
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invariant: Random never appears in valid deterministic patterns
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn deterministic_patterns_never_flagged(
        fn_name in "[a-z][a-z0-9_]{2,10}",
        var_name in "[a-z][a-z0-9_]{2,10}",
        type_name in "[A-Z][a-zA-Z0-9]{2,10}"
    ) {
        let src = format!(
            "fn w{}() -> T{} {{ let v{}: T{} = ctx.random_u64(); v{} }}",
            fn_name, type_name, var_name, type_name, var_name
        );

        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        prop_assert_eq!(
            diags.len(),
            0,
            "Deterministic pattern should never be flagged"
        );
    }
}
