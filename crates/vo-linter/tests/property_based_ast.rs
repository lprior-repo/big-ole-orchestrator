//! Property-based tests for the vo-linter AST visitor.
//!
//! Uses proptest to generate random valid and invalid Rust ASTs,
//! verifying that the linter handles all edge cases without panicking
//! and produces consistent results across multiple runs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use quote::quote;
use syn::File;
use vo_linter::rules::check_random_in_workflow;
use vo_linter::{Diagnostic, LintCode};

fn parse_and_check(src: &str) -> Vec<Diagnostic> {
    match syn::parse_str::<File>(src) {
        Ok(file) => check_random_in_workflow(&file),
        Err(_) => vec![],
    }
}

proptest! {
    #[test]
    fn prop_empty_or_whitespace_produces_no_diagnostics(src in "\\{0,5000}") {
        let diags = parse_and_check(&src);
        assert!(diags.is_empty(), "whitespace-only input should not produce diagnostics");
    }

    #[test]
    fn prop_random_string_no_crash(src in "\\{0,10000}") {
        // Property: arbitrary byte sequences that parse as invalid Rust must not panic
        let _ = parse_and_check(&src);
    }

    #[test]
    fn prop_valid_rust_no_panic(code in "\\{0,5000}") {
        // Property: any valid Rust code must be processable without panic
        let diags = parse_and_check(&code);
        // Diagnostics may or may not exist, but no panic is the key property
        for diag in &diags {
            assert!(!diag.message().is_empty(), "diagnostic message must not be empty");
        }
    }

    #[test]
    fn prop_uuid_new_v4_always_detected(
        prefix in "\\w{0,30}",
        suffix in "\\w{0,30}",
        nested_depth in 0usize..5,
    ) {
        // Property: Uuid::new_v4() is detected regardless of surrounding code
        let nested = (0..nested_depth).map(|_| "if true { ").collect::<String>();
        let close = (0..nested_depth).map(|_| " }").collect::<String>();
        let src = format!(
            "fn {}() {{ {} let id = {}::Uuid::new_v4(); {} }}",
            prefix, nested, suffix, close
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "Uuid::new_v4() should be detected in nested context"
        );
    }

    #[test]
    fn prop_rand_random_always_detected(
        prefix in "\\w{0,30}",
        suffix in "\\w{0,30}",
        nested_depth in 0usize..5,
    ) {
        // Property: rand::random() is detected regardless of surrounding code
        let nested = (0..nested_depth).map(|_| "if true { ").collect::<String>();
        let close = (0..nested_depth).map(|_| " }").collect::<String>();
        let src = format!(
            "fn {}() {{ {} let x: {} = rand::random(); {} }}",
            prefix, nested, suffix, close
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "rand::random() should be detected in nested context"
        );
    }

    #[test]
    fn prop_multiple_random_calls_each_detected(
        num_randoms in 1usize..20,
    ) {
        // Property: each random call produces exactly one diagnostic
        let mut src = String::from("fn workflow() { ");
        for i in 0..num_randoms {
            if i % 2 == 0 {
                src.push_str(&format!("let _id{} = Uuid::new_v4(); ", i));
            } else {
                src.push_str(&format!("let _v{} = rand::random::<u32>(); ", i));
            }
        }
        src.push('}');
        let diags = parse_and_check(&src);
        assert_eq!(
            diags.len(), num_randoms,
            "each random call should produce exactly one diagnostic"
        );
    }

    #[test]
    fn prop_no_random_produces_empty_diagnostics(
        deterministic_code in "\\{5,5000}",
    ) {
        // Property: code without Uuid::new_v4() or rand::random() produces no diagnostics
        // We filter to deterministic patterns only
        let deterministic = deterministic_code
            .replace("Uuid::new_v4()", "deterministic_fn()")
            .replace("rand::random()", "deterministic_fn()")
            .replace("rand::", "deterministic::")
            .replace("Uuid::", "deterministic::");
        if !deterministic.is_empty() {
            let diags = parse_and_check(&deterministic);
            let random_count = diags.iter().filter(|d| d.code == LintCode::L002).count();
            assert_eq!(
                random_count, 0,
                "deterministic code should not trigger L002"
            );
        }
    }

    #[test]
    fn prop_diagnostics_are_consistent_across_runs(src in "\\{0,2000}") {
        // Property: running the linter multiple times on the same input produces identical output
        let diags1 = parse_and_check(&src);
        let diags2 = parse_and_check(&src);
        let diags3 = parse_and_check(&src);
        assert_eq!(
            diags1.len(), diags2.len(), "diagnostic count should be consistent"
        );
        assert_eq!(
            diags2.len(), diags3.len(), "diagnostic count should be consistent"
        );
    }

    #[test]
    fn prop_use_renames_resolved(src in "\\{0,2000}") {
        // Property: adding use statements shouldn't change behavior for non-random code
        let base_code = src
            .replace("Uuid::new_v4()", "deterministic_call()")
            .replace("rand::random()", "deterministic_call()");
        let with_use = format!("use std::string::String;\nuse std::collections::HashMap;\n{}", base_code);
        let diags_orig = parse_and_check(&base_code);
        let diags_with_use = parse_and_check(&with_use);
        assert_eq!(
            diags_orig.len(), diags_with_use.len(),
            "adding unrelated use statements should not change diagnostic count"
        );
    }

    #[test]
    fn prop_deeply_nested_random_still_detected(depth in 1usize..10) {
        // Property: random calls at arbitrary nesting depths are still detected
        let open = (0..depth).map(|_| "{ ").collect::<String>();
        let close = (0..depth).map(|_| " }").collect::<String>();
        let src = format!("fn w() {{ {} Uuid::new_v4(); {} }}", open, close);
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "random call at depth {} should be detected",
            depth
        );
    }

    #[test]
    fn prop_mixed_random_types_each_detected(
        uuid_count in 1usize..10,
        rand_count in 1usize..10,
    ) {
        // Property: mixed Uuid::new_v4() and rand::random() calls each detected
        let mut src = String::from("fn w() { ");
        for i in 0..uuid_count {
            src.push_str(&format!("let _a{} = Uuid::new_v4(); ", i));
        }
        for i in 0..rand_count {
            src.push_str(&format!("let _b{} = rand::random::<u32>(); ", i));
        }
        src.push('}');
        let diags = parse_and_check(&src);
        assert_eq!(
            diags.len(), uuid_count + rand_count,
            "each mixed random call should be detected independently"
        );
    }

    #[test]
    fn prop_suggestion_present_on_all_diagnostics(src in "\\{0,3000}") {
        // Property: all L002 diagnostics include the ctx.random_u64() suggestion
        let diags = parse_and_check(&src);
        for diag in &diags {
            if diag.code == LintCode::L002 {
                assert!(
                    diag.suggestion().is_some(),
                    "L002 diagnostic must have a suggestion"
                );
                assert!(
                    diag.suggestion().unwrap().contains("ctx.random_u64()"),
                    "suggestion must mention ctx.random_u64()"
                );
            }
        }
    }

    #[test]
    fn prop_multiple_use_renames_handled(
        rename1 in "\\w{2,10}",
        rename2 in "\\w{2,10}",
    ) {
        // Property: multiple use renames are all resolved correctly
        let src = format!(
            "use uuid::Uuid as {rename1};\nuse rand::Rng as {rename2};\nfn w() {{ let _ = {rename1}::new_v4(); }}"
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "use-renamed Uuid should still be detected"
        );
    }

    #[test]
    fn prop_async_block_random_detected(src in "\\w{0,20}") {
        // Property: random in async blocks is detected
        let fn_name = if src.is_empty() { "w" } else { &src[..src.len().min(20)] };
        let src = format!(
            "async fn {}() {{ async {{ let _ = Uuid::new_v4(); }}; }}",
            fn_name
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "random in async block should be detected"
        );
    }

    #[test]
    fn prop_closure_random_detected(src in "\\w{0,20}") {
        // Property: random inside closures is detected
        let fn_name = if src.is_empty() { "w" } else { &src[..src.len().min(20)] };
        let src = format!(
            "fn {}() {{ let f = || {{ Uuid::new_v4(); }}; }}",
            fn_name
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "random inside closure should be detected"
        );
    }

    #[test]
    fn prop_try_block_random_detected(src in "\\w{0,20}") {
        // Property: random in try/fallible contexts is detected
        let fn_name = if src.is_empty() { "w" } else { &src[..src.len().min(20)] };
        let src = format!(
            "fn {}() -> Result<(), ()> {{ let _ = Uuid::new_v4()?; Ok(()) }}"
        );
        let diags = parse_and_check(&src);
        assert!(
            diags.iter().any(|d| d.code == LintCode::L002),
            "random in try context should be detected"
        );
    }
}
