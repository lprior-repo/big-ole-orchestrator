use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ColumnStats {
    pub ndv: f64,
    pub null_count: u64,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub avg_width: f64,
    pub total_size_bytes: u64,
}

impl ColumnStats {
    #[must_use]
    pub fn new(ndv: f64, null_count: u64) -> Self {
        Self {
            ndv,
            null_count,
            min_value: None,
            max_value: None,
            avg_width: 32.0,
            total_size_bytes: 0,
        }
    }

    #[must_use]
    pub fn with_range(ndv: f64, null_count: u64, min: f64, max: f64) -> Self {
        Self {
            ndv,
            null_count,
            min_value: Some(min),
            max_value: Some(max),
            avg_width: 32.0,
            total_size_bytes: 0,
        }
    }
}

impl Default for ColumnStats {
    fn default() -> Self {
        Self::new(100.0, 0)
    }
}

#[derive(Debug, Clone)]
pub struct TableStatistics {
    pub row_count: f64,
    pub columns: HashMap<String, ColumnStats>,
    pub table_size_bytes: u64,
}

impl TableStatistics {
    #[must_use]
    pub fn new(row_count: f64) -> Self {
        Self {
            row_count,
            columns: HashMap::new(),
            table_size_bytes: 0,
        }
    }

    #[must_use]
    pub fn with_size(row_count: f64, table_size_bytes: u64) -> Self {
        Self {
            row_count,
            columns: HashMap::new(),
            table_size_bytes,
        }
    }

    pub fn add_column(&mut self, name: &str, stats: ColumnStats) {
        self.columns.insert(name.to_string(), stats);
    }

    #[must_use]
    pub fn column_stats(&self, column: &str) -> Option<&ColumnStats> {
        self.columns.get(column)
    }

    #[must_use]
    pub fn ndv(&self, column: &str) -> f64 {
        self.columns
            .get(column)
            .map(|c| c.ndv)
            .unwrap_or_else(|| self.row_count * 0.1)
    }

    #[must_use]
    pub fn null_fraction(&self, column: &str) -> f64 {
        if self.row_count <= 0.0 {
            return 0.0;
        }
        self.columns
            .get(column)
            .map(|c| c.null_count as f64 / self.row_count)
            .unwrap_or(0.0)
    }

    #[must_use]
    pub fn row_size_estimate(&self) -> f64 {
        if self.columns.is_empty() || self.row_count <= 0.0 {
            return 64.0;
        }
        let total_width: f64 = self.columns.values().map(|c| c.avg_width).sum();
        total_width / self.columns.len() as f64
    }
}

impl Default for TableStatistics {
    fn default() -> Self {
        Self::new(1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_stats_default() {
        let stats = ColumnStats::default();
        assert_eq!(stats.ndv, 100.0);
        assert_eq!(stats.null_count, 0);
    }

    #[test]
    fn column_stats_with_range() {
        let stats = ColumnStats::with_range(50.0, 5, 1.0, 100.0);
        assert_eq!(stats.ndv, 50.0);
        assert_eq!(stats.null_count, 5);
        assert_eq!(stats.min_value, Some(1.0));
        assert_eq!(stats.max_value, Some(100.0));
    }

    #[test]
    fn table_statistics_basic() {
        let mut stats = TableStatistics::new(10_000.0);
        stats.add_column("id", ColumnStats::new(10_000.0, 0));
        stats.add_column("status", ColumnStats::new(5.0, 0));

        assert_eq!(stats.row_count, 10_000.0);
        assert_eq!(stats.ndv("id"), 10_000.0);
        assert_eq!(stats.ndv("status"), 5.0);
        assert_eq!(stats.ndv("unknown"), 1_000.0);
        assert_eq!(stats.null_fraction("id"), 0.0);
    }

    #[test]
    fn table_statistics_null_fraction() {
        let mut stats = TableStatistics::new(1000.0);
        stats.add_column("nullable_col", ColumnStats::new(800.0, 200));

        assert!((stats.null_fraction("nullable_col") - 0.2).abs() < 0.001);
    }

    #[test]
    fn table_statistics_row_size() {
        let stats = TableStatistics::default();
        assert_eq!(stats.row_size_estimate(), 64.0);
    }

    #[test]
    fn table_statistics_with_columns_row_size() {
        let mut stats = TableStatistics::new(100.0);
        stats.add_column("a", ColumnStats::new(10.0, 0));
        stats.add_column("b", ColumnStats::new(20.0, 0));

        let avg = stats.row_size_estimate();
        assert!(avg > 0.0);
    }
}
