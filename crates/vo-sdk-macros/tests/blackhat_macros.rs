//! BLACK-HAT adversarial macro expansion tests.
//!
//! Attack surfaces: malicious token streams, panic recovery under pathological
//! inputs, unicode identifier injection, raw-string smuggling, and internal API
//! abuse. Every test must either produce a clean `compile_error!` or succeed
//! without panicking — a proc macro panic poisons the compiler.

use trybuild::TestCases;

#[test]
fn bh_rejects_raw_string_smuggling() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_raw_string_smuggling.rs");
}

#[test]
fn bh_rejects_impl_block() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_doc_comment_injection.rs");
}

#[test]
fn bh_rejects_trait_impl_method() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_trait_impl_method.rs");
}

#[test]
fn bh_rejects_nested_macro_invocation() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_nested_macro_invocation.rs");
}

#[test]
fn bh_rejects_const_fn() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_const_fn.rs");
}

#[test]
fn bh_pass_unicode_ident() {
    let t = TestCases::new();
    t.pass("tests/bh_ui/pass_unicode_ident.rs");
}

// ---------------------------------------------------------------------------
// Panic recovery — adversarial inputs that must not crash the compiler
// ---------------------------------------------------------------------------

#[test]
fn bh_rejects_retries_overflow() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/fail_retries_overflow.rs");
}

#[test]
fn bh_pass_unsafe_fn() {
    let t = TestCases::new();
    t.pass("tests/bh_ui/pass_unsafe_fn.rs");
}

#[test]
fn bh_pass_extern_fn() {
    let t = TestCases::new();
    t.pass("tests/bh_ui/pass_extern_fn.rs");
}

#[test]
fn bh_pass_generic_fn() {
    let t = TestCases::new();
    t.compile_fail("tests/bh_ui/pass_generic_fn.rs");
}
