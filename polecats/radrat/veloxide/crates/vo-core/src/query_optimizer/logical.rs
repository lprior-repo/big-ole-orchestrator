use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Predicate {
    Eq {
        column: String,
        value: String,
    },
    NotEq {
        column: String,
        value: String,
    },
    Lt {
        column: String,
        value: String,
    },
    LtEq {
        column: String,
        value: String,
    },
    Gt {
        column: String,
        value: String,
    },
    GtEq {
        column: String,
        value: String,
    },
    Between {
        column: String,
        low: String,
        high: String,
    },
    In {
        column: String,
        values: Vec<String>,
    },
    Like {
        column: String,
        pattern: String,
    },
    IsNull {
        column: String,
    },
    IsNotNull {
        column: String,
    },
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    AlwaysTrue,
    AlwaysFalse,
}

impl Predicate {
    #[must_use]
    pub fn eq(column: &str, value: &str) -> Self {
        Self::Eq {
            column: column.to_string(),
            value: value.to_string(),
        }
    }

    #[must_use]
    pub fn not_eq(column: &str, value: &str) -> Self {
        Self::NotEq {
            column: column.to_string(),
            value: value.to_string(),
        }
    }

    #[must_use]
    pub fn gt(column: &str, value: &str) -> Self {
        Self::Gt {
            column: column.to_string(),
            value: value.to_string(),
        }
    }

    #[must_use]
    pub fn lt(column: &str, value: &str) -> Self {
        Self::Lt {
            column: column.to_string(),
            value: value.to_string(),
        }
    }

    #[must_use]
    pub fn between(column: &str, low: &str, high: &str) -> Self {
        Self::Between {
            column: column.to_string(),
            low: low.to_string(),
            high: high.to_string(),
        }
    }

    #[must_use]
    pub fn r#in(column: &str, values: Vec<&str>) -> Self {
        Self::In {
            column: column.to_string(),
            values: values.into_iter().map(String::from).collect(),
        }
    }

    #[must_use]
    pub fn always_true() -> Self {
        Self::AlwaysTrue
    }

    #[must_use]
    pub fn always_false() -> Self {
        Self::AlwaysFalse
    }

    #[must_use]
    pub fn and(predicates: Vec<Predicate>) -> Self {
        Self::And(predicates)
    }

    #[must_use]
    pub fn or(predicates: Vec<Predicate>) -> Self {
        Self::Or(predicates)
    }

    pub fn referenced_columns(&self) -> Vec<String> {
        match self {
            Self::Eq { column, .. }
            | Self::NotEq { column, .. }
            | Self::Lt { column, .. }
            | Self::LtEq { column, .. }
            | Self::Gt { column, .. }
            | Self::GtEq { column, .. }
            | Self::Between { column, .. }
            | Self::In { column, .. }
            | Self::Like { column, .. }
            | Self::IsNull { column }
            | Self::IsNotNull { column } => vec![column.clone()],
            Self::And(preds) | Self::Or(preds) => {
                preds.iter().flat_map(|p| p.referenced_columns()).collect()
            }
            Self::Not(inner) => inner.referenced_columns(),
            Self::AlwaysTrue | Self::AlwaysFalse => vec![],
        }
    }

    #[must_use]
    pub fn is_always_false(&self) -> bool {
        matches!(self, Self::AlwaysFalse)
    }

    #[must_use]
    pub fn is_always_true(&self) -> bool {
        matches!(self, Self::AlwaysTrue)
    }

    #[must_use]
    pub fn is_equality(&self) -> bool {
        matches!(self, Self::Eq { .. })
    }

    #[must_use]
    pub fn extract_column(&self) -> Option<&str> {
        match self {
            Self::Eq { column, .. }
            | Self::NotEq { column, .. }
            | Self::Lt { column, .. }
            | Self::LtEq { column, .. }
            | Self::Gt { column, .. }
            | Self::GtEq { column, .. }
            | Self::Between { column, .. }
            | Self::In { column, .. }
            | Self::Like { column, .. }
            | Self::IsNull { column }
            | Self::IsNotNull { column } => Some(column),
            _ => None,
        }
    }

    #[must_use]
    pub fn conjunctions(&self) -> Vec<&Predicate> {
        match self {
            Self::And(preds) => preds.iter().collect(),
            other => vec![other],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl Default for SortDirection {
    #[inline]
    fn default() -> Self {
        Self::Ascending
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub column: String,
    pub direction: SortDirection,
}

impl SortKey {
    #[must_use]
    pub fn asc(column: &str) -> Self {
        Self {
            column: column.to_string(),
            direction: SortDirection::Ascending,
        }
    }

    #[must_use]
    pub fn desc(column: &str) -> Self {
        Self {
            column: column.to_string(),
            direction: SortDirection::Descending,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanNode {
    Scan {
        source: String,
        alias: Option<String>,
        columns: Vec<String>,
    },
    Filter {
        predicate: Predicate,
        input: Box<PlanNode>,
    },
    Project {
        columns: Vec<String>,
        input: Box<PlanNode>,
    },
    Sort {
        keys: Vec<SortKey>,
        input: Box<PlanNode>,
    },
    Limit {
        count: u64,
        offset: u64,
        input: Box<PlanNode>,
    },
    HashJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        left_key: String,
        right_key: String,
    },
    Merge {
        sources: Vec<PlanNode>,
    },
    Empty {
        reason: String,
    },
}

impl PlanNode {
    #[must_use]
    pub fn scan(source: &str) -> Self {
        Self::Scan {
            source: source.to_string(),
            alias: None,
            columns: vec![],
        }
    }

    #[must_use]
    pub fn scan_with_alias(source: &str, alias: &str) -> Self {
        Self::Scan {
            source: source.to_string(),
            alias: Some(alias.to_string()),
            columns: vec![],
        }
    }

    #[must_use]
    pub fn filter(predicate: Predicate, input: PlanNode) -> Self {
        if predicate.is_always_false() {
            return Self::Empty {
                reason: "false predicate".to_string(),
            };
        }
        if predicate.is_always_true() {
            return input;
        }
        Self::Filter {
            predicate,
            input: Box::new(input),
        }
    }

    #[must_use]
    pub fn project(columns: Vec<&str>, input: PlanNode) -> Self {
        Self::Project {
            columns: columns.into_iter().map(String::from).collect(),
            input: Box::new(input),
        }
    }

    #[must_use]
    pub fn sort(keys: Vec<SortKey>, input: PlanNode) -> Self {
        if keys.is_empty() {
            return input;
        }
        Self::Sort {
            keys,
            input: Box::new(input),
        }
    }

    #[must_use]
    pub fn limit(count: u64, input: PlanNode) -> Self {
        Self::Limit {
            count,
            offset: 0,
            input: Box::new(input),
        }
    }

    #[must_use]
    pub fn limit_with_offset(count: u64, offset: u64, input: PlanNode) -> Self {
        Self::Limit {
            count,
            offset,
            input: Box::new(input),
        }
    }

    #[must_use]
    pub fn hash_join(left: PlanNode, right: PlanNode, left_key: &str, right_key: &str) -> Self {
        Self::HashJoin {
            left: Box::new(left),
            right: Box::new(right),
            left_key: left_key.to_string(),
            right_key: right_key.to_string(),
        }
    }

    #[must_use]
    pub fn merge(sources: Vec<PlanNode>) -> Self {
        Self::Merge { sources }
    }

    #[must_use]
    pub fn empty(reason: &str) -> Self {
        Self::Empty {
            reason: reason.to_string(),
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
            Self::Merge { sources } => sources.iter().flat_map(|s| s.sources()).collect(),
            Self::Empty { .. } => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicalPlan {
    pub root: PlanNode,
}

impl LogicalPlan {
    #[must_use]
    pub fn new(root: PlanNode) -> Self {
        Self { root }
    }

    pub fn sources(&self) -> Vec<String> {
        self.root.sources().into_iter().map(String::from).collect()
    }
}

impl fmt::Display for LogicalPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_node(f, &self.root, 0)
    }
}

fn fmt_node(f: &mut fmt::Formatter<'_>, node: &PlanNode, indent: usize) -> fmt::Result {
    let pad = "  ".repeat(indent);
    match node {
        PlanNode::Scan {
            source,
            alias,
            columns,
        } => {
            write!(f, "{pad}Scan({source}")?;
            if let Some(a) = alias {
                write!(f, " as {a}")?;
            }
            if !columns.is_empty() {
                write!(f, " [{}]", columns.join(", "))?;
            }
            writeln!(f, ")")
        }
        PlanNode::Filter { predicate, input } => {
            writeln!(f, "{pad}Filter({predicate:?})")?;
            fmt_node(f, input, indent + 1)
        }
        PlanNode::Project { columns, input } => {
            writeln!(f, "{pad}Project([{}])", columns.join(", "))?;
            fmt_node(f, input, indent + 1)
        }
        PlanNode::Sort { keys, input } => {
            let key_strs: Vec<String> = keys
                .iter()
                .map(|k| match k.direction {
                    SortDirection::Ascending => format!("{} ASC", k.column),
                    SortDirection::Descending => format!("{} DESC", k.column),
                })
                .collect();
            writeln!(f, "{pad}Sort([{}])", key_strs.join(", "))?;
            fmt_node(f, input, indent + 1)
        }
        PlanNode::Limit {
            count,
            offset,
            input,
        } => {
            writeln!(f, "{pad}Limit(count={count}, offset={offset})")?;
            fmt_node(f, input, indent + 1)
        }
        PlanNode::HashJoin {
            left,
            right,
            left_key,
            right_key,
        } => {
            writeln!(
                f,
                "{pad}HashJoin(left_key={left_key}, right_key={right_key})"
            )?;
            writeln!(f, "{pad}  Left:")?;
            fmt_node(f, left, indent + 2)?;
            writeln!(f, "{pad}  Right:")?;
            fmt_node(f, right, indent + 2)
        }
        PlanNode::Merge { sources } => {
            writeln!(f, "{pad}Merge({} sources)", sources.len())?;
            for source in sources {
                fmt_node(f, source, indent + 1)?;
            }
            Ok(())
        }
        PlanNode::Empty { reason } => writeln!(f, "{pad}Empty({reason})"),
    }
}
