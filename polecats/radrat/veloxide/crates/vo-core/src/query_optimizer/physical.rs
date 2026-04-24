use std::fmt;

use crate::query_optimizer::logical::{Predicate, SortKey};

#[derive(Debug, Clone, PartialEq)]
pub enum AccessStrategy {
    FullScan,
    IndexScan { index_hint: Option<String> },
    SeekByPK { key: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhysicalNode {
    Scan {
        source: String,
        columns: Vec<String>,
        alias: Option<String>,
        strategy: AccessStrategy,
        estimated_rows: f64,
    },
    Filter {
        predicate: Predicate,
        input: Box<PhysicalNode>,
        estimated_rows: f64,
        selectivity: f64,
    },
    Project {
        columns: Vec<String>,
        input: Box<PhysicalNode>,
        estimated_rows: f64,
    },
    Sort {
        keys: Vec<SortKey>,
        input: Box<PhysicalNode>,
        estimated_rows: f64,
    },
    Limit {
        count: u64,
        offset: u64,
        input: Box<PhysicalNode>,
        estimated_rows: f64,
    },
    HashJoin {
        left: Box<PhysicalNode>,
        right: Box<PhysicalNode>,
        left_key: String,
        right_key: String,
        estimated_rows: f64,
    },
    Merge {
        sources: Vec<PhysicalNode>,
        estimated_rows: f64,
    },
    Empty {
        reason: String,
    },
}

impl PhysicalNode {
    #[must_use]
    pub fn estimated_rows(&self) -> f64 {
        match self {
            Self::Scan { estimated_rows, .. }
            | Self::Filter { estimated_rows, .. }
            | Self::Project { estimated_rows, .. }
            | Self::Sort { estimated_rows, .. }
            | Self::Limit { estimated_rows, .. }
            | Self::HashJoin { estimated_rows, .. }
            | Self::Merge { estimated_rows, .. } => *estimated_rows,
            Self::Empty { .. } => 0.0,
        }
    }

    pub fn sources(&self) -> Vec<&str> {
        match self {
            Self::Scan { source, .. } => vec![source],
            Self::Filter { input, .. }
            | Self::Project { input, .. }
            | Self::Sort { input, .. }
            | Self::Limit { input, .. } => input.sources(),
            Self::HashJoin { left, right, .. } => {
                let mut s = left.sources();
                s.extend(right.sources());
                s
            }
            Self::Merge { sources, .. } => sources.iter().flat_map(|s| s.sources()).collect(),
            Self::Empty { .. } => vec![],
        }
    }

    #[must_use]
    pub fn max_depth(&self) -> usize {
        match self {
            Self::Scan { .. } | Self::Empty { .. } => 1,
            Self::Filter { input, .. }
            | Self::Project { input, .. }
            | Self::Sort { input, .. }
            | Self::Limit { input, .. } => 1 + input.max_depth(),
            Self::HashJoin { left, right, .. } => 1 + left.max_depth().max(right.max_depth()),
            Self::Merge { sources, .. } => {
                1 + sources.iter().map(|s| s.max_depth()).max().unwrap_or(0)
            }
        }
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        match self {
            Self::Scan { .. } | Self::Empty { .. } => 1,
            Self::Filter { input, .. }
            | Self::Project { input, .. }
            | Self::Sort { input, .. }
            | Self::Limit { input, .. } => 1 + input.node_count(),
            Self::HashJoin { left, right, .. } => 1 + left.node_count() + right.node_count(),
            Self::Merge { sources, .. } => {
                1 + sources.iter().map(|s| s.node_count()).sum::<usize>()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalPlan {
    pub root: PhysicalNode,
    pub cost: crate::query_optimizer::Cost,
}

fn fmt_physical_node(
    f: &mut fmt::Formatter<'_>,
    node: &PhysicalNode,
    indent: usize,
) -> fmt::Result {
    let pad = "  ".repeat(indent);
    match node {
        PhysicalNode::Scan {
            source,
            strategy,
            estimated_rows,
            ..
        } => {
            writeln!(
                f,
                "{pad}Scan({source}, strategy={strategy:?}, rows={estimated_rows:.1})"
            )
        }
        PhysicalNode::Filter {
            predicate,
            selectivity,
            estimated_rows,
            input,
        } => {
            writeln!(
                f,
                "{pad}Filter(sel={selectivity:.3}, rows={estimated_rows:.1}, pred={predicate:?})"
            )?;
            fmt_physical_node(f, input, indent + 1)
        }
        PhysicalNode::Project {
            columns,
            estimated_rows,
            input,
        } => {
            writeln!(
                f,
                "{pad}Project([{}], rows={estimated_rows:.1})",
                columns.join(", ")
            )?;
            fmt_physical_node(f, input, indent + 1)
        }
        PhysicalNode::Sort {
            estimated_rows,
            input,
            ..
        } => {
            writeln!(f, "{pad}Sort(rows={estimated_rows:.1})")?;
            fmt_physical_node(f, input, indent + 1)
        }
        PhysicalNode::Limit {
            count,
            offset,
            estimated_rows,
            input,
        } => {
            writeln!(
                f,
                "{pad}Limit(count={count}, offset={offset}, rows={estimated_rows:.1})"
            )?;
            fmt_physical_node(f, input, indent + 1)
        }
        PhysicalNode::HashJoin {
            left,
            right,
            left_key,
            right_key,
            estimated_rows,
        } => {
            writeln!(
                f,
                "{pad}HashJoin({left_key}={right_key}, rows={estimated_rows:.1})"
            )?;
            fmt_physical_node(f, left, indent + 1)?;
            fmt_physical_node(f, right, indent + 1)
        }
        PhysicalNode::Merge {
            sources,
            estimated_rows,
        } => {
            writeln!(
                f,
                "{pad}Merge({} sources, rows={estimated_rows:.1})",
                sources.len()
            )?;
            for source in sources {
                fmt_physical_node(f, source, indent + 1)?;
            }
            Ok(())
        }
        PhysicalNode::Empty { reason } => writeln!(f, "{pad}Empty({reason})"),
    }
}

impl fmt::Display for PhysicalPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PhysicalPlan (cost: {})", self.cost)?;
        fmt_physical_node(f, &self.root, 0)
    }
}
