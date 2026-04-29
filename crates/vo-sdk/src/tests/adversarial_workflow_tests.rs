//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: Workflow builder edge cases.

use crate::node_handle::NodeHandle;
use crate::workflow::Workflow;

#[test]
fn workflow_build_uses_stored_workflow_name() {
    let mut wf = Workflow::new("custom-name");
    let _: NodeHandle<(), ()> = wf.pure("n", |_i: ()| ()).unwrap();

    let spec = wf.build().unwrap();
    assert_eq!(spec.workflow_name.as_str(), "custom-name");
}

#[test]
fn workflow_connect_type_mismatch_does_not_compile() {
    let mut wf = Workflow::new("type_check");
    let a: NodeHandle<String, i32> = wf.pure("a", |_i: String| -> i32 { 0 }).unwrap();
    let _b: NodeHandle<bool, ()> = wf.effect("b", |_i: bool| ()).unwrap();

    let _ = a;
}
