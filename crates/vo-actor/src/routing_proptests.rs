//! Property-based tests for routing module (ADR-038).
//!
//! These tests verify invariants on existing routing code using proptest.
//! Feature-gated behind `#[cfg(feature = "proptest")]`.

#![cfg(feature = "proptest")]

use proptest::prelude::*;
use std::sync::Arc;
use vo_types::{Epoch, InstanceId, LineageStatus, LineageState, WorkflowLineage};

use crate::routing::{
    ActiveLineageInfo, EpochResolver, LineageQuery, LineageRouter, RoutingError,
};

struct MockEpochResolver {
    epochs: std::collections::HashMap<String, ActiveLineageInfo>,
}

impl MockEpochResolver {
    fn new() -> Self {
        Self {
            epochs: std::collections::HashMap::new(),
        }
    }

    fn with_lineage(mut self, lineage_id: &str, epoch: Epoch, status: LineageStatus) -> Self {
        let lineage = if epoch == Epoch::ZERO {
            WorkflowLineage::new(lineage_id).unwrap()
        } else {
            WorkflowLineage::with_parent(lineage_id, epoch, Some(Epoch::ZERO)).unwrap()
        };
        let state = LineageState::with_status(lineage, status);
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let info = ActiveLineageInfo {
            lineage_state: state,
            active_instance_id: instance_id,
        };
        self.epochs.insert(lineage_id.to_string(), info);
        self
    }
}

impl EpochResolver for MockEpochResolver {
    async fn resolve_active_epoch(
        &self,
        lineage_id: &str,
    ) -> Result<ActiveLineageInfo, RoutingError> {
        self.epochs
            .get(lineage_id)
            .cloned()
            .ok_or_else(|| RoutingError::LineageNotFound(lineage_id.to_string()))
    }

    async fn resolve_specific_epoch(
        &self,
        lineage_id: &str,
        epoch: Epoch,
    ) -> Result<InstanceId, RoutingError> {
        let info = self
            .epochs
            .get(lineage_id)
            .ok_or_else(|| RoutingError::LineageNotFound(lineage_id.to_string()))?;
        if info.lineage_state.epoch() < epoch {
            return Err(RoutingError::EpochNotFound {
                lineage_id: lineage_id.to_string(),
                requested: epoch,
            });
        }
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA")
            .map_err(|e| RoutingError::StorageError(e.to_string()))
    }
}

// Invariant 1: route() with explicit epoch sets is_active_epoch correctly.
//
// Invariant: When route() is called with Some(epoch), is_active_epoch must
// equal (requested_epoch == resolved_active_epoch).
//
// Strategy: Generate active_epoch in 0..100, requested_epoch in 0..active_epoch+1.
// Anti-invariant: is_active_epoch == true when requested != active.

proptest! {
    #[test]
    fn route_explicit_epoch_is_active_epoch_matches_resolved(
        active_epoch_val in 0u64..100u64,
        requested_offset in 0u64..100u64
    ) -> Result<(), TestCaseError> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let active_epoch = Epoch::new(active_epoch_val);
            let requested_epoch = Epoch::new(requested_offset % (active_epoch_val + 1));

            let resolver = MockEpochResolver::new()
                .with_lineage("lin-propt", active_epoch, LineageStatus::Active);
            let router = LineageRouter::new(Arc::new(resolver));

            let query = LineageQuery::QueryByLineage {
                lineage_id: "lin-propt".to_string(),
                epoch: Some(requested_epoch),
            };

            let route = router.route(query).await.unwrap();
            prop_assert_eq!(
                route.is_active_epoch,
                requested_epoch == active_epoch,
                "is_active_epoch should be true iff requested == active"
            );
            prop_assert_eq!(route.target_epoch, requested_epoch);
            Ok(())
        })
    }
}
