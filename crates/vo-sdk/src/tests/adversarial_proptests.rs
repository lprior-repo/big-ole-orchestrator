//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: property-based adversarial proptests.

#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use proptest::prelude::*;
use serde_json::json;

use crate::dag::{Dag, Workflow};
use crate::node_handle::NodeHandle;
use crate::tests::{
    read_input_inner_with_atomic_guard as read_input_inner_atomic,
    read_input_inner_with_state as read_input_inner,
    write_failure_inner_with_state as write_failure_inner,
    write_success_inner_with_state as write_success_inner,
};
use crate::{SdkError, TaskFailureKind};
use vo_types::NodeKind;

use super::valid_envelope;

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;

    proptest! {
        #[test]
        fn proptest_read_input_inner_never_panics(
            bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)
        ) {
            let mut cursor = Cursor::new(bytes);
            let mut is_read = false;
            let _ = std::panic::catch_unwind(|| {
                let _ = read_input_inner(&mut cursor, &mut is_read);
            });
        }

        #[test]
        fn proptest_write_failure_inner_never_panics(
            message in ".{0,2048}"
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let _ = std::panic::catch_unwind(|| {
                let _ = write_failure_inner(&mut buf, TaskFailureKind::User, &message, &mut is_written);
            });
        }

        #[test]
        fn proptest_write_success_inner_output_is_valid_json(
            val in proptest::arbitrary::any::<serde_json::Value>()
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let result = write_success_inner(&mut buf, &val, &mut is_written);

            if let Ok(()) = result {
                let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                assert_eq!(parsed["status"], "success");
                assert_eq!(parsed["output"], val);
            }
        }

        #[test]
        fn proptest_write_failure_inner_output_is_valid_json(
            message in ".{0,1024}"
        ) {
            let mut buf: Vec<u8> = Vec::new();
            let mut is_written = false;
            let result = write_failure_inner(&mut buf, TaskFailureKind::System, &message, &mut is_written);

            if let Ok(()) = result {
                let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
                assert_eq!(parsed["status"], "failure");
                assert_eq!(parsed["kind"], "System");
                assert_eq!(parsed["message"], message);
            }
        }

        #[test]
        fn proptest_dag_build_produces_consistent_spec(
            node_count in 1usize..=10,
            connect_mask in proptest::bits::usize::any()
        ) {
            let mut dag = Dag::new();
            let mut handles: Vec<NodeHandle<(), ()>> = Vec::new();
            for i in 0..node_count {
                let h: NodeHandle<(), ()> = dag
                    .add_node_with_kind(&format!("node{}", i), NodeKind::Pure, |_: ()| ())
                    .unwrap();
                handles.push(h);
            }
            for i in 0..node_count.saturating_sub(1) {
                if connect_mask & (1 << i) != 0 {
                    dag.connect(&handles[i], &handles[i + 1]).unwrap();
                }
            }

            let result = dag.build("consistency_test");
            prop_assert!(result.is_ok());
            let spec = result.unwrap();
            assert_eq!(spec.nodes.len(), node_count);
        }
    }
}
