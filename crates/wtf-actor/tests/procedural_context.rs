// Integration tests for WorkflowContext (bead wtf-wsg6).
use std::collections::HashMap;
use std::sync::Arc;
use bytes::Bytes;
use tokio::sync::RwLock;
use wtf_actor::procedural::WorkflowContext;
use wtf_common::InstanceId;

#[test]
fn next_op_id_returns_incrementing_ids() {
    let ctx = WorkflowContext::new_test(InstanceId::new("inst-01"), HashMap::new());
    let id0 = ctx.next_op_id();
    let id1 = ctx.next_op_id();
    assert_eq!(id0.as_str(), "inst-01:0");
    assert_eq!(id1.as_str(), "inst-01:1");
}
