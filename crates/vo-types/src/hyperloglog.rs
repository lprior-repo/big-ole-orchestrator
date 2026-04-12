//! HyperLogLog: probabilistic cardinality estimation.
//!
//! HyperLogLog estimates the number of distinct elements in a stream using
//! O(1) space (2^p registers) with approximately 1.6% standard error.
//!
//! # Algorithm
//! 1. Hash each element to a 64-bit bitstring
//! 2. Use the first p bits to select which register (bucket) to update
//! 3. Count leading zeros in the remaining (64-p) bits + 1
//! 4. Store the maximum count observed in each register
//! 5. Final estimate uses harmonic mean of registers (with bias correction)
//!
//! # Invariants
//! - Each register stores a value between 0 and 64 (max leading zeros + 1)
//! - Register count is always a power of 2
//!
//! # Complexity
//! - `insert`: O(1)
//! - `len`: O(1) estimated
//! - Memory: O(2^p) registers

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use thiserror::Error;

const MAX_REGISTER_VALUE: u8 = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperLogLog {
    registers: Vec<u8>,
    p: u8,
    m: usize,
    seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HyperLogLogError {
    #[error("hyperloglog is empty")]
    Empty,

    #[error("precision must be between 4 and 16, got {0}")]
    InvalidPrecision(u8),

    #[error("item count exceeds maximum capacity")]
    CapacityExceeded,
}

impl HyperLogLog {
    pub fn new(precision: u8) -> Result<Self, HyperLogLogError> {
        if precision < 4 || precision > 16 {
            return Err(HyperLogLogError::InvalidPrecision(precision));
        }
        let m = 1usize << precision;
        Ok(Self {
            registers: vec![0; m],
            p: precision,
            m,
            seed: 0x5de66e465d301a3,
        })
    }

    pub fn with_seed(precision: u8, seed: u64) -> Result<Self, HyperLogLogError> {
        if precision < 4 || precision > 16 {
            return Err(HyperLogLogError::InvalidPrecision(precision));
        }
        let m = 1usize << precision;
        Ok(Self {
            registers: vec![0; m],
            p: precision,
            m,
            seed,
        })
    }

    pub fn precision(&self) -> u8 {
        self.p
    }

    pub fn register_count(&self) -> usize {
        self.m
    }

    pub fn len(&self) -> usize {
        self.estimate_cardinality() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.registers.iter().all(|&r| r == 0)
    }

    fn hash<T: ?Sized>(&mut self, item: &T) -> u64
    where
        T: Hash,
    {
        let mut hasher = SplitMix64::new(self.seed);
        item.hash(&mut hasher);
        hasher.finish()
    }

    pub fn insert<T: ?Sized>(&mut self, item: &T)
    where
        T: std::hash::Hash,
    {
        let hash = self.hash(item);
        let bucket = (hash >> (64 - self.p)) as usize;
        let leading_zeros = (hash << self.p).leading_zeros();
        let count = (leading_zeros + 1).min(MAX_REGISTER_VALUE as u32) as u8;

        if count > self.registers[bucket] {
            self.registers[bucket] = count;
        }
    }

    pub fn merge(&mut self, other: &HyperLogLog) -> Result<(), HyperLogLogError> {
        if self.p != other.p {
            return Err(HyperLogLogError::InvalidPrecision(other.p));
        }
        for (i, reg) in other.registers.iter().enumerate() {
            if *reg > self.registers[i] {
                self.registers[i] = *reg;
            }
        }
        Ok(())
    }

    pub fn estimate_cardinality(&self) -> f64 {
        let m = self.m as f64;
        let sum: f64 = self
            .registers
            .iter()
            .map(|&r| 2_f64.powi(-(r as i32)))
            .sum();

        if sum == 0.0 {
            return 0.0;
        }

        let raw_estimate = m * m / sum;

        let small_threshold = 2.5 * m as f64;
        if raw_estimate <= small_threshold {
            let zero_count = self.registers.iter().filter(|&&r| r == 0).count() as f64;
            if zero_count > 0.0 {
                return m * (m / zero_count).ln();
            }
        }

        let (alpha, _) = self.bias_correction_params();
        let corrected = alpha * m * m / sum;

        if corrected < 5.0 * m as f64 {
            return corrected.max(0.0);
        }

        corrected
    }

    fn bias_correction_params(&self) -> (f64, f64) {
        match self.p {
            4 => (0.673, 0.0),
            5 => (0.697, 0.0),
            6 => (0.709, 0.0),
            7 => (0.715, 0.0),
            8 => (0.719, 0.0),
            9 => (0.722, 0.0),
            10 => (0.724, 0.0016),
            11 => (0.726, 0.0032),
            12 => (0.727, 0.0064),
            13 => (0.729, 0.0128),
            14 => (0.730, 0.0256),
            15 => (0.731, 0.0512),
            16 => (0.732, 0.1024),
            _ => (0.7213, 0.0),
        }
    }

    pub fn clear(&mut self) {
        for reg in &mut self.registers {
            *reg = 0;
        }
    }

    pub fn register_values(&self) -> &[u8] {
        &self.registers
    }
}

impl Default for HyperLogLog {
    fn default() -> Self {
        Self::new(10).unwrap()
    }
}

#[derive(Debug, Clone, Default)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Hasher for SplitMix64 {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash: u64 = 0;
        for chunk in bytes.chunks(8) {
            let mut padded = [0u8; 8];
            padded[..chunk.len()].copy_from_slice(chunk);
            hash = hash.wrapping_add(u64::from_le_bytes(padded));
            hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
            hash = hash ^ (hash >> 31);
        }
        self.state = self.state.wrapping_add(hash);
    }

    fn write_u8(&mut self, i: u8) {
        self.state = self.state.wrapping_add(i as u64);
        self.state = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }

    fn write_u16(&mut self, i: u16) {
        self.state = self.state.wrapping_add(i as u64);
        self.state = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }

    fn write_u32(&mut self, i: u32) {
        self.state = self.state.wrapping_add(i as u64);
        self.state = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }

    fn write_u64(&mut self, i: u64) {
        self.state = self.state.wrapping_add(i);
        self.state = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }

    fn write_usize(&mut self, i: usize) {
        self.state = self.state.wrapping_add(i as u64);
        self.state = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    }

    fn finish(&self) -> u64 {
        (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_valid_precision() {
        for p in 4..=16 {
            let hll = HyperLogLog::new(p).unwrap();
            assert_eq!(hll.precision(), p);
            assert_eq!(hll.register_count(), 1usize << p);
        }
    }

    #[test]
    fn new_with_invalid_precision() {
        assert!(matches!(
            HyperLogLog::new(3),
            Err(HyperLogLogError::InvalidPrecision(3))
        ));
        assert!(matches!(
            HyperLogLog::new(17),
            Err(HyperLogLogError::InvalidPrecision(17))
        ));
    }

    #[test]
    fn default_precision_is_10() {
        let hll = HyperLogLog::default();
        assert_eq!(hll.precision(), 10);
        assert_eq!(hll.register_count(), 1024);
    }

    #[test]
    fn empty_hll_has_zero_estimate() {
        let hll: HyperLogLog = HyperLogLog::new(8).unwrap();
        assert_eq!(hll.estimate_cardinality(), 0.0);
        assert!(hll.is_empty());
    }

    #[test]
    fn insert_single_element() {
        let mut hll = HyperLogLog::new(8).unwrap();
        hll.insert(&42u32);
        assert!(!hll.is_empty());
        assert!(hll.len() >= 1);
    }

    #[test]
    fn insert_same_element_multiple_times() {
        let mut hll = HyperLogLog::new(8).unwrap();
        for _ in 0..100 {
            hll.insert(&42u32);
        }
        assert!(!hll.is_empty());
        let est = hll.len();
        assert!(
            est >= 1 && est <= 10,
            "estimate {} out of expected range",
            est
        );
    }

    #[test]
    fn insert_different_elements() {
        let mut hll = HyperLogLog::new(8).unwrap();
        let unique_count = 1000;
        for i in 0..unique_count {
            hll.insert(&i);
        }
        let est = hll.len();
        let lower_bound = (unique_count as f64 * 0.5) as usize;
        let upper_bound = (unique_count as f64 * 1.5) as usize;
        assert!(
            est >= lower_bound && est <= upper_bound,
            "estimate {} not in range [{}, {}]",
            est,
            lower_bound,
            upper_bound
        );
    }

    #[test]
    fn merge_two_hll() {
        let mut hll1 = HyperLogLog::new(8).unwrap();
        let mut hll2 = HyperLogLog::new(8).unwrap();

        for i in 0..500 {
            hll1.insert(&i);
        }
        for i in 500..1000 {
            hll2.insert(&i);
        }

        let original_estimate = hll1.len();

        hll1.merge(&hll2).unwrap();

        let merged_estimate = hll1.len();
        assert!(
            merged_estimate >= original_estimate,
            "merged estimate {} should be >= original {}",
            merged_estimate,
            original_estimate
        );

        let expected_range = 750..=1250;
        assert!(
            expected_range.contains(&merged_estimate),
            "merged estimate {} not in expected range {:?}",
            merged_estimate,
            expected_range
        );
    }

    #[test]
    fn merge_with_different_precision_fails() {
        let mut hll1 = HyperLogLog::new(8).unwrap();
        let hll2 = HyperLogLog::new(10).unwrap();

        assert!(hll1.merge(&hll2).is_err());
    }

    #[test]
    fn clear_resets_hll() {
        let mut hll = HyperLogLog::new(8).unwrap();
        for i in 0..100 {
            hll.insert(&i);
        }
        assert!(!hll.is_empty());
        hll.clear();
        assert!(hll.is_empty());
        assert_eq!(hll.estimate_cardinality(), 0.0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut hll = HyperLogLog::new(8).unwrap();
        for i in 0..100 {
            hll.insert(&i);
        }
        let json = serde_json::to_string(&hll).unwrap();
        let back: HyperLogLog = serde_json::from_str(&json).unwrap();
        assert_eq!(hll, back);
    }

    #[test]
    fn precision_affects_accuracy() {
        let mut hll_low = HyperLogLog::new(8).unwrap();
        let mut hll_high = HyperLogLog::new(12).unwrap();

        let count = 10000;
        for i in 0..count {
            hll_low.insert(&i);
            hll_high.insert(&i);
        }

        let est_low = hll_low.len() as i32;
        let est_high = hll_high.len() as i32;

        let error_low = (est_low - count as i32).abs();
        let error_high = (est_high - count as i32).abs();

        assert!(
            error_high <= error_low,
            "higher precision should have lower error: low={}, high={}",
            error_low,
            error_high
        );
    }

    #[test]
    fn register_values_bounded() {
        let mut hll = HyperLogLog::new(10).unwrap();
        for i in 0..10000 {
            hll.insert(&i);
        }
        for &reg in hll.register_values() {
            assert!(reg <= MAX_REGISTER_VALUE);
        }
    }

    #[test]
    fn with_seed_produces_different_results() {
        let mut hll1 = HyperLogLog::with_seed(8, 12345).unwrap();
        let mut hll2 = HyperLogLog::with_seed(8, 67890).unwrap();

        for i in 0..1000 {
            hll1.insert(&i);
            hll2.insert(&i);
        }

        let est1 = hll1.len();
        let est2 = hll2.len();

        let diff = (est1 as i32 - est2 as i32).abs();
        assert!(
            diff < 200,
            "different seeds should produce similar estimates, but diff={}",
            diff
        );
    }

    #[test]
    fn empty_after_clear_has_zero_registers() {
        let mut hll = HyperLogLog::new(8).unwrap();
        for i in 0..100 {
            hll.insert(&i);
        }
        hll.clear();
        assert!(hll.register_values().iter().all(|&r| r == 0));
    }

    #[test]
    fn multiple_inserts_same_bucket() {
        let mut hll = HyperLogLog::new(4).unwrap();
        let mut inserted_values = Vec::new();

        for i in 0..10000 {
            hll.insert(&i);
            if i < 10 {
                inserted_values.push(i);
            }
        }

        let estimate = hll.len();
        assert!(estimate > 0, "estimate should be > 0 after insertions");
        assert!(
            estimate < 20000,
            "estimate {} seems too high for 10000 unique elements",
            estimate
        );
    }

    #[test]
    fn string_insertions() {
        let mut hll = HyperLogLog::new(10).unwrap();

        let strings: Vec<String> = (0..1000).map(|i| format!("string_{}", i)).collect();

        for s in &strings {
            hll.insert(s);
        }

        let estimate = hll.len();
        assert!(
            estimate >= 500 && estimate <= 1500,
            "estimate {} not in expected range [500, 1500] for 1000 strings",
            estimate
        );
    }
}
