use crate::query_optimizer::cost::CostModel;
use crate::query_optimizer::logical::{LogicalPlan, PlanNode, Predicate, SortKey};
use crate::query_optimizer::optimizer::Optimizer;
use crate::query_optimizer::statistics::TableStatistics;
use crate::query_optimizer::{OptimizationResult, PhysicalPlan};

#[derive(Debug, Clone, PartialEq)]
pub struct QueryDescriptor {
    pub source: String,
    pub predicate: Option<Predicate>,
    pub projections: Vec<String>,
    pub sort_keys: Vec<SortKey>,
    pub limit: Option<u64>,
    pub offset: u64,
    pub join: Option<JoinDescriptor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinDescriptor {
    pub right_source: String,
    pub left_key: String,
    pub right_key: String,
}

impl QueryDescriptor {
    #[must_use]
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            predicate: None,
            projections: vec![],
            sort_keys: vec![],
            limit: None,
            offset: 0,
            join: None,
        }
    }

    #[must_use]
    pub fn with_predicate(mut self, predicate: Predicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    #[must_use]
    pub fn with_projections(mut self, projections: Vec<&str>) -> Self {
        self.projections = projections.into_iter().map(String::from).collect();
        self
    }

    #[must_use]
    pub fn with_sort(mut self, keys: Vec<SortKey>) -> Self {
        self.sort_keys = keys;
        self
    }

    #[must_use]
    pub fn with_limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    #[must_use]
    pub fn with_join(mut self, right_source: &str, left_key: &str, right_key: &str) -> Self {
        self.join = Some(JoinDescriptor {
            right_source: right_source.to_string(),
            left_key: left_key.to_string(),
            right_key: right_key.to_string(),
        });
        self
    }
}

pub struct QueryPlanner {
    optimizer: Optimizer,
}

impl QueryPlanner {
    #[must_use]
    pub fn new(cost_model: CostModel) -> Self {
        Self {
            optimizer: Optimizer::new(cost_model),
        }
    }

    pub fn with_statistics(
        cost_model: CostModel,
        statistics: std::collections::HashMap<String, TableStatistics>,
    ) -> Self {
        Self {
            optimizer: Optimizer::with_statistics(cost_model, statistics),
        }
    }

    pub fn update_statistics(&mut self, table: &str, stats: TableStatistics) {
        self.optimizer.update_statistics(table, stats);
    }

    pub fn plan(&self, descriptor: &QueryDescriptor) -> OptimizationResult<PhysicalPlan> {
        let logical = self.build_logical(descriptor);
        self.optimizer.optimize(&logical)
    }

    pub fn plan_debug(&self, descriptor: &QueryDescriptor) -> OptimizationResult<String> {
        let plan = self.plan(descriptor)?;
        Ok(plan.to_string())
    }

    fn build_logical(&self, desc: &QueryDescriptor) -> LogicalPlan {
        let scan = PlanNode::scan(&desc.source);

        let base = if let Some(join) = &desc.join {
            let right_scan = PlanNode::scan(&join.right_source);
            PlanNode::hash_join(scan, right_scan, &join.left_key, &join.right_key)
        } else {
            scan
        };

        let filtered = match &desc.predicate {
            Some(pred) => PlanNode::filter(pred.clone(), base),
            None => base,
        };

        let projected = if !desc.projections.is_empty() {
            PlanNode::project(
                desc.projections.iter().map(String::as_str).collect(),
                filtered,
            )
        } else {
            filtered
        };

        let sorted = if !desc.sort_keys.is_empty() {
            PlanNode::sort(desc.sort_keys.clone(), projected)
        } else {
            projected
        };

        let limited = match desc.limit {
            Some(count) => PlanNode::limit_with_offset(count, desc.offset, sorted),
            None => sorted,
        };

        LogicalPlan::new(limited)
    }
}

impl Default for QueryPlanner {
    fn default() -> Self {
        Self::new(CostModel::new())
    }
}

impl Default for QueryDescriptor {
    fn default() -> Self {
        Self::new("instances")
    }
}
