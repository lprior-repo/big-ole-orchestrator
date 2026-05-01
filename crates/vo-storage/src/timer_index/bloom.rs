use vo_types::TimerId;

type BitVec = Vec<bool>;

const DEFAULT_EXPECTED_ITEMS: usize = 1_000_000;
const DEFAULT_FP_RATE: f64 = 0.01;
const REBUILD_THRESHOLD: usize = 10_000;

#[derive(Debug)]
pub struct BloomStats {
    pub insert_count: u64,
    pub check_count: u64,
    pub false_positive_estimated: u64,
}

impl BloomStats {
    pub fn new() -> Self {
        Self {
            insert_count: 0,
            check_count: 0,
            false_positive_estimated: 0,
        }
    }
}

impl Default for BloomStats {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TimerBloomFilter {
    bit_vector: BitVec,
    num_hash_functions: usize,
    expected_items: usize,
    stats: BloomStats,
    inserts_since_rebuild: usize,
}

impl TimerBloomFilter {
    pub fn new(expected_items: usize, fp_rate: f64) -> Self {
        let num_hash_functions = Self::optimal_k(fp_rate);
        let num_bits = Self::optimal_m(expected_items, fp_rate);
        let bit_vector = vec![false; num_bits];

        Self {
            bit_vector,
            num_hash_functions,
            expected_items,
            stats: BloomStats::new(),
            inserts_since_rebuild: 0,
        }
    }

    fn optimal_k(fp_rate: f64) -> usize {
        let k = -(fp_rate.ln() / std::f64::consts::LN_2).round() as usize;
        k.max(1)
    }

    fn optimal_m(expected_items: usize, fp_rate: f64) -> usize {
        let m = -(expected_items as f64 * fp_rate.ln()) / (std::f64::consts::LN_2 * std::f64::consts::LN_2);
        m.round() as usize
    }

    fn compute_h1(timer_id: &TimerId) -> u64 {
        wyhash::wyhash(timer_id.as_str().as_bytes(), 0)
    }

    fn compute_h2(fire_at_ms: u64) -> u64 {
        use xxhash_rust::xxh3::xxh3_64;
        xxh3_64(&fire_at_ms.to_be_bytes())
    }

    fn compute_hash_positions(&self, timer_id: &TimerId, fire_at_ms: u64) -> Vec<usize> {
        let h1 = Self::compute_h1(timer_id);
        let h2 = Self::compute_h2(fire_at_ms);
        let m = self.bit_vector.len();

        (0..self.num_hash_functions)
            .map(|i| {
                let h_i = h1.wrapping_add((i as u64).wrapping_mul(h2));
                (h_i % m as u64) as usize
            })
            .collect()
    }

    pub fn might_be_expired(&mut self, timer_id: &TimerId, fire_at_ms: u64, _now_ms: u64) -> bool {
        self.stats.check_count += 1;

        let positions = self.compute_hash_positions(timer_id, fire_at_ms);

        if positions.iter().all(|&pos| self.bit_vector[pos]) {
            self.stats.false_positive_estimated += 1;
            true
        } else {
            false
        }
    }

    pub fn insert(&mut self, timer_id: &TimerId, fire_at_ms: u64) {
        self.stats.insert_count += 1;
        self.inserts_since_rebuild += 1;

        let positions = self.compute_hash_positions(timer_id, fire_at_ms);

        for pos in positions {
            self.bit_vector[pos] = true;
        }

        if self.inserts_since_rebuild >= REBUILD_THRESHOLD {
            self.reset_for_rebuild();
        }
    }

    fn reset_for_rebuild(&mut self) {
        for v in &mut self.bit_vector {
            *v = false;
        }
        self.inserts_since_rebuild = 0;
    }

    pub fn rebuild(&mut self) {
        self.reset_for_rebuild();
    }

    pub fn stats(&self) -> &BloomStats {
        &self.stats
    }

    pub fn clear_stats(&mut self) {
        self.stats = BloomStats::new();
        self.inserts_since_rebuild = 0;
    }

    pub fn fp_rate_estimate(&self) -> f64 {
        if self.stats.check_count == 0 {
            return 0.0;
        }
        self.stats.false_positive_estimated as f64 / self.stats.check_count as f64
    }
}

impl Default for TimerBloomFilter {
    fn default() -> Self {
        Self::new(DEFAULT_EXPECTED_ITEMS, DEFAULT_FP_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_timer_id() -> TimerId {
        TimerId::from_bytes([2; 16])
    }

    #[test]
    fn test_bloom_insert_and_check() {
        let timer_id = create_timer_id();
        let mut bloom = TimerBloomFilter::new(1000, 0.01);

        bloom.insert(&timer_id, 1000);

        assert!(bloom.might_be_expired(&timer_id, 1000, 500));
    }

    #[test]
    fn test_bloom_absent_timer() {
        let timer_id = create_timer_id();
        let bloom = TimerBloomFilter::new(1000, 0.01);

        assert!(!bloom.might_be_expired(&timer_id, 1000, 500));
    }

    #[test]
    fn test_bloom_false_positive_rate() {
        let mut bloom = TimerBloomFilter::new(100_000, 0.01);

        for i in 0..100_000u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let timer_id = TimerId::from_bytes(bytes);
            bloom.insert(&timer_id, i * 1000);
        }

        let mut false_positives = 0u64;
        let check_count = 100_000u64;

        for i in 100_000..200_000u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let timer_id = TimerId::from_bytes(bytes);
            if bloom.might_be_expired(&timer_id, i * 1000, u64::MAX) {
                false_positives += 1;
            }
        }

        let observed_fp_rate = false_positives as f64 / check_count as f64;
        assert!(
            observed_fp_rate < 0.02,
            "FP rate {}% should be < 2%",
            observed_fp_rate * 100.0
        );
    }

    #[test]
    fn test_bloom_rebuild_clears_saturation() {
        let mut bloom = TimerBloomFilter::new(1000, 0.01);

        for i in 0..2000u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let timer_id = TimerId::from_bytes(bytes);
            bloom.insert(&timer_id, i * 1000);
        }

        bloom.rebuild();

        let mut false_positives = 0u64;
        for i in 2000..2100u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let timer_id = TimerId::from_bytes(bytes);
            if bloom.might_be_expired(&timer_id, i * 1000, u64::MAX) {
                false_positives += 1;
            }
        }

        let check_count = 100u64;
        let observed_fp_rate = false_positives as f64 / check_count as f64;
        assert!(
            observed_fp_rate < 0.03,
            "After rebuild, FP rate {}% should be < 3%",
            observed_fp_rate * 100.0
        );
    }

    #[test]
    fn test_bloom_stats_tracking() {
        let timer_id = create_timer_id();
        let mut bloom = TimerBloomFilter::new(1000, 0.01);

        for i in 0..100u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let tid = TimerId::from_bytes(bytes);
            bloom.insert(&tid, i * 1000);
        }

        for i in 0..50u64 {
            let bytes: [u8; 16] = i.to_be_bytes();
            let tid = TimerId::from_bytes(bytes);
            bloom.might_be_expired(&tid, i * 1000, u64::MAX);
        }

        assert_eq!(bloom.stats.insert_count, 100);
        assert_eq!(bloom.stats.check_count, 50);
    }

    #[test]
    fn test_optimal_k_calculation() {
        assert_eq!(TimerBloomFilter::optimal_k(0.01), 7);
        assert_eq!(TimerBloomFilter::optimal_k(0.05), 5);
        assert_eq!(TimerBloomFilter::optimal_k(0.10), 4);
    }

    #[test]
    fn test_optimal_m_calculation() {
        let m = TimerBloomFilter::optimal_m(1_000_000, 0.01);
        assert!(m > 9_000_000 && m < 10_000_000);
    }
}
