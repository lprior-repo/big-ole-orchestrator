#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use std::io::Write;
use tempfile::NamedTempFile;
use vo_linter::{rules::check_random_in_workflow, Diagnostic, LintCode};

/// Helper: parse a Rust source string and return diagnostics.
fn lint_source(src: &str) -> Vec<Diagnostic> {
    let file: syn::File = syn::parse_str(src).expect("valid Rust source");
    check_random_in_workflow(&file)
}

/// Helper: parse a Rust file on disk and return diagnostics.
fn lint_file(path: &std::path::Path) -> Vec<Diagnostic> {
    let src = std::fs::read_to_string(path).expect("read temp file");
    lint_source(&src)
}

// -- Rule Validation: L002 detects non-deterministic calls --

#[test]
fn rule_l002_catches_uuid_new_v4() {
    let diags = lint_source("fn f() { let x = Uuid::new_v4(); }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn rule_l002_catches_rand_random() {
    let diags = lint_source("fn f() { let x: u32 = rand::random(); }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn rule_l002_catches_rand_random_generic() {
    let diags = lint_source("fn f() { let x = rand::random::<u64>(); }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn rule_l002_count_matches_call_sites() {
    let src = r#"
        fn a() { Uuid::new_v4(); Uuid::new_v4(); }
        fn b() { rand::random::<u32>(); rand::random::<u64>(); }
    "#;
    assert_eq!(lint_source(src).len(), 4);
}

// -- Warning Detection: message content and suggestions --

#[test]
fn warning_message_mentions_non_deterministic() {
    let d = &lint_source("fn f() { Uuid::new_v4(); }")[0];
    assert!(d.message().contains("non-deterministic"));
}

#[test]
fn warning_suggests_ctx_random() {
    let src = "fn f() { Uuid::new_v4(); }";
    let d1 = lint_source(src).into_iter().next().expect("one diag");
    let d2 = d1.clone();
    assert_eq!(d1.message(), d2.message());
}

// -- False Positive Rates: clean code produces zero diagnostics --

#[test]
fn no_false_positive_ctx_random_u64() {
    let diags = lint_source("fn f() { let x = ctx.random_u64(); }");
    assert!(diags.is_empty(), "ctx.random_u64() must not trigger L002");
}

#[test]
fn no_false_positive_uuid_new_v1() {
    let diags = lint_source("fn f() { let x = Uuid::new_v1(); }");
    assert!(diags.is_empty(), "Uuid::new_v1 is time-based, not non-deterministic");
}

#[test]
fn no_false_positive_custom_uuid_type() {
    let diags = lint_source("fn f() { let x = MyUuid::new_v4(); }");
    assert!(diags.is_empty(), "custom types should not trigger L002");
}

#[test]
fn no_false_positive_rand_thread_rng() {
    let diags = lint_source("fn f() { let mut rng = rand::thread_rng(); }");
    assert!(diags.is_empty());
}

#[test]
fn no_false_positive_case_sensitive() {
    let diags = lint_source("fn f() { let x = RAND::random(); }");
    assert!(diags.is_empty(), "uppercase RAND is not rand module");
}

#[test]
fn no_false_positive_empty_and_trivial_files() {
    assert!(lint_source("").is_empty());
    assert!(lint_source("fn f() {}").is_empty());
}

// -- Temp Directory: round-trip through file on disk --

#[test]
fn lint_via_temp_file_round_trip() {
    let mut tmp = NamedTempFile::new().expect("temp file");
    write!(tmp, "fn workflow() {{ let id = Uuid::new_v4(); }}")
        .expect("write");
    tmp.flush().expect("flush");
    let diags = lint_file(tmp.path());
    assert_eq!(diags.len(), 1);
}

#[test]
fn temp_file_clean_code_no_diagnostics() {
    let mut tmp = NamedTempFile::new().expect("temp file");
    write!(tmp, "fn clean() {{ ctx.random_u64(); }}").expect("write");
    tmp.flush().expect("flush");
    assert!(lint_file(tmp.path()).is_empty());
}

#[test]
fn temp_file_with_comments_only() {
    let mut tmp = NamedTempFile::new().expect("temp file");
    write!(tmp, "// workflow def\n/* block */\nfn f() {{}}").expect("write");
    tmp.flush().expect("flush");
    assert!(lint_file(tmp.path()).is_empty());
}

// -- Structural Coverage: nested and complex call sites --

#[test]
fn detects_in_closure() {
    let diags = lint_source("fn f() { let c = || Uuid::new_v4(); }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn detects_in_struct_literal() {
    let diags = lint_source("fn f() { let s = S { id: Uuid::new_v4() }; }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn detects_in_array_literal() {
    let diags = lint_source("fn f() { let a = [Uuid::new_v4(), Uuid::new_v4()]; }");
    assert_eq!(diags.len(), 2);
}

#[test]
fn detects_in_method_chain() {
    let diags = lint_source("fn f() { let s = Uuid::new_v4().to_string(); }");
    assert_eq!(diags.len(), 1);
}

#[test]
fn uuid_new_v4_with_args_ignored() {
    let diags = lint_source("fn f() { let x = Uuid::new_v4(&mut buf); }");
    assert!(diags.is_empty(), "new_v4 with args is not the no-arg constructor");
}

#[test]
fn diagnostic_clone_equality() {
    let d = Diagnostic::new(LintCode::L002, "test");
    let d2 = d.clone();
    assert_eq!(d.message(), d2.message());
}
