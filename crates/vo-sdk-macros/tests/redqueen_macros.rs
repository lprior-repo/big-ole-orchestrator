//! RED-QUEEN coevolutionary adversarial macro expansion tests.
//!
//! Adversarial probes that coevolve with macro changes: edge-case token
//! streams, boundary conditions, and mutation-resistant invariants.

use trybuild::TestCases;

#[test]
fn rq_fail_impl_block() {
    let t = TestCases::new();
    t.compile_fail("tests/rq_ui/fail_impl_block.rs");
}

#[test]
fn rq_fail_enum_rejected() {
    let t = TestCases::new();
    t.compile_fail("tests/rq_ui/fail_trait_fn.rs");
}

#[test]
fn rq_fail_multi_attr() {
    let t = TestCases::new();
    t.compile_fail("tests/rq_ui/fail_multi_attr.rs");
}

#[test]
fn rq_fail_equals_attr() {
    let t = TestCases::new();
    t.compile_fail("tests/rq_ui/fail_equals_attr.rs");
}

#[test]
fn rq_pass_pub_crate() {
    let t = TestCases::new();
    t.pass("tests/rq_ui/pass_pub_crate.rs");
}

#[test]
fn rq_pass_async() {
    let t = TestCases::new();
    t.pass("tests/rq_ui/pass_async.rs");
}

#[test]
fn rq_fail_async_generic() {
    let t = TestCases::new();
    t.compile_fail("tests/rq_ui/fail_async_generic.rs");
}

#[test]
fn rq_pass_unsafe() {
    let t = TestCases::new();
    t.pass("tests/rq_ui/pass_unsafe.rs");
}
