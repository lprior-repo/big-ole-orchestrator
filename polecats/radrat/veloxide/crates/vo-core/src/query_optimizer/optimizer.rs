use crate::query_optimizer::cost::CostModel;
use crate::query_optimizer::logical::{LogicalPlan, PlanNode, Predicate};
use crate::query_optimizer::statistics::TableStatistics;
use crate::query_optimizer::{Cost, OptimizationResult, PhysicalNode, PhysicalPlan};

pub struct Optimizer {
    cost_model: CostModel,
    statistics: std::collections::HashMap<String, TableStatistics>,
}

impl Optimizer {
    #[must_use]
    pub fn new(cost_model: CostModel) -> Self {
        Self {
            cost_model,
            statistics: std::collections::HashMap::new(),
        }
    }

    pub fn with_statistics(
        cost_model: CostModel,
        statistics: std::collections::HashMap<String, TableStatistics>,
    ) -> Self {
        Self {
            cost_model,
            statistics,
        }
    }

    pub fn update_statistics(&mut self, table: &str, stats: TableStatistics) {
        self.statistics.insert(table.to_string(), stats);
    }

    pub fn get_statistics(&self, table: &str) -> Option<&TableStatistics> {
        self.statistics.get(table)
    }

    pub fn optimize(&self, plan: &LogicalPlan) -> OptimizationResult<PhysicalPlan> {
        let optimized_logical = self.apply_rules(&plan.root);
        let physical = self.to_physical(&optimized_logical)?;
        let cost = self.compute_node_cost(&physical)?;
        if !cost.is_finite() {
            return Err(crate::query_optimizer::OptimizationError::CostOverflow);
        }
        Ok(PhysicalPlan {
            root: physical,
            cost,
        })
    }

    fn apply_rules(&self, node: &PlanNode) -> PlanNode {
        let after_pushdown = self.predicate_pushdown(node);
        let after_prune = self.projection_pruning(&after_pushdown);
        let after_fusion = self.scan_filter_fusion(&after_prune);
        self.limit_pushdown(&after_fusion)
    }

    fn predicate_pushdown(&self, node: &PlanNode) -> PlanNode {
        match node {
            PlanNode::Filter { predicate, input } => {
                let pushed_input = self.predicate_pushdown(input);
                match pushed_input {
                    PlanNode::Project {
                        columns,
                        input: proj_input,
                    } => {
                        let cols_referenced: Vec<String> = predicate.referenced_columns();
                        let all_available = cols_referenced.iter().all(|c| columns.contains(c));
                        if all_available {
                            PlanNode::Project {
                                columns,
                                input: Box::new(PlanNode::Filter {
                                    predicate: predicate.clone(),
                                    input: proj_input,
                                }),
                            }
                        } else {
                            PlanNode::Filter {
                                predicate: predicate.clone(),
                                input: Box::new(PlanNode::Project {
                                    columns,
                                    input: proj_input,
                                }),
                            }
                        }
                    }
                    PlanNode::Sort {
                        keys,
                        input: sort_input,
                    } => PlanNode::Sort {
                        keys,
                        input: Box::new(PlanNode::Filter {
                            predicate: predicate.clone(),
                            input: sort_input,
                        }),
                    },
                    PlanNode::Limit {
                        count,
                        offset,
                        input: lim_input,
                    } => PlanNode::Limit {
                        count,
                        offset,
                        input: Box::new(PlanNode::Filter {
                            predicate: predicate.clone(),
                            input: lim_input,
                        }),
                    },
                    other => PlanNode::Filter {
                        predicate: predicate.clone(),
                        input: Box::new(other),
                    },
                }
            }
            PlanNode::Sort { keys, input } => PlanNode::Sort {
                keys: keys.clone(),
                input: Box::new(self.predicate_pushdown(input)),
            },
            PlanNode::Project { columns, input } => PlanNode::Project {
                columns: columns.clone(),
                input: Box::new(self.predicate_pushdown(input)),
            },
            PlanNode::Limit {
                count,
                offset,
                input,
            } => PlanNode::Limit {
                count: *count,
                offset: *offset,
                input: Box::new(self.predicate_pushdown(input)),
            },
            PlanNode::HashJoin {
                left,
                right,
                left_key,
                right_key,
            } => PlanNode::HashJoin {
                left: Box::new(self.predicate_pushdown(left)),
                right: Box::new(self.predicate_pushdown(right)),
                left_key: left_key.clone(),
                right_key: right_key.clone(),
            },
            PlanNode::Merge { sources } => PlanNode::Merge {
                sources: sources.iter().map(|s| self.predicate_pushdown(s)).collect(),
            },
            PlanNode::Scan { .. } | PlanNode::Empty { .. } => node.clone(),
        }
    }

    fn projection_pruning(&self, node: &PlanNode) -> PlanNode {
        match node {
            PlanNode::Project { columns, input } => {
                let pruned_input = self.projection_pruning(input);
                let mut needed_columns: std::collections::HashSet<String> =
                    columns.iter().cloned().collect();
                self.collect_filter_columns(&pruned_input, &mut needed_columns);
                self.collect_join_columns(&pruned_input, &mut needed_columns);
                self.collect_sort_columns(&pruned_input, &mut needed_columns);
                PlanNode::Project {
                    columns: columns.clone(),
                    input: Box::new(pruned_input),
                }
            }
            PlanNode::Filter { predicate, input } => PlanNode::Filter {
                predicate: predicate.clone(),
                input: Box::new(self.projection_pruning(input)),
            },
            PlanNode::Sort { keys, input } => PlanNode::Sort {
                keys: keys.clone(),
                input: Box::new(self.projection_pruning(input)),
            },
            PlanNode::Limit {
                count,
                offset,
                input,
            } => PlanNode::Limit {
                count: *count,
                offset: *offset,
                input: Box::new(self.projection_pruning(input)),
            },
            PlanNode::HashJoin {
                left,
                right,
                left_key,
                right_key,
            } => PlanNode::HashJoin {
                left: Box::new(self.projection_pruning(left)),
                right: Box::new(self.projection_pruning(right)),
                left_key: left_key.clone(),
                right_key: right_key.clone(),
            },
            PlanNode::Merge { sources } => PlanNode::Merge {
                sources: sources.iter().map(|s| self.projection_pruning(s)).collect(),
            },
            PlanNode::Scan { .. } | PlanNode::Empty { .. } => node.clone(),
        }
    }

    fn collect_filter_columns(
        &self,
        node: &PlanNode,
        columns: &mut std::collections::HashSet<String>,
    ) {
        if let PlanNode::Filter { predicate, .. } = node {
            for col in predicate.referenced_columns() {
                columns.insert(col);
            }
        }
    }

    fn collect_join_columns(
        &self,
        node: &PlanNode,
        columns: &mut std::collections::HashSet<String>,
    ) {
        if let PlanNode::HashJoin {
            left_key,
            right_key,
            ..
        } = node
        {
            columns.insert(left_key.clone());
            columns.insert(right_key.clone());
        }
    }

    fn collect_sort_columns(
        &self,
        node: &PlanNode,
        columns: &mut std::collections::HashSet<String>,
    ) {
        if let PlanNode::Sort { keys, .. } = node {
            for key in keys {
                columns.insert(key.column.clone());
            }
        }
    }

    fn scan_filter_fusion(&self, node: &PlanNode) -> PlanNode {
        match node {
            PlanNode::Filter { predicate, input } => {
                let fused_input = self.scan_filter_fusion(input);
                match &fused_input {
                    PlanNode::Filter {
                        predicate: inner_pred,
                        input: inner_input,
                    } => {
                        let fused = Predicate::and(vec![predicate.clone(), inner_pred.clone()]);
                        PlanNode::Filter {
                            predicate: fused,
                            input: inner_input.clone(),
                        }
                    }
                    other => PlanNode::Filter {
                        predicate: predicate.clone(),
                        input: Box::new(other.clone()),
                    },
                }
            }
            PlanNode::Project { columns, input } => PlanNode::Project {
                columns: columns.clone(),
                input: Box::new(self.scan_filter_fusion(input)),
            },
            PlanNode::Sort { keys, input } => PlanNode::Sort {
                keys: keys.clone(),
                input: Box::new(self.scan_filter_fusion(input)),
            },
            PlanNode::Limit {
                count,
                offset,
                input,
            } => PlanNode::Limit {
                count: *count,
                offset: *offset,
                input: Box::new(self.scan_filter_fusion(input)),
            },
            PlanNode::HashJoin {
                left,
                right,
                left_key,
                right_key,
            } => PlanNode::HashJoin {
                left: Box::new(self.scan_filter_fusion(left)),
                right: Box::new(self.scan_filter_fusion(right)),
                left_key: left_key.clone(),
                right_key: right_key.clone(),
            },
            PlanNode::Merge { sources } => PlanNode::Merge {
                sources: sources.iter().map(|s| self.scan_filter_fusion(s)).collect(),
            },
            PlanNode::Scan { .. } | PlanNode::Empty { .. } => node.clone(),
        }
    }

    fn limit_pushdown(&self, node: &PlanNode) -> PlanNode {
        match node {
            PlanNode::Limit {
                count,
                offset,
                input,
            } => {
                let pushed = self.limit_pushdown_inner(*count, *offset, input);
                PlanNode::Limit {
                    count: *count,
                    offset: *offset,
                    input: Box::new(pushed),
                }
            }
            other => other.clone(),
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn limit_pushdown_inner(&self, count: u64, _offset: u64, node: &PlanNode) -> PlanNode {
        match node {
            PlanNode::Sort { keys, input } => {
                let inner = self.limit_pushdown_inner(count, 0, input);
                PlanNode::Sort {
                    keys: keys.clone(),
                    input: Box::new(inner),
                }
            }
            PlanNode::Filter { predicate, input } => PlanNode::Filter {
                predicate: predicate.clone(),
                input: Box::new(self.limit_pushdown_inner(count, 0, input)),
            },
            PlanNode::HashJoin {
                left,
                right,
                left_key,
                right_key,
            } => PlanNode::HashJoin {
                left: Box::new(self.limit_pushdown_inner(count, 0, left)),
                right: Box::new(self.limit_pushdown_inner(count, 0, right)),
                left_key: left_key.clone(),
                right_key: right_key.clone(),
            },
            PlanNode::Project { columns, input } => PlanNode::Project {
                columns: columns.clone(),
                input: Box::new(self.limit_pushdown_inner(count, 0, input)),
            },
            PlanNode::Merge { sources } => PlanNode::Merge {
                sources: sources
                    .iter()
                    .map(|s| self.limit_pushdown_inner(count, 0, s))
                    .collect(),
            },
            other => other.clone(),
        }
    }

    fn to_physical(&self, node: &PlanNode) -> OptimizationResult<PhysicalNode> {
        let input_rows = self.estimate_input_rows(node);
        match node {
            PlanNode::Scan {
                source,
                columns,
                alias,
            } => {
                let stats = self.statistics.get(source);
                let (strategy, cost) = if let Some(s) = stats {
                    (
                        super::physical::AccessStrategy::IndexScan {
                            index_hint: columns.first().cloned(),
                        },
                        self.cost_model.index_scan_cost(s.row_count, s.row_count),
                    )
                } else {
                    (
                        super::physical::AccessStrategy::FullScan,
                        self.cost_model.full_scan_cost(input_rows),
                    )
                };
                Ok(PhysicalNode::Scan {
                    source: source.clone(),
                    columns: columns.clone(),
                    alias: alias.clone(),
                    strategy,
                    estimated_rows: cost.estimated_rows,
                })
            }
            PlanNode::Filter { predicate, input } => {
                let physical_input = self.to_physical(input)?;
                let selectivity = self.estimate_selectivity(predicate);
                let cost = self
                    .cost_model
                    .filter_cost(physical_input.estimated_rows(), selectivity);
                Ok(PhysicalNode::Filter {
                    predicate: predicate.clone(),
                    input: Box::new(physical_input),
                    estimated_rows: cost.estimated_rows,
                    selectivity,
                })
            }
            PlanNode::Project { columns, input } => {
                let physical_input = self.to_physical(input)?;
                let rows = physical_input.estimated_rows();
                Ok(PhysicalNode::Project {
                    columns: columns.clone(),
                    input: Box::new(physical_input),
                    estimated_rows: rows,
                })
            }
            PlanNode::Sort { keys, input } => {
                let physical_input = self.to_physical(input)?;
                let cost = self.cost_model.sort_cost(physical_input.estimated_rows());
                Ok(PhysicalNode::Sort {
                    keys: keys.clone(),
                    input: Box::new(physical_input),
                    estimated_rows: cost.estimated_rows,
                })
            }
            PlanNode::Limit {
                count,
                offset,
                input,
            } => {
                let physical_input = self.to_physical(input)?;
                let cost = self
                    .cost_model
                    .limit_cost(physical_input.estimated_rows(), *count + *offset);
                Ok(PhysicalNode::Limit {
                    count: *count,
                    offset: *offset,
                    input: Box::new(physical_input),
                    estimated_rows: cost.estimated_rows,
                })
            }
            PlanNode::HashJoin {
                left,
                right,
                left_key,
                right_key,
            } => {
                let physical_left = self.to_physical(left)?;
                let physical_right = self.to_physical(right)?;
                let cost = self.cost_model.hash_join_cost(
                    physical_left.estimated_rows(),
                    physical_right.estimated_rows(),
                );
                Ok(PhysicalNode::HashJoin {
                    left: Box::new(physical_left),
                    right: Box::new(physical_right),
                    left_key: left_key.clone(),
                    right_key: right_key.clone(),
                    estimated_rows: cost.estimated_rows,
                })
            }
            PlanNode::Merge { sources } => {
                let physical_sources: Vec<PhysicalNode> = sources
                    .iter()
                    .map(|s| self.to_physical(s))
                    .collect::<Result<_, _>>()?;
                let total: f64 = physical_sources.iter().map(|s| s.estimated_rows()).sum();
                Ok(PhysicalNode::Merge {
                    sources: physical_sources,
                    estimated_rows: total,
                })
            }
            PlanNode::Empty { reason } => Ok(PhysicalNode::Empty {
                reason: reason.clone(),
            }),
        }
    }

    fn estimate_input_rows(&self, node: &PlanNode) -> f64 {
        match node {
            PlanNode::Scan { source, .. } => self
                .statistics
                .get(source)
                .map(|s| s.row_count)
                .unwrap_or(1000.0),
            PlanNode::Filter { input, predicate } => {
                let sel = self.estimate_selectivity(predicate);
                self.estimate_input_rows(input) * sel
            }
            PlanNode::Project { input, .. } => self.estimate_input_rows(input),
            PlanNode::Sort { input, .. } => self.estimate_input_rows(input),
            PlanNode::Limit { input, count, .. } => {
                self.estimate_input_rows(input).min(*count as f64)
            }
            PlanNode::HashJoin { left, right, .. } => {
                let left_rows = self.estimate_input_rows(left);
                let right_rows = self.estimate_input_rows(right);
                left_rows.min(right_rows)
            }
            PlanNode::Merge { sources } => {
                sources.iter().map(|s| self.estimate_input_rows(s)).sum()
            }
            PlanNode::Empty { .. } => 0.0,
        }
    }

    fn estimate_selectivity(&self, predicate: &Predicate) -> f64 {
        match predicate {
            Predicate::AlwaysTrue => 1.0,
            Predicate::AlwaysFalse => 0.0,
            Predicate::Eq { column, .. } => {
                let source_table = self.find_source_table(predicate);
                if let Some(table) = source_table {
                    let ndv = self.statistics.get(&table).map(|s| s.ndv(column));
                    match ndv {
                        Some(n) if n > 0.0 => self.cost_model.equality_selectivity(n),
                        _ => self.cost_model.default_selectivity(),
                    }
                } else {
                    self.cost_model.default_selectivity()
                }
            }
            Predicate::NotEq { .. } => 1.0 - self.cost_model.default_selectivity(),
            Predicate::Lt { .. }
            | Predicate::Gt { .. }
            | Predicate::LtEq { .. }
            | Predicate::GtEq { .. } => self.cost_model.range_selectivity(100.0),
            Predicate::Between { .. } => self.cost_model.range_selectivity(100.0) * 2.0,
            Predicate::In { values, .. } => {
                let eq_sel = self.cost_model.default_selectivity();
                (values.len() as f64 * eq_sel).min(1.0)
            }
            Predicate::Like { .. } => 0.1,
            Predicate::IsNull { column } => {
                let source_table = self.find_source_table(predicate);
                if let Some(table) = source_table {
                    self.statistics
                        .get(&table)
                        .map(|s| s.null_fraction(column))
                        .unwrap_or(0.1)
                } else {
                    0.1
                }
            }
            Predicate::IsNotNull { column } => {
                let source_table = self.find_source_table(predicate);
                if let Some(table) = source_table {
                    1.0 - self
                        .statistics
                        .get(&table)
                        .map(|s| s.null_fraction(column))
                        .unwrap_or(0.1)
                } else {
                    0.9
                }
            }
            Predicate::And(preds) => {
                let sels: Vec<f64> = preds.iter().map(|p| self.estimate_selectivity(p)).collect();
                self.cost_model.compound_selectivity(&sels)
            }
            Predicate::Or(preds) => {
                let sels: Vec<f64> = preds.iter().map(|p| self.estimate_selectivity(p)).collect();
                let product: f64 = sels.iter().map(|s| 1.0 - s).product();
                1.0 - product
            }
            Predicate::Not(inner) => 1.0 - self.estimate_selectivity(inner),
        }
    }

    fn find_source_table(&self, _predicate: &Predicate) -> Option<String> {
        None
    }

    fn compute_node_cost(&self, node: &PhysicalNode) -> OptimizationResult<Cost> {
        match node {
            PhysicalNode::Scan { source, .. } => {
                let stats = self.statistics.get(source.as_str());
                let cost = if let Some(s) = stats {
                    self.cost_model.index_scan_cost(s.row_count, s.row_count)
                } else {
                    self.cost_model.full_scan_cost(node.estimated_rows())
                };
                Ok(cost)
            }
            PhysicalNode::Filter {
                input, selectivity, ..
            } => {
                let input_cost = self.compute_node_cost(input)?;
                let filter_cost = self
                    .cost_model
                    .filter_cost(input.estimated_rows(), *selectivity);
                Ok(Cost::new(
                    filter_cost.estimated_rows,
                    input_cost.io_cost + filter_cost.io_cost,
                    input_cost.cpu_cost + filter_cost.cpu_cost,
                    input_cost.memory_bytes + filter_cost.memory_bytes,
                ))
            }
            PhysicalNode::Sort { input, .. } => {
                let input_cost = self.compute_node_cost(input)?;
                let sort_cost = self.cost_model.sort_cost(input.estimated_rows());
                Ok(Cost::new(
                    sort_cost.estimated_rows,
                    input_cost.io_cost + sort_cost.io_cost,
                    input_cost.cpu_cost + sort_cost.cpu_cost,
                    input_cost.memory_bytes + sort_cost.memory_bytes,
                ))
            }
            PhysicalNode::Limit { input, .. } => {
                let input_cost = self.compute_node_cost(input)?;
                Ok(input_cost)
            }
            PhysicalNode::Project { input, .. } => self.compute_node_cost(input),
            PhysicalNode::HashJoin { left, right, .. } => {
                let left_cost = self.compute_node_cost(left)?;
                let right_cost = self.compute_node_cost(right)?;
                let join_cost = self
                    .cost_model
                    .hash_join_cost(left.estimated_rows(), right.estimated_rows());
                Ok(Cost::new(
                    join_cost.estimated_rows,
                    left_cost.io_cost + right_cost.io_cost + join_cost.io_cost,
                    left_cost.cpu_cost + right_cost.cpu_cost + join_cost.cpu_cost,
                    left_cost.memory_bytes + right_cost.memory_bytes + join_cost.memory_bytes,
                ))
            }
            PhysicalNode::Merge { sources, .. } => {
                let mut total = Cost::zero();
                for source in sources {
                    let source_cost = self.compute_node_cost(source)?;
                    total = Cost::new(
                        total.estimated_rows + source_cost.estimated_rows,
                        total.io_cost + source_cost.io_cost,
                        total.cpu_cost + source_cost.cpu_cost,
                        total.memory_bytes + source_cost.memory_bytes,
                    );
                }
                Ok(total)
            }
            PhysicalNode::Empty { .. } => Ok(Cost::zero()),
        }
    }
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new(CostModel::new())
    }
}
