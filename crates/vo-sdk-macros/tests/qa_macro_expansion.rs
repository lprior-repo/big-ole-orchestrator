//! QA macro expansion tests — consumer-perspective correctness via trybuild.
//!
//! Tests the public `#[task]` proc-macro at compile time, verifying:
//! - Attribute parsing rejects unknown keys
//! - Code generation produces valid executables for sync/async functions
//! - Error messages are precise for wrong item types and signatures

use trybuild::TestCases;

#[test]
fn qa_pass_sync_task_compiles() {
    let t = TestCases::new();
    t.pass("tests/qa_ui/pass_sync_no_return.rs");
}

#[test]
fn qa_pass_async_task_compiles() {
    let t = TestCases::new();
    t.pass("tests/qa_ui/pass_async_with_return.rs");
}

#[test]
fn qa_pass_sync_return_propagates() {
    let t = TestCases::new();
    t.pass("tests/qa_ui/pass_sync_return_type.rs");
}

#[test]
fn qa_fail_struct_rejected() {
    let t = TestCases::new();
    t.compile_fail("tests/qa_ui/fail_struct.rs");
}

#[test]
fn qa_fail_arguments_rejected() {
    let t = TestCases::new();
    t.compile_fail("tests/qa_ui/fail_arguments.rs");
}

#[test]
fn qa_fail_unsupported_attr_rejected() {
    let t = TestCases::new();
    t.compile_fail("tests/qa_ui/fail_bad_attr.rs");
}
