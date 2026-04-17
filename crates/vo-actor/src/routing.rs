//! Lineage-aware query routing for epoch resolution (ADR-038).
//!
//! Architecture: Data → Calc → Actions
//! - Data: `LineageQuery`, `ResolvedRoute`, `RoutingError`, `EpochResolver`
//! - Calc: Pure routing decisions, epoch resolution logic
//! - Actions: Query dispatch via resolved routes
//!
//! # Overview
//!
//! When a query targets a `lineage_id` without an explicit epoch, the router
//! resolves the currently active epoch from storage and routes to it. When an
//! explicit epoch is specified, the query is routed directly to that epoch's
//! instance (historical query).

use std::sync::Arc;
use thiserror::Error;
use vo_types::{Epoch, InstanceId, LineageState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageQuery {
    QueryByLineage {
        lineage_id: String,
        epoch: Option<Epoch>,
    },
}

impl LineageQuery {
    pub fn lineage_id(&self) -> &str {
        match self {
            Self::QueryByLineage { lineage_id, .. } => lineage_id,
        }
    }

    pub fn epoch(&self) -> Option<Epoch> {
        match self {
            Self::QueryByLineage { epoch, .. } => *epoch,
        }
    }

    pub fn is_historical(&self) -> bool {
        self.epoch().is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub lineage_id: String,
    pub target_epoch: Epoch,
    pub target_instance_id: InstanceId,
    pub is_active_epoch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RoutingError {
    #[error("lineage not found: {0}")]
    LineageNotFound(String),
    #[error("epoch {requested} does not exist for lineage {lineage_id}")]
    EpochNotFound { lineage_id: String, requested: Epoch },
    #[error("lineage is tombstoned: {0}")]
    LineageTombstoned(String),
    #[error("storage error: {0}")]
    StorageError(String),
}

impl RoutingError {
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::LineageNotFound(_) | Self::EpochNotFound { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLineageInfo {
    pub lineage_state: LineageState,
    pub active_instance_id: InstanceId,
}

pub trait EpochResolver: Send + Sync {
    fn resolve_active_epoch(
        &self,
        lineage_id: &str,
    ) -> impl std::future::Future<Output = Result<ActiveLineageInfo, RoutingError>> + Send;

    fn resolve_specific_epoch(
        &self,
        lineage_id: &str,
        epoch: Epoch,
    ) -> impl std::future::Future<Output = Result<InstanceId, RoutingError>> + Send;
}

pub struct LineageRouter<R: EpochResolver> {
    resolver: Arc<R>,
}

impl<R: EpochResolver> LineageRouter<R> {
    pub fn new(resolver: Arc<R>) -> Self {
        Self { resolver }
    }

    pub async fn route(&self, query: LineageQuery) -> Result<ResolvedRoute, RoutingError> {
        match query {
            LineageQuery::QueryByLineage {
                lineage_id,
                epoch: None,
            } => {
                let active_info = self.resolver.resolve_active_epoch(&lineage_id).await?;
                if !active_info.lineage_state.can_spawn_epoch() {
                    return Err(RoutingError::LineageTombstoned(lineage_id));
                }
                Ok(ResolvedRoute {
                    lineage_id: lineage_id.clone(),
                    target_epoch: active_info.lineage_state.epoch(),
                    target_instance_id: active_info.active_instance_id,
                    is_active_epoch: true,
                })
            }
            LineageQuery::QueryByLineage {
                lineage_id,
                epoch: Some(requested_epoch),
            } => {
                let active_info = self.resolver.resolve_active_epoch(&lineage_id).await?;
                if !active_info.lineage_state.can_spawn_epoch() {
                    return Err(RoutingError::LineageTombstoned(lineage_id));
                }
                let instance_id =
                    self.resolver
                        .resolve_specific_epoch(&lineage_id, requested_epoch)
                        .await?;
                Ok(ResolvedRoute {
                    lineage_id,
                    target_epoch: requested_epoch,
                    target_instance_id: instance_id,
                    is_active_epoch: requested_epoch == active_info.lineage_state.epoch(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{LineageStatus, WorkflowLineage};

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
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").map_err(|e| RoutingError::StorageError(e.to_string()))
        }
    }

    #[tokio::test]
    async fn route_query_without_epoch_resolves_to_active_epoch() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-1", Epoch::new(3), LineageStatus::Active);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: None,
        };

        let result = router.route(query).await.unwrap();
        assert_eq!(result.lineage_id, "lin-1");
        assert_eq!(result.target_epoch, Epoch::new(3));
        assert!(result.is_active_epoch);
    }

    #[tokio::test]
    async fn route_query_with_explicit_epoch_routes_to_that_epoch() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-1", Epoch::new(3), LineageStatus::Active);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(1)),
        };

        let result = router.route(query).await.unwrap();
        assert_eq!(result.lineage_id, "lin-1");
        assert_eq!(result.target_epoch, Epoch::new(1));
        assert!(!result.is_active_epoch);
    }

    #[tokio::test]
    async fn route_nonexistent_lineage_returns_error() {
        let resolver = MockEpochResolver::new();
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "nonexistent".to_string(),
            epoch: None,
        };

        let result = router.route(query).await;
        assert!(matches!(result, Err(RoutingError::LineageNotFound(_))));
    }

    #[tokio::test]
    async fn route_tombstoned_lineage_returns_error() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-dead", Epoch::ZERO, LineageStatus::Tombstoned);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-dead".to_string(),
            epoch: None,
        };

        let result = router.route(query).await;
        assert!(matches!(result, Err(RoutingError::LineageTombstoned(_))));
    }

    #[test]
    fn lineage_query_epoch_accessor() {
        let q = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(5)),
        };
        assert_eq!(q.epoch(), Some(Epoch::new(5)));
        assert!(q.is_historical());
    }

    #[test]
    fn lineage_query_without_epoch_is_not_historical() {
        let q = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: None,
        };
        assert!(q.epoch().is_none());
        assert!(!q.is_historical());
    }

    #[test]
    fn routing_error_is_not_found() {
        assert!(RoutingError::LineageNotFound("x".to_string()).is_not_found());
        assert!(RoutingError::EpochNotFound {
            lineage_id: "x".to_string(),
            requested: Epoch::ZERO
        }
        .is_not_found());
        assert!(!RoutingError::LineageTombstoned("x".to_string()).is_not_found());
        assert!(!RoutingError::StorageError("x".to_string()).is_not_found());
    }

    #[tokio::test]
    async fn route_requested_epoch_equals_active_epoch_sets_is_active() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-1", Epoch::new(3), LineageStatus::Active);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(3)),
        };

        let result = router.route(query).await.unwrap();
        assert_eq!(result.lineage_id, "lin-1");
        assert_eq!(result.target_epoch, Epoch::new(3));
        assert!(result.is_active_epoch, "is_active_epoch should be true when requested epoch equals active epoch");
    }

    #[tokio::test]
    async fn route_requested_epoch_greater_than_active_returns_not_found() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-1", Epoch::new(3), LineageStatus::Active);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(10)),
        };

        let result = router.route(query).await;
        assert!(matches!(result, Err(RoutingError::EpochNotFound { .. })), "Should return EpochNotFound when requested epoch does not exist");
    }

    #[tokio::test]
    async fn route_multiple_sequential_calls_are_consistent() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-1", Epoch::new(3), LineageStatus::Active);
        let router = LineageRouter::new(Arc::new(resolver));

        let query1 = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(1)),
        };
        let query2 = LineageQuery::QueryByLineage {
            lineage_id: "lin-1".to_string(),
            epoch: Some(Epoch::new(1)),
        };

        let result1 = router.route(query1).await.unwrap();
        let result2 = router.route(query2).await.unwrap();

        assert_eq!(result1, result2, "Sequential calls with same lineage and epoch should return consistent results");
        assert_eq!(result1.target_epoch, result2.target_epoch);
        assert_eq!(result1.target_instance_id, result2.target_instance_id);
        assert_eq!(result1.is_active_epoch, result2.is_active_epoch);
    }

    #[tokio::test]
    async fn route_lineage_not_found_returns_lineage_not_found_not_epoch_not_found() {
        let resolver = MockEpochResolver::new();
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "nonexistent".to_string(),
            epoch: Some(Epoch::new(1)),
        };

        let result = router.route(query).await;
        assert!(matches!(result, Err(RoutingError::LineageNotFound(_))), "Should return LineageNotFound when lineage does not exist, not EpochNotFound");
    }

    #[tokio::test]
    async fn route_tombstoned_lineage_with_explicit_epoch_returns_tombstoned() {
        let resolver = MockEpochResolver::new()
            .with_lineage("lin-dead", Epoch::new(1), LineageStatus::Tombstoned);
        let router = LineageRouter::new(Arc::new(resolver));

        let query = LineageQuery::QueryByLineage {
            lineage_id: "lin-dead".to_string(),
            epoch: Some(Epoch::new(1)),
        };

        let result = router.route(query).await;
        assert!(matches!(result, Err(RoutingError::LineageTombstoned(_))), "Tombstoned lineage should return LineageTombstoned regardless of epoch specification");
    }
}
