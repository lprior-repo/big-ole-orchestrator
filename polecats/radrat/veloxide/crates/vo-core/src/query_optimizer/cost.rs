use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub estimated_rows: f64,
    pub io_cost: f64,
    pub cpu_cost: f64,
    pub memory_bytes: f64,
}

impl Cost {
    #[must_use]
    pub fn new(estimated_rows: f64, io_cost: f64, cpu_cost: f64, memory_bytes: f64) -> Self {
        Self {
            estimated_rows,
            io_cost,
            cpu_cost,
            memory_bytes,
        }
    }

    #[must_use]
    pub fn zero() -> Self {
        Self {
            estimated_rows: 0.0,
            io_cost: 0.0,
            cpu_cost: 0.0,
            memory_bytes: 0.0,
        }
    }

    #[must_use]
    pub fn total(&self) -> f64 {
        self.io_cost + self.cpu_cost + (self.memory_bytes / 1_000_000.0)
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.estimated_rows.is_finite()
            && self.io_cost.is_finite()
            && self.cpu_cost.is_finite()
            && self.memory_bytes.is_finite()
    }
}

impl fmt::Display for Cost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rows={:.1} io={:.2} cpu={:.2} mem={:.0} total={:.2}",
            self.estimated_rows,
            self.io_cost,
            self.cpu_cost,
            self.memory_bytes,
            self.total()
        )
    }
}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.total().partial_cmp(&other.total())
    }
}

#[derive(Debug, Clone)]
pub struct CostModel {
    io_weight: f64,
    cpu_weight: f64,
    #[allow(dead_code)]
    memory_weight: f64,
    scan_row_cost: f64,
    seek_cost: f64,
    sort_cost_per_row: f64,
    filter_selectivity_base: f64,
}

impl CostModel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            io_weight: 1.0,
            cpu_weight: 0.5,
            memory_weight: 0.001,
            scan_row_cost: 0.001,
            seek_cost: 0.5,
            sort_cost_per_row: 0.002,
            filter_selectivity_base: 0.3,
        }
    }

    #[must_use]
    pub fn with_weights(io_weight: f64, cpu_weight: f64, memory_weight: f64) -> Self {
        Self {
            io_weight,
            cpu_weight,
            memory_weight,
            ..Self::new()
        }
    }

    #[must_use]
    pub fn full_scan_cost(&self, total_rows: f64) -> Cost {
        Cost::new(
            total_rows,
            total_rows * self.scan_row_cost * self.io_weight,
            total_rows * self.scan_row_cost * self.cpu_weight,
            0.0,
        )
    }

    #[must_use]
    pub fn index_scan_cost(&self, estimated_rows: f64, total_rows: f64) -> Cost {
        let selectivity = if total_rows > 0.0 {
            (estimated_rows / total_rows).min(1.0)
        } else {
            0.0
        };
        let rows_fetched = estimated_rows;
        let io = self.seek_cost * self.io_weight
            + rows_fetched * self.scan_row_cost * selectivity * self.io_weight;
        let cpu = rows_fetched * self.scan_row_cost * self.cpu_weight;
        Cost::new(rows_fetched, io, cpu, 0.0)
    }

    #[must_use]
    pub fn filter_cost(&self, input_rows: f64, selectivity: f64) -> Cost {
        let output_rows = input_rows * selectivity;
        Cost::new(
            output_rows,
            0.0,
            input_rows * self.scan_row_cost * self.cpu_weight,
            0.0,
        )
    }

    #[must_use]
    pub fn sort_cost(&self, input_rows: f64) -> Cost {
        let log_n = if input_rows > 1.0 {
            input_rows.log2()
        } else {
            0.0
        };
        let cpu = input_rows * log_n * self.sort_cost_per_row * self.cpu_weight;
        Cost::new(
            input_rows,
            input_rows * self.scan_row_cost * self.io_weight,
            cpu,
            input_rows * 64.0,
        )
    }

    #[must_use]
    pub fn limit_cost(&self, input_rows: f64, limit: u64) -> Cost {
        let output = input_rows.min(limit as f64);
        Cost::new(
            output,
            0.0,
            output * self.scan_row_cost * self.cpu_weight * 0.5,
            0.0,
        )
    }

    #[must_use]
    pub fn hash_join_cost(&self, build_rows: f64, probe_rows: f64) -> Cost {
        let output = build_rows.min(probe_rows);
        let build_cpu = build_rows * self.scan_row_cost * self.cpu_weight;
        let probe_cpu = probe_rows * self.scan_row_cost * self.cpu_weight;
        Cost::new(output, 0.0, build_cpu + probe_cpu, build_rows * 64.0)
    }

    #[must_use]
    pub fn merge_cost(&self, left_rows: f64, right_rows: f64) -> Cost {
        let output = left_rows + right_rows;
        Cost::new(
            output,
            (left_rows + right_rows) * self.scan_row_cost * self.io_weight,
            output * self.scan_row_cost * self.cpu_weight,
            0.0,
        )
    }

    #[must_use]
    pub fn default_selectivity(&self) -> f64 {
        self.filter_selectivity_base
    }

    #[must_use]
    pub fn equality_selectivity(&self, ndv: f64) -> f64 {
        if ndv > 0.0 {
            1.0 / ndv
        } else {
            self.filter_selectivity_base
        }
    }

    #[must_use]
    pub fn range_selectivity(&self, _ndv: f64) -> f64 {
        0.25
    }

    #[must_use]
    pub fn compound_selectivity(&self, selectivities: &[f64]) -> f64 {
        selectivities.iter().copied().product::<f64>().max(0.001)
    }
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new()
    }
}
