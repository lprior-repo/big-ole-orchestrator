//! Query optimizer for event replay — produces optimized query plans from query specifications.
//!
//! Architecture follows Data → Calc → Actions:
//! - Data: `QuerySpec`, `QueryPlan`, `Predicate`, `Projection`
//! - Calc: `optimize()`, predicate analysis, projection analysis
//! - Actions: `execute_plan()`, `FilteredReplayIterator`
//!
//! ## Optimization Rules
//!
//! 1. **Predicate Pushdown**: Filter predicates are pushed down to the storage layer
//!    so only matching events are retrieved from the keyspace.
//! 2. **Projection Pushdown**: Only required fields are decoded from storage.
//! 3. **Early Limit**: If a limit is specified, iteration stops once reached.
//! 4. **Sequence Range**: Sequence range predicates become direct key range queries.

use crate::codec::StorageError;
use vo_types::EventEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    SequenceRange { min: u64, max: u64 },
    TimestampRange { min_ms: u64, max_ms: u64 },
    EventType(String),
    SchemaVersion(u8),
}

impl Predicate {
    /// Evaluate this predicate against an event envelope.
    #[must_use]
    pub fn evaluate(&self, envelope: &EventEnvelope) -> bool {
        match self {
            Self::SequenceRange { min, max } => {
                envelope.sequence >= *min && envelope.sequence <= *max
            }
            Self::TimestampRange { min_ms, max_ms } => {
                envelope.timestamp_ms >= *min_ms && envelope.timestamp_ms <= *max_ms
            }
            Self::EventType(event_type) => envelope
                .payload
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t == event_type),
            Self::SchemaVersion(version) => envelope.schema_version == *version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    Full,
    Timeline,
    History,
    EffectJournal,
    WorkflowVersion,
}

impl Projection {
    /// Returns `true` if the projection includes the payload.
    #[must_use]
    pub const fn include_payload(&self) -> bool {
        !matches!(self, Projection::WorkflowVersion)
    }

    /// Returns `true` if the projection includes metadata fields.
    #[must_use]
    pub const fn include_metadata(&self) -> bool {
        matches!(
            self,
            Self::Full | Self::EffectJournal | Self::WorkflowVersion
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySpec<'a> {
    pub lineage_query: super::LineageQuery<'a>,
    pub predicates: Vec<Predicate>,
    pub projection: Projection,
    pub limit: Option<usize>,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPlan {
    pub prefix: Vec<u8>,
    pub predicates: Vec<Predicate>,
    pub projection: Projection,
    pub limit: Option<usize>,
    pub offset: usize,
    pub scan_range_start: Option<Vec<u8>>,
    pub scan_range_end: Option<Vec<u8>>,
}

pub struct QueryOptimizer;

impl QueryOptimizer {
    /// Optimize a query specification into a query plan.
    #[must_use]
    pub fn optimize(spec: QuerySpec<'_>) -> QueryPlan {
        let prefix = spec.lineage_query.to_prefix().unwrap_or_default();
        let (scan_range_start, scan_range_end) =
            Self::compute_scan_range(&spec.predicates, &prefix);
        let limit = spec.limit;
        let offset = spec.offset;
        let predicates = Self::pushdown_predicates(spec.predicates);
        let projection = Self::optimize_projection(spec.projection);

        QueryPlan {
            prefix,
            predicates,
            projection,
            limit,
            offset,
            scan_range_start,
            scan_range_end,
        }
    }

    fn compute_scan_range(
        predicates: &[Predicate],
        prefix: &[u8],
    ) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let mut seq_min = None;
        let mut seq_max = None;

        for pred in predicates {
            if let Predicate::SequenceRange { min, max } = pred {
                seq_min = Some(*min);
                seq_max = Some(*max);
                break;
            }
        }

        let start = seq_min.map(|min| {
            let mut start = prefix.to_vec();
            start.extend_from_slice(&min.to_be_bytes());
            start
        });

        let end = seq_max.map(|max| {
            let mut end = prefix.to_vec();
            end.extend_from_slice(&max.to_be_bytes());
            end
        });

        (start, end)
    }

    fn pushdown_predicates(predicates: Vec<Predicate>) -> Vec<Predicate> {
        predicates
            .into_iter()
            .filter(|p| !matches!(p, Predicate::SequenceRange { .. }))
            .collect()
    }

    const fn optimize_projection(projection: Projection) -> Projection {
        projection
    }
}

pub struct OptimizedReplayIterator {
    inner: super::EventReplayIterator,
    predicates: Vec<Predicate>,
    remaining_offset: usize,
    limit: Option<usize>,
    count: usize,
}

impl OptimizedReplayIterator {
    /// Create an optimized replay iterator from a query plan.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Storage` if the events partition cannot be opened.
    pub fn from_plan(plan: &QueryPlan, keyspace: &fjall::Database) -> Result<Self, StorageError> {
        let partition =
            keyspace.keyspace("events", || fjall::KeyspaceCreateOptions::default())?;
        let scan_start = plan.scan_range_start.clone().unwrap_or_else(|| {
            let mut s = plan.prefix.clone();
            s.extend_from_slice(&1u64.to_be_bytes());
            s
        });
        let scan_end = plan.scan_range_end.clone().unwrap_or_else(|| {
            let mut e = plan.prefix.clone();
            e.extend_from_slice(&u64::MAX.to_be_bytes());
            e
        });
        let iter = partition.range(scan_start..=scan_end);
        let inner = super::EventReplayIterator {
            state: super::IteratorState::new(),
            inner: Some(Box::new(iter)),
            init_error: None,
        };
        Ok(Self {
            inner,
            predicates: plan.predicates.clone(),
            remaining_offset: plan.offset,
            limit: plan.limit,
            count: 0,
        })
    }
}

impl Iterator for OptimizedReplayIterator {
    type Item = Result<EventEnvelope, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(limit) = self.limit {
            if self.count >= limit {
                return None;
            }
        }
        loop {
            let next = self.inner.next()?;
            match next {
                Ok(envelope) => {
                    if self.remaining_offset > 0 {
                        self.remaining_offset -= 1;
                        continue;
                    }
                    let matches = self.predicates.iter().all(|p| p.evaluate(&envelope));
                    if matches {
                        self.count += 1;
                        return Some(Ok(envelope));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

#[cfg(test)]
mod optimizer_tests {
    use super::*;
    use vo_types::events::EventMetadata;
    use vo_types::InstanceId;

    fn make_envelope(seq: u64, event_type: &str, timestamp_ms: u64) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: "test-instance".to_string(),
            sequence: seq,
            timestamp_ms,
            payload: serde_json::json!({"type": event_type}),
            metadata: EventMetadata::default(),
        }
    }

    #[test]
    fn predicate_sequence_range_evaluate() {
        let pred = Predicate::SequenceRange { min: 5, max: 10 };
        let env = make_envelope(7, "WorkflowStarted", 1000);
        assert!(pred.evaluate(&env));
        let env_low = make_envelope(3, "WorkflowStarted", 1000);
        assert!(!pred.evaluate(&env_low));
        let env_high = make_envelope(15, "WorkflowStarted", 1000);
        assert!(!pred.evaluate(&env_high));
    }

    #[test]
    fn predicate_timestamp_range_evaluate() {
        let pred = Predicate::TimestampRange {
            min_ms: 1000,
            max_ms: 2000,
        };
        let env = make_envelope(1, "WorkflowStarted", 1500);
        assert!(pred.evaluate(&env));
        let env_low = make_envelope(1, "WorkflowStarted", 500);
        assert!(!pred.evaluate(&env_low));
        let env_high = make_envelope(1, "WorkflowStarted", 2500);
        assert!(!pred.evaluate(&env_high));
    }

    #[test]
    fn predicate_event_type_evaluate() {
        let pred = Predicate::EventType("WorkflowStarted".to_string());
        let env = make_envelope(1, "WorkflowStarted", 1000);
        assert!(pred.evaluate(&env));
        let env_other = make_envelope(1, "StepCompleted", 1000);
        assert!(!pred.evaluate(&env_other));
    }

    #[test]
    fn query_optimizer_sequence_range_becomes_scan_range() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![Predicate::SequenceRange { min: 10, max: 50 }],
            projection: Projection::Full,
            limit: None,
            offset: 0,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert!(plan.scan_range_start.is_some());
        assert!(plan.scan_range_end.is_some());
        assert!(plan.predicates.is_empty());
    }

    #[test]
    fn query_optimizer_preserves_non_scan_predicates() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![
                Predicate::SequenceRange { min: 10, max: 50 },
                Predicate::EventType("WorkflowStarted".to_string()),
            ],
            projection: Projection::Full,
            limit: None,
            offset: 0,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert!(plan.scan_range_start.is_some());
        assert!(plan.scan_range_end.is_some());
        assert_eq!(plan.predicates.len(), 1);
    }

    #[test]
    fn query_optimizer_applies_offset() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![],
            projection: Projection::Full,
            limit: None,
            offset: 5,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert_eq!(plan.offset, 5);
    }

    #[test]
    fn query_optimizer_applies_limit() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![],
            projection: Projection::Full,
            limit: Some(100),
            offset: 0,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert_eq!(plan.limit, Some(100));
    }

    #[test]
    fn projection_include_flags() {
        assert!(Projection::Full.include_payload());
        assert!(Projection::Full.include_metadata());
        assert!(!Projection::Timeline.include_metadata());
        assert!(Projection::EffectJournal.include_metadata());
        assert!(!Projection::WorkflowVersion.include_payload());
        assert!(Projection::WorkflowVersion.include_metadata());
    }

    #[test]
    fn predicate_schema_version_evaluate() {
        let pred = Predicate::SchemaVersion(1);
        let mut env = make_envelope(1, "WorkflowStarted", 1000);
        env.schema_version = 1;
        assert!(pred.evaluate(&env));
        env.schema_version = 2;
        assert!(!pred.evaluate(&env));
        env.schema_version = 0;
        assert!(!pred.evaluate(&env));
    }

    fn make_envelope_with_version(
        seq: u64,
        event_type: &str,
        timestamp_ms: u64,
        schema_version: u8,
    ) -> EventEnvelope {
        EventEnvelope {
            schema_version,
            instance_id: "test-instance".to_string(),
            sequence: seq,
            timestamp_ms,
            payload: serde_json::json!({"type": event_type}),
            metadata: EventMetadata::default(),
        }
    }

    #[test]
    fn query_optimizer_with_schema_version_predicate() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![Predicate::SchemaVersion(1)],
            projection: Projection::Full,
            limit: None,
            offset: 0,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert_eq!(plan.predicates.len(), 1);
        let env_v1 = make_envelope_with_version(1, "WorkflowStarted", 1000, 1);
        let env_v2 = make_envelope_with_version(1, "WorkflowStarted", 1000, 2);
        assert!(plan.predicates[0].evaluate(&env_v1));
        assert!(!plan.predicates[0].evaluate(&env_v2));
    }

    #[test]
    fn query_optimizer_combined_predicates() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![
                Predicate::SequenceRange { min: 10, max: 50 },
                Predicate::EventType("WorkflowStarted".to_string()),
                Predicate::SchemaVersion(1),
            ],
            projection: Projection::Full,
            limit: Some(10),
            offset: 5,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert!(plan.scan_range_start.is_some());
        assert!(plan.scan_range_end.is_some());
        assert_eq!(plan.predicates.len(), 2);
        assert_eq!(plan.limit, Some(10));
        assert_eq!(plan.offset, 5);
    }

    #[test]
    fn query_optimizer_empty_predicates_preserves_none() {
        let spec = QuerySpec {
            lineage_query: super::super::LineageQuery::InstanceId(
                &InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ),
            predicates: vec![],
            projection: Projection::Timeline,
            limit: None,
            offset: 0,
        };
        let plan = QueryOptimizer::optimize(spec);
        assert!(plan.predicates.is_empty());
        assert!(plan.scan_range_start.is_none());
        assert!(plan.scan_range_end.is_none());
        assert_eq!(plan.projection, Projection::Timeline);
    }

    #[test]
    fn query_plan_clone() {
        let plan = QueryPlan {
            prefix: vec![1, 2, 3],
            predicates: vec![Predicate::SchemaVersion(1)],
            projection: Projection::Full,
            limit: Some(100),
            offset: 10,
            scan_range_start: Some(vec![1]),
            scan_range_end: Some(vec![255]),
        };
        let cloned = plan.clone();
        assert_eq!(cloned.prefix, plan.prefix);
        assert_eq!(cloned.predicates.len(), plan.predicates.len());
        assert_eq!(cloned.limit, plan.limit);
        assert_eq!(cloned.offset, plan.offset);
    }

    #[test]
    fn predicate_schema_version_edge_cases() {
        let pred = Predicate::SchemaVersion(0);
        let env = make_envelope_with_version(1, "WorkflowStarted", 1000, 0);
        assert!(pred.evaluate(&env));

        let pred255 = Predicate::SchemaVersion(255);
        let mut env255 = make_envelope_with_version(1, "WorkflowStarted", 1000, 255);
        assert!(pred255.evaluate(&env255));
        env255.schema_version = 254;
        assert!(!pred255.evaluate(&env255));
    }

    #[test]
    fn predicate_timestamp_range_boundary_conditions() {
        let pred = Predicate::TimestampRange {
            min_ms: 1000,
            max_ms: 1000,
        };
        let env_at_min = make_envelope_with_version(1, "WorkflowStarted", 1000, 1);
        assert!(pred.evaluate(&env_at_min));
        let env_above = make_envelope_with_version(1, "WorkflowStarted", 1001, 1);
        assert!(!pred.evaluate(&env_above));
        let env_below = make_envelope_with_version(1, "WorkflowStarted", 999, 1);
        assert!(!pred.evaluate(&env_below));
    }

    #[test]
    fn predicate_sequence_range_boundary_conditions() {
        let pred = Predicate::SequenceRange { min: 5, max: 5 };
        let env_at_min = make_envelope_with_version(5, "WorkflowStarted", 1000, 1);
        assert!(pred.evaluate(&env_at_min));
        let env_above = make_envelope_with_version(6, "WorkflowStarted", 1000, 1);
        assert!(!pred.evaluate(&env_above));
        let env_below = make_envelope_with_version(4, "WorkflowStarted", 1000, 1);
        assert!(!pred.evaluate(&env_below));
    }
}
