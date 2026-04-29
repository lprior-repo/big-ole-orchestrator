//! Lineage graph construction and traversal (ADR-038).
//!
//! Pure functions for routing events to epochs and querying graph state.

use crate::lineage_projection::types::*;

/// Route an incoming event to the correct epoch.
///
/// Returns RouteResult indicating how the event should be handled.
pub fn route_event(epoch_map: &EpochMap, event: &CanonicalEvent) -> RouteResult {
    if epoch_map.is_rollover_in_progress() {
        if let Some(active) = epoch_map.active_epoch(&event.lineage_id) {
            if active != event.epoch_id {
                return RouteResult::Buffered {
                    lineage_id: event.lineage_id.clone(),
                    epoch_id: event.epoch_id,
                };
            }
        } else {
            return RouteResult::Buffered {
                lineage_id: event.lineage_id.clone(),
                epoch_id: event.epoch_id,
            };
        }
    }

    if let Some(active) = epoch_map.active_epoch(&event.lineage_id) {
        if active == event.epoch_id {
            return RouteResult::Routed {
                lineage_id: event.lineage_id.clone(),
                epoch_id: event.epoch_id,
                routed_to_active: true,
            };
        } else if event.epoch_id.as_u64() < active.as_u64() {
            return RouteResult::OldEpochRejected {
                lineage_id: event.lineage_id.clone(),
                event_epoch: event.epoch_id,
                active_epoch: active,
            };
        }
    }

    RouteResult::NewLineage {
        lineage_id: event.lineage_id.clone(),
        epoch_id: event.epoch_id,
    }
}

/// Check if an epoch is historical (not the active epoch).
pub fn is_historical_epoch(
    epoch_map: &EpochMap,
    lineage_id: &LineageId,
    epoch_id: EpochId,
) -> bool {
    epoch_map.is_old_epoch(lineage_id, epoch_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lineage(id: &str) -> LineageId {
        LineageId(id.to_string())
    }

    fn test_epoch(n: u64) -> EpochId {
        EpochId(n)
    }

    fn test_event(
        lineage: LineageId,
        epoch: EpochId,
        seq: u64,
        event_type: &str,
    ) -> CanonicalEvent {
        CanonicalEvent {
            lineage_id: lineage,
            epoch_id: epoch,
            sequence: seq,
            event_type: event_type.to_string(),
            payload: serde_json::json!({"test": true}),
        }
    }

    #[test]
    fn route_event_routed_to_active() {
        let mut epoch_map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        epoch_map.register_epoch(lineage.clone(), test_epoch(1));
        let event = test_event(lineage.clone(), test_epoch(1), 1, "test");
        let result = route_event(&epoch_map, &event);
        match result {
            RouteResult::Routed {
                routed_to_active: true,
                ..
            } => {}
            other => panic!("expected Routed, got {:?}", other),
        }
    }

    #[test]
    fn route_event_old_epoch_rejected() {
        let mut epoch_map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        epoch_map.register_epoch(lineage.clone(), test_epoch(2));
        let event = test_event(lineage.clone(), test_epoch(1), 1, "test");
        let result = route_event(&epoch_map, &event);
        match result {
            RouteResult::OldEpochRejected {
                event_epoch: test_epoch(1),
                active_epoch: test_epoch(2),
                ..
            } => {}
            other => panic!("expected OldEpochRejected, got {:?}", other),
        }
    }

    #[test]
    fn route_event_new_lineage() {
        let epoch_map = EpochMap::new();
        let lineage = test_lineage("new-wf");
        let event = test_event(lineage.clone(), test_epoch(1), 1, "test");
        let result = route_event(&epoch_map, &event);
        match result {
            RouteResult::NewLineage {
                lineage_id,
                epoch_id: test_epoch(1),
            } => {
                assert_eq!(lineage_id, lineage);
            }
            other => panic!("expected NewLineage, got {:?}", other),
        }
    }

    #[test]
    fn route_event_buffered_during_rollover() {
        let mut epoch_map = EpochMap::new();
        epoch_map.set_rollover_in_progress(true);
        let lineage = test_lineage("wf-1");
        let event = test_event(lineage.clone(), test_epoch(1), 1, "test");
        let result = route_event(&epoch_map, &event);
        match result {
            RouteResult::Buffered { .. } => {}
            other => panic!("expected Buffered, got {:?}", other),
        }
    }

    #[test]
    fn epoch_map_new_is_empty() {
        let map = EpochMap::new();
        assert!(map.entries.is_empty());
        assert!(!map.is_rollover_in_progress());
    }

    #[test]
    fn epoch_map_returns_active_epoch() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        let epoch = test_epoch(1);
        map.register_epoch(lineage.clone(), epoch);
        assert_eq!(map.active_epoch(&lineage), Some(epoch));
    }

    #[test]
    fn epoch_map_is_active_returns_true() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        let epoch = test_epoch(1);
        map.register_epoch(lineage.clone(), epoch);
        assert!(map.is_active(&lineage, epoch));
    }

    #[test]
    fn epoch_map_is_active_returns_false_for_wrong_epoch() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        map.register_epoch(lineage.clone(), test_epoch(1));
        assert!(!map.is_active(&lineage, test_epoch(2)));
    }

    #[test]
    fn epoch_map_is_old_epoch_returns_true() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        map.register_epoch(lineage.clone(), test_epoch(3));
        assert!(map.is_old_epoch(&lineage, test_epoch(1)));
        assert!(map.is_old_epoch(&lineage, test_epoch(2)));
    }

    #[test]
    fn epoch_map_is_old_epoch_returns_false_for_active() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        map.register_epoch(lineage.clone(), test_epoch(3));
        assert!(!map.is_old_epoch(&lineage, test_epoch(3)));
    }

    #[test]
    fn epoch_map_register_and_unregister() {
        let mut map = EpochMap::new();
        let lineage = test_lineage("wf-1");
        map.register_epoch(lineage.clone(), test_epoch(1));
        assert!(map.active_epoch(&lineage).is_some());
        map.unregister_epoch(&lineage);
        assert!(map.active_epoch(&lineage).is_none());
    }

    #[test]
    fn epoch_map_rollover_guard() {
        let mut map = EpochMap::new();
        assert!(!map.is_rollover_in_progress());
        map.set_rollover_in_progress(true);
        assert!(map.is_rollover_in_progress());
        map.set_rollover_in_progress(false);
        assert!(!map.is_rollover_in_progress());
    }

    #[test]
    fn signal_buffer_buffers_and_drains() {
        let mut buffer = SignalBuffer::new();
        let event = test_event(test_lineage("wf-1"), test_epoch(1), 42, "test");
        buffer.buffer(event.clone());
        assert!(buffer.has_pending(&event.lineage_id));
        assert_eq!(buffer.pending_count(), 1);
        let drained = buffer.drain(&event.lineage_id);
        assert_eq!(drained, vec![event]);
        assert!(!buffer.has_pending(&event.lineage_id));
    }

    #[test]
    fn signal_buffer_drain_empty_returns_none() {
        let mut buffer = SignalBuffer::new();
        let drained = buffer.drain(&test_lineage("nonexistent"));
        assert!(drained.is_empty());
    }

    #[test]
    fn signal_buffer_multiple_lineages() {
        let mut buffer = SignalBuffer::new();
        let e1 = test_event(test_lineage("wf-1"), test_epoch(1), 1, "s1");
        let e2 = test_event(test_lineage("wf-2"), test_epoch(1), 2, "s2");
        buffer.buffer(e1.clone());
        buffer.buffer(e2.clone());
        assert_eq!(buffer.pending_count(), 2);
        let drained = buffer.drain(&test_lineage("wf-1"));
        assert_eq!(drained, vec![e1]);
        assert_eq!(buffer.pending_count(), 1);
    }
}
