use crate::timer_index::TimerKey;

/// A simple Bloom filter for timer keys using BLAKE3 hashing.
///
/// Provides probabilistic membership queries for timer keys with
/// configurable false positive rate. Does NOT track expiration state —
/// `might_be_expired()` checks whether a key might have been inserted
/// (and therefore might need expiration checking).
pub struct TimerBloomFilter {
    bits: Vec<u64>,
    num_hash_functions: u32,
    num_bits: u64,
}

impl TimerBloomFilter {
    /// Creates a new `TimerBloomFilter` sized for approximately `capacity`
    /// elements with a target false positive rate of ~5%.
    ///
    /// Uses the formula:
    /// - m = -n * ln(p) / (ln(2))^2
    /// - k = (m/n) * ln(2)
    pub fn new(capacity: u64) -> Self {
        const TARGET_FPR: f64 = 0.05;
        let m = (-(capacity as f64) * TARGET_FPR.ln() / (0.6931471805599453_f64).powi(2))
            .ceil() as u64;
        // Round up to next power of 2 for efficient bit manipulation
        let m = m.next_power_of_two();
        let k = ((m as f64) / (capacity as f64) * 0.6931471805599453_f64).ceil() as u32;

        Self {
            bits: vec![0u64; (m / 64 + if m % 64 != 0 { 1 } else { 0 }) as usize],
            num_hash_functions: k,
            num_bits: m,
        }
    }

    /// Returns the index of the bit to set/query for a given seed.
    fn hash(&self, key: &[u8], seed: u32) -> u64 {
        // Use BLAKE3 with domain separation via seed
        let mut hasher = blake3::Hasher::new();
        hasher.update(key);
        hasher.update(&seed.to_be_bytes());
        let hash = hasher.finalize();

        // Extract two 32-bit values for double hashing (Kirsch-Mitzenmacher optimization)
        let bytes = hash.as_bytes();
        let h1 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let h2 = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        // Combined index using k * h1 + h2
        let combined = (h1 as u64).wrapping_mul(self.num_hash_functions as u64)
            + (h2 as u64);
        combined % self.num_bits
    }

    /// Inserts a timer key into the filter.
    pub fn insert(&mut self, key: &TimerKey) {
        let bytes = key.as_bytes();
        for i in 0..self.num_hash_functions {
            let idx = self.hash(bytes, i);
            self.set_bit(idx);
        }
    }

    /// Returns true if the key *might* be in the filter (possible false positive).
    pub fn might_be_expired(&self, key: &TimerKey) -> bool {
        let bytes = key.as_bytes();
        for i in 0..self.num_hash_functions {
            let idx = self.hash(bytes, i);
            if !self.get_bit(idx) {
                return false;
            }
        }
        true
    }

    /// Clears all bits in the filter.
    pub fn clear(&mut self) {
        for word in &mut self.bits {
            *word = 0;
        }
    }

    /// Returns an approximate count of set bits.
    pub fn population(&self) -> u64 {
        self.bits.iter().map(|w| w.count_ones() as u64).sum()
    }

    /// Returns the number of elements that were estimated to be inserted.
    #[must_use]
    pub fn num_hash_functions(&self) -> u32 {
        self.num_hash_functions
    }

    #[must_use]
    pub fn num_bits(&self) -> u64 {
        self.num_bits
    }

    #[inline]
    fn set_bit(&mut self, idx: u64) {
        let word_idx = (idx / 64) as usize;
        let bit_idx = (idx % 64) as u32;
        self.bits[word_idx] |= 1u64 << bit_idx;
    }

    #[inline]
    fn get_bit(&self, idx: u64) -> bool {
        let word_idx = (idx / 64) as usize;
        let bit_idx = (idx % 64) as u32;
        self.bits[word_idx] & (1u64 << bit_idx) != 0
    }
}

impl Default for TimerBloomFilter {
    fn default() -> Self {
        Self::new(100)
    }
}
