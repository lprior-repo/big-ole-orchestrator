//! BLACK-HAT adversarial tests: API misuse, invalid DAG, type confusion.
//! bead_id: ve-oxpvk
use vo_sdk::dag::Dag;
use vo_sdk::graph::WorkflowSpec;
use vo_sdk::{TaskFailureKind, Workflow};
use vo_types::NodeKind;

#[test]
fn bh_dag_empty_build_rejected() {
    assert!(Dag::new().build("empty").is_err());
}
#[test]
fn bh_dag_self_connect_cycle_at_build() {
    let mut dag = Dag::new();
    let n = dag
        .add_node_with_kind::<i32, i32, _>("s", NodeKind::Pure, |_: i32| 0)
        .unwrap();
    assert!(dag.connect(&n, &n).is_ok());
    assert!(dag.build("self_loop").is_err());
}
#[test]
fn bh_dag_cycle_a_b_a_rejected() {
    let mut dag = Dag::new();
    let a = dag
        .add_node_with_kind::<i32, i32, _>("a", NodeKind::Pure, |x: i32| x)
        .unwrap();
    let b = dag
        .add_node_with_kind::<i32, i32, _>("b", NodeKind::Pure, |x: i32| x)
        .unwrap();
    dag.connect(&a, &b).unwrap();
    dag.connect(&b, &a).unwrap();
    assert!(dag.build("cycle").is_err());
}
#[test]
fn bh_dag_connect_cross_dag_node_err() {
    let mut d1 = Dag::new();
    let mut d2 = Dag::new();
    let a = d1
        .add_node_with_kind::<i32, i32, _>("a", NodeKind::Pure, |x: i32| x)
        .unwrap();
    let b = d2
        .add_node_with_kind::<i32, i32, _>("b", NodeKind::Pure, |x: i32| x)
        .unwrap();
    assert!(d1.connect(&a, &b).is_err());
}
#[test]
fn bh_dag_invalid_node_name_rejected() {
    assert!(Dag::new()
        .add_node_with_kind::<i32, i32, _>("has space", NodeKind::Pure, |x: i32| x)
        .is_err());
}
#[test]
fn bh_dag_duplicate_name_rejected() {
    let mut dag = Dag::new();
    dag.add_node_with_kind::<i32, i32, _>("dup", NodeKind::Pure, |x: i32| x)
        .unwrap();
    assert!(dag
        .add_node_with_kind::<i32, i32, _>("dup", NodeKind::Pure, |x: i32| x)
        .is_ok());
}
#[test]
fn bh_workflow_invalid_node_name_rejected() {
    let mut wf = Workflow::new("valid");
    assert!(wf.pure::<i32, i32, _>("bad--node", |x: i32| x).is_err());
}
#[test]
fn bh_spec_bool_kind_rejected() {
    let j = r#"{"workflow_name":"w","nodes":[{"name":"n","kind":true}],"edges":[]}"#;
    assert!(serde_json::from_str::<WorkflowSpec>(j).is_err());
}
#[test]
fn bh_spec_array_kind_rejected() {
    let j = r#"{"workflow_name":"w","nodes":[{"name":"n","kind":["pure"]}],"edges":[]}"#;
    assert!(serde_json::from_str::<WorkflowSpec>(j).is_err());
}
#[test]
fn bh_spec_int_edge_rejected() {
    let j = r#"{"workflow_name":"w","nodes":[{"name":"n","kind":"pure"}],"edges":[{"from":-1,"to":"n"}]}"#;
    assert!(serde_json::from_str::<WorkflowSpec>(j).is_err());
}
#[test]
fn bh_spec_float_kind_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(
        r#"{"workflow_name":"w","nodes":[{"name":"n","kind":1.5}],"edges":[]}"#
    )
    .is_err());
}
#[test]
fn bh_spec_missing_fields_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(r#"{"workflow_name":"w"}"#).is_err());
    assert!(serde_json::from_str::<WorkflowSpec>(r#"{"nodes":[],"edges":[]}"#).is_err());
}
#[test]
fn bh_spec_null_nodes_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(
        r#"{"workflow_name":"w","nodes":null,"edges":[]}"#
    )
    .is_err());
}
#[test]
fn bh_spec_unicode_name_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(
        r#"{"workflow_name":"w","nodes":[{"name":"日本語","kind":"pure"}],"edges":[]}"#
    )
    .is_err());
}
#[test]
fn bh_spec_double_dash_name_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(
        r#"{"workflow_name":"w","nodes":[{"name":"bad--n","kind":"pure"}],"edges":[]}"#
    )
    .is_err());
}
#[test]
fn bh_spec_trailing_dash_rejected() {
    assert!(serde_json::from_str::<WorkflowSpec>(
        r#"{"workflow_name":"w","nodes":[{"name":"n-","kind":"pure"}],"edges":[]}"#
    )
    .is_err());
}
#[test]
fn bh_node_handle_hash_ignores_phantom() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use vo_sdk::node_handle::NodeHandle;
    use vo_types::NodeName;
    let nm = NodeName::parse("x").unwrap();
    let h1: NodeHandle<String, i32> = NodeHandle::new(nm.clone());
    let h2: NodeHandle<bool, ()> = NodeHandle::new(nm);
    let (mut s1, mut s2) = (DefaultHasher::new(), DefaultHasher::new());
    h1.hash(&mut s1);
    h2.hash(&mut s2);
    assert_eq!(s1.finish(), s2.finish());
    assert_eq!(h1.name(), h2.name());
}
#[test]
fn bh_double_write_blocked() {
    use vo_sdk::io::write_success_inner_with_state;
    let (mut w, mut buf) = (false, Vec::new());
    let v = serde_json::json!(42);
    write_success_inner_with_state(&mut buf, &v, &mut w).unwrap();
    assert!(write_success_inner_with_state(&mut buf, &v, &mut w).is_err());
}
#[test]
fn bh_failure_after_success_blocked() {
    use vo_sdk::io::{write_failure_inner_with_state, write_success_inner_with_state};
    let (mut w, mut buf) = (false, Vec::new());
    write_success_inner_with_state(&mut buf, &serde_json::json!(1), &mut w).unwrap();
    assert!(write_failure_inner_with_state(&mut buf, TaskFailureKind::User, "f", &mut w).is_err());
}
#[test]
fn bh_read_guard_independent() {
    use vo_sdk::io::{read_input_inner_with_state, write_success_inner_with_state};
    let (mut w, mut r) = (false, false);
    let mut buf = Vec::new();
    write_success_inner_with_state(&mut buf, &serde_json::json!("ok"), &mut w).unwrap();
    let mut data = &br#"{"idempotency_key":"k","data":1}"#[..];
    assert!(read_input_inner_with_state(&mut data, &mut r).is_ok());
}
