//! RED-QUEEN coevolutionary linter tests.
//!
//! Adversarial patterns that evolve alongside linter rules.
//! If a rule changes, these tests must co-evolve or break —
//! preventing silent regressions where the linter weakens.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use quote::quote;
use syn::File;
use vo_linter::{rules::check_random_in_workflow, Diagnostic};

fn parse(src: &str) -> File {
    syn::parse_str(src).expect("failed to parse Rust source")
}

fn lint(src: &str) -> Vec<Diagnostic> {
    check_random_in_workflow(&parse(src))
}

// ── Generation 0: Core detection invariants ──

#[test]
fn coevo_gen0_uuid_v4_always_detected() {
    let diags = lint(&quote! { fn f() { Uuid::new_v4(); } }.to_string());
    assert_eq!(diags.len(), 1, "Uuid::new_v4 must always be flagged");
}

#[test]
fn coevo_gen0_rand_random_always_detected() {
    let diags = lint(&quote! { fn f() { rand::random::<u32>(); } }.to_string());
    assert_eq!(diags.len(), 1, "rand::random must always be flagged");
}

#[test]
fn coevo_gen0_safe_alternatives_never_flagged() {
    let diags = lint(&quote! { fn f() { ctx.random_u64(); ctx.random_u128(); } }.to_string());
    assert!(diags.is_empty(), "ctx.random_* must never be flagged");
}

// ── Generation 1: Evasion attempts ──

#[test]
fn coevo_gen1_uuid_v4_chained_method_not_silenced() {
    let diags =
        lint(&quote! { fn f() { let s = Uuid::new_v4().hyphenated().to_string(); } }.to_string());
    assert_eq!(diags.len(), 1, "chaining on Uuid::new_v4() must be caught");
}

#[test]
fn coevo_gen1_uuid_v4_in_tuple_not_silenced() {
    let diags = lint(&quote! { fn f() { let pair = (Uuid::new_v4(), "name"); } }.to_string());
    assert_eq!(
        diags.len(),
        1,
        "Uuid::new_v4() inside tuples must be caught"
    );
}

#[test]
fn coevo_gen1_rand_random_in_vec_macro_not_expanded() {
    // Known limitation: macro bodies aren't expanded by syn visitor.
    let diags = lint(&quote! { fn f() { let v = vec![rand::random::<u8>(); 256]; } }.to_string());
    assert_eq!(
        diags.len(),
        0,
        "macro bodies are not expanded — known blind spot"
    );
}

#[test]
fn coevo_gen1_rand_random_near_default_still_caught() {
    let diags = lint(
        &quote! { fn f() { let x: u64 = Default::default(); let r = rand::random::<u64>(); } }
            .to_string(),
    );
    assert_eq!(
        diags.len(),
        1,
        "rand::random near defaults must still be caught"
    );
}

// ── Generation 2: Structural camouflage ──

#[test]
fn coevo_gen2_random_inside_impl_block() {
    let diags = lint(
        &quote! {
            impl Foo {
                fn generate(&self) -> u64 { rand::random::<u64>() }
            }
        }
        .to_string(),
    );
    assert_eq!(diags.len(), 1, "random inside impl methods must be caught");
}

#[test]
fn coevo_gen2_random_inside_trait_impl() {
    let diags = lint(
        &quote! {
            trait Generator { fn make(&self) -> u32; }
            impl Generator for Foo {
                fn make(&self) -> u32 { rand::random() }
            }
        }
        .to_string(),
    );
    assert_eq!(diags.len(), 1, "random inside trait impls must be caught");
}

#[test]
fn coevo_gen2_random_in_let_else() {
    let diags = lint(
        &quote! {
            fn f() {
                let id = Some(Uuid::new_v4()).unwrap_or_else(|| ctx.random_u64());
            }
        }
        .to_string(),
    );
    assert_eq!(
        diags.len(),
        1,
        "Uuid::new_v4() inside let-else must be caught"
    );
}

#[test]
fn coevo_gen2_random_in_unsafe_block() {
    let diags = lint(&quote! { fn f() { unsafe { let id = Uuid::new_v4(); } } }.to_string());
    assert_eq!(diags.len(), 1, "random inside unsafe blocks must be caught");
}

// ── Generation 3: Mutation robustness ──

#[test]
fn coevo_gen3_diagnostic_message_contains_guidance() {
    let diags = lint(&quote! { fn f() { Uuid::new_v4(); } }.to_string());
    assert_eq!(diags.len(), 1);
    let msg = diags[0].message();
    assert!(
        msg.contains("non-deterministic") || msg.contains("random"),
        "diagnostic must explain WHY: got '{msg}'"
    );
}

#[test]
fn coevo_gen3_false_positive_pressure_empty_structs() {
    let diags = lint(
        &quote! {
            struct Config { name: String, port: u16 }
            struct App { config: Config }
        }
        .to_string(),
    );
    assert!(
        diags.is_empty(),
        "pure data structs must never trigger diagnostics"
    );
}

#[test]
fn coevo_gen3_false_positive_pressure_traits_only() {
    let diags = lint(
        &quote! {
            trait RandomSource { fn next(&self) -> u64; }
            trait Deterministic { fn compute(&self) -> u64; }
        }
        .to_string(),
    );
    assert!(
        diags.is_empty(),
        "trait declarations alone must not trigger diagnostics"
    );
}

#[test]
fn coevo_gen4_blast_radius_single_vs_many_calls() {
    let one = lint(&quote! { fn f() { Uuid::new_v4(); } }.to_string());
    let ten = lint(
        &quote! {
            fn f() {
                Uuid::new_v4(); Uuid::new_v4(); Uuid::new_v4();
                rand::random::<u8>(); rand::random::<u16>();
                Uuid::new_v4(); Uuid::new_v4(); Uuid::new_v4();
                rand::random::<u32>(); rand::random::<u64>();
                Uuid::new_v4();
            }
        }
        .to_string(),
    );
    assert_eq!(one.len(), 1);
    assert_eq!(ten.len(), 11, "must detect every single random call");
}

#[test]
fn coevo_gen4_case_sensitivity_boundary() {
    let lower = lint(&quote! { fn f() { rand::random::<u8>(); } }.to_string());
    let upper = lint(&quote! { fn f() { RAND::random::<u8>(); } }.to_string());
    assert_eq!(lower.len(), 1, "rand::random must be flagged");
    assert_eq!(upper.len(), 0, "RAND::random must not be flagged");
}
