use crate::query_optimizer::cost::{Cost, CostModel};
use crate::query_optimizer::logical::{LogicalPlan, PlanNode, Predicate, SortDirection, SortKey};
use crate::query_optimizer::optimizer::Optimizer;
use crate::query_optimizer::physical::{AccessStrategy, PhysicalNode};
use crate::query_optimizer::planner::{QueryDescriptor, QueryPlanner};
use crate::query_optimizer::statistics::{ColumnStats, TableStatistics};
use crate::query_optimizer::PhysicalPlan;

fn instances_stats() -> TableStatistics {
    let mut stats = TableStatistics::new(10_000.0);
    stats.add_column("id", ColumnStats::new(10_000.0, 0));
    stats.add_column("status", ColumnStats::new(5.0, 0));
    stats.add_column("workflow_id", ColumnStats::new(50.0, 0));
    stats.add_column(
        "created_at",
        ColumnStats::with_range(10_000.0, 0, 1_000_000.0, 2_000_000.0),
    );
    stats
}

#[test]
fn qo_001_simple_scan() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances");

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(matches!(
        plan.root,
        PhysicalNode::Scan {
            source: _,
            strategy: AccessStrategy::IndexScan { .. },
            ..
        }
    ));
    assert!((plan.root.estimated_rows() - 10_000.0).abs() < 1.0);
}

#[test]
fn qo_002_scan_with_filter() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances").with_predicate(Predicate::eq("status", "running"));

    let plan = planner.plan(&desc).expect("plan should succeed");
    let expected_rows = 10_000.0 * 0.3;
    assert!((plan.root.estimated_rows() - expected_rows).abs() < 1.0);
}

#[test]
fn qo_003_scan_filter_limit() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances")
        .with_predicate(Predicate::eq("status", "running"))
        .with_limit(10);

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(plan.root.estimated_rows() <= 10.0);
}

#[test]
fn qo_004_scan_sort_limit() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances")
        .with_sort(vec![SortKey::desc("created_at")])
        .with_limit(50);

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(plan.root.estimated_rows() <= 50.0);
}

#[test]
fn qo_005_filter_always_false_produces_empty() {
    let planner = QueryPlanner::default();
    let desc = QueryDescriptor::new("instances").with_predicate(Predicate::always_false());

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(matches!(plan.root, PhysicalNode::Empty { .. }));
}

#[test]
fn qo_006_filter_always_true_passthrough() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances").with_predicate(Predicate::always_true());

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(!matches!(plan.root, PhysicalNode::Empty { .. }));
    assert!(!matches!(plan.root, PhysicalNode::Filter { .. }));
}

#[test]
fn qo_007_compound_predicate_selectivity() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances").with_predicate(Predicate::and(vec![
        Predicate::eq("status", "running"),
        Predicate::eq("workflow_id", "wf-42"),
    ]));

    let plan = planner.plan(&desc).expect("plan should succeed");
    let expected = 10_000.0 * 0.3 * 0.3;
    assert!((plan.root.estimated_rows() - expected).abs() < 10.0);
}

#[test]
fn qo_008_or_predicate_selectivity() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances").with_predicate(Predicate::or(vec![
        Predicate::eq("status", "running"),
        Predicate::eq("status", "completed"),
    ]));

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(plan.root.estimated_rows() > 10_000.0 / 5.0);
    assert!(plan.root.estimated_rows() <= 10_000.0);
}

#[test]
fn qo_009_join_plan() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());
    stats_map.insert("events".to_string(), TableStatistics::new(100_000.0));

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances")
        .with_join("events", "id", "instance_id")
        .with_limit(20);

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(matches!(plan.root, PhysicalNode::Limit { .. }));
}

#[test]
fn qo_010_physical_plan_depth_and_node_count() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances")
        .with_predicate(Predicate::eq("status", "running"))
        .with_sort(vec![SortKey::asc("created_at")])
        .with_limit(10);

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(plan.root.max_depth() >= 3);
    assert!(plan.root.node_count() >= 3);
}

#[test]
fn qo_011_cost_model_full_scan() {
    let model = CostModel::new();
    let cost = model.full_scan_cost(10_000.0);
    assert!((cost.estimated_rows - 10_000.0).abs() < 0.1);
    assert!(cost.io_cost > 0.0);
    assert!(cost.cpu_cost > 0.0);
}

#[test]
fn qo_012_cost_model_index_scan() {
    let model = CostModel::new();
    let cost = model.index_scan_cost(100.0, 10_000.0);
    assert!((cost.estimated_rows - 100.0).abs() < 0.1);
    assert!(cost.io_cost < model.full_scan_cost(10_000.0).io_cost);
}

#[test]
fn qo_013_cost_model_sort() {
    let model = CostModel::new();
    let cost = model.sort_cost(1_000.0);
    assert!((cost.estimated_rows - 1_000.0).abs() < 0.1);
    assert!(cost.cpu_cost > 0.0);
}

#[test]
fn qo_014_cost_model_limit() {
    let model = CostModel::new();
    let cost = model.limit_cost(10_000.0, 100);
    assert!((cost.estimated_rows - 100.0).abs() < 0.1);
}

#[test]
fn qo_015_cost_model_hash_join() {
    let model = CostModel::new();
    let cost = model.hash_join_cost(1_000.0, 5_000.0);
    assert!((cost.estimated_rows - 1_000.0).abs() < 0.1);
}

#[test]
fn qo_016_cost_total_is_monotonic() {
    let model = CostModel::new();
    let cheap = model.index_scan_cost(100.0, 10_000.0);
    let expensive = model.full_scan_cost(10_000.0);
    assert!(cheap.total() < expensive.total());
}

#[test]
fn qo_017_cost_display() {
    let cost = Cost::new(100.0, 1.5, 0.75, 6400.0);
    let display = format!("{cost}");
    assert!(display.contains("rows=100.0"));
    assert!(display.contains("io=1.50"));
    assert!(display.contains("cpu=0.75"));
}

#[test]
fn qo_018_predicate_referenced_columns() {
    let pred = Predicate::and(vec![Predicate::eq("a", "1"), Predicate::gt("b", "2")]);
    let mut cols: Vec<String> = pred.referenced_columns();
    cols.sort();
    assert_eq!(cols, vec!["a", "b"]);
}

#[test]
fn qo_019_predicate_extract_column() {
    assert_eq!(
        Predicate::eq("status", "x").extract_column(),
        Some("status")
    );
    assert_eq!(Predicate::and(vec![]).extract_column(), None);
}

#[test]
fn qo_020_predicate_conjunctions() {
    let pred = Predicate::and(vec![Predicate::eq("a", "1"), Predicate::eq("b", "2")]);
    let conjs = pred.conjunctions();
    assert_eq!(conjs.len(), 2);

    let single = Predicate::eq("a", "1");
    let conjs2 = single.conjunctions();
    assert_eq!(conjs2.len(), 1);
}

#[test]
fn qo_021_plan_node_sources() {
    let scan = PlanNode::scan("instances");
    assert_eq!(scan.sources(), vec!["instances"]);

    let filtered = PlanNode::filter(Predicate::eq("status", "x"), scan);
    assert_eq!(filtered.sources(), vec!["instances"]);

    let joined = PlanNode::hash_join(PlanNode::scan("a"), PlanNode::scan("b"), "id", "a_id");
    let mut sources = joined.sources();
    sources.sort();
    assert_eq!(sources, vec!["a", "b"]);
}

#[test]
fn qo_022_logical_plan_display() {
    let plan = LogicalPlan::new(PlanNode::filter(
        Predicate::eq("status", "running"),
        PlanNode::scan("instances"),
    ));
    let display = format!("{plan}");
    assert!(display.contains("Scan(instances)"));
    assert!(display.contains("Filter"));
}

#[test]
fn qo_023_sort_key_constructors() {
    let asc = SortKey::asc("created_at");
    assert_eq!(asc.column, "created_at");
    assert_eq!(asc.direction, SortDirection::Ascending);

    let desc = SortKey::desc("id");
    assert_eq!(desc.direction, SortDirection::Descending);
}

#[test]
fn qo_024_optimizer_predicate_pushdown() {
    let optimizer = Optimizer::default();
    let plan = LogicalPlan::new(PlanNode::sort(
        vec![SortKey::asc("id")],
        PlanNode::filter(Predicate::eq("status", "x"), PlanNode::scan("instances")),
    ));

    let result = optimizer.optimize(&plan).expect("optimize should succeed");
    assert!(matches!(result.root, PhysicalNode::Sort { .. }));
}

#[test]
fn qo_025_empty_plan_cost() {
    let cost = Cost::zero();
    assert!((cost.total()).abs() < 0.001);
    assert!(cost.is_finite());
}

#[test]
fn qo_026_plan_with_offset() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances")
        .with_limit(10)
        .with_offset(20);

    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!(matches!(plan.root, PhysicalNode::Limit { offset: 20, .. }));
}

#[test]
fn qo_027_query_descriptor_builder() {
    let desc = QueryDescriptor::new("events")
        .with_predicate(Predicate::gt("sequence", "1000"))
        .with_projections(vec!["event_type", "payload"])
        .with_sort(vec![SortKey::asc("sequence")])
        .with_limit(100);

    assert_eq!(desc.source, "events");
    assert!(desc.predicate.is_some());
    assert_eq!(desc.projections.len(), 2);
    assert_eq!(desc.sort_keys.len(), 1);
    assert_eq!(desc.limit, Some(100));
    assert_eq!(desc.offset, 0);
}

#[test]
fn qo_028_statistics_helpers() {
    let stats = instances_stats();
    assert!((stats.ndv("id") - 10_000.0).abs() < 0.1);
    assert!((stats.ndv("status") - 5.0).abs() < 0.1);
    assert!((stats.null_fraction("id")).abs() < 0.001);
    assert!(stats.row_size_estimate() > 0.0);
}

#[test]
fn qo_029_cost_model_selectivity_helpers() {
    let model = CostModel::new();
    assert!((model.equality_selectivity(100.0) - 0.01).abs() < 0.001);
    assert!((model.range_selectivity(100.0) - 0.25).abs() < 0.001);

    let compound = model.compound_selectivity(&[0.1, 0.2, 0.3]);
    assert!((compound - 0.006).abs() < 0.001);
}

#[test]
fn qo_030_optimizer_with_statistics_update() {
    let mut planner = QueryPlanner::default();
    planner.update_statistics("instances", instances_stats());

    let desc = QueryDescriptor::new("instances");
    let plan = planner.plan(&desc).expect("plan should succeed");
    assert!((plan.root.estimated_rows() - 10_000.0).abs() < 1.0);
}

#[test]
fn qo_031_plan_debug_output() {
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert("instances".to_string(), instances_stats());

    let planner = QueryPlanner::with_statistics(CostModel::new(), stats_map);
    let desc = QueryDescriptor::new("instances").with_limit(10);

    let debug = planner.plan_debug(&desc).expect("debug should succeed");
    assert!(debug.contains("PhysicalPlan"));
    assert!(debug.contains("Limit"));
}
