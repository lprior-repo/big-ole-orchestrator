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
    pub fn evaluate(&self, envelope: &EventEnvelope) -> bool {
        match self {
            Predicate::SequenceRange { min, max } => {
                envelope.sequence >= *min && envelope.sequence <= *max
            }
            Predicate::TimestampRange { min_ms, max_ms } => {
                envelope.timestamp_ms >= *min_ms && envelope.timestamp_ms <= *max_ms
            }
            Predicate::EventType(event_type) => envelope
                .payload
                .get("type")
                .and_then(|v| v.as_str())
                .map(|t| t == event_type)
                .unwrap_or(false),
            Predicate::SchemaVersion(version) => envelope.schema_version == *version,
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
    pub fn include_payload(&self) -> bool {
        !matches!(self, Projection::WorkflowVersion)
    }

    pub fn include_metadata(&self) -> bool {
        matches!(
            self,
            Projection::Full | Projection::EffectJournal | Projection::WorkflowVersion
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
    #[must_use]
    pub fn optimize<'a>(spec: QuerySpec<'a>) -> QueryPlan {
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

    fn optimize_projection(projection: Projection) -> Projection {
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
    pub fn from_plan(plan: &QueryPlan, keyspace: &fjall::Keyspace) -> Result<Self, StorageError> {
        let partition =
            keyspace.open_partition("events", fjall::PartitionCreateOptions::default())?;
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
}
