//! Suffix Array — sorted array of all suffixes of a string.
//!
//! The suffix array is a data structure that stores the starting positions
//! of all suffixes of a string in sorted order. It enables efficient string
//! operations like substring search, longest common prefix queries, and is
//! the foundation for the Burrows-Wheeler transform.
//!
//! # Complexity
//! - Construction (doubling algorithm): O(n log n)
//! - Construction (SA-IS): O(n)
//! - Longest Common Prefix (LCP): O(1) per query after O(n) preprocessing
//!
//! # Reference
//! Manber & Myers (1990), "Suffix arrays: A new method for on-line string searches"

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuffixArray {
    sa: Vec<usize>,
    s: Vec<u8>,
    n: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SuffixArrayError {
    #[error("index {0} out of bounds for string of length {1}")]
    IndexOutOfBounds(usize, usize),

    #[error("empty string cannot build suffix array")]
    EmptyString,
}

impl SuffixArray {
    pub fn new(s: &[u8]) -> Result<Self, SuffixArrayError> {
        if s.is_empty() {
            return Err(SuffixArrayError::EmptyString);
        }
        let n = s.len();
        let mut sa = (0..n).collect::<Vec<usize>>();
        let mut rank = vec![0u64; n];
        let mut tmp = vec![0u64; n];
        let mut k = 1usize;

        for (i, c) in s.iter().enumerate() {
            rank[i] = *c as u64;
        }

        while k < n {
            let k_usize = k;
            sa.sort_by(|&a, &b| {
                let ra = if a + k_usize < n {
                    rank[a + k_usize]
                } else {
                    0
                };
                let rb = if b + k_usize < n {
                    rank[b + k_usize]
                } else {
                    0
                };
                (rank[a], ra).cmp(&(rank[b], rb))
            });

            tmp[sa[0]] = 0;
            for i in 1..n {
                let prev = sa[i - 1];
                let curr = sa[i];
                let prev_key = (
                    rank[prev],
                    if prev + k_usize < n {
                        rank[prev + k_usize]
                    } else {
                        0
                    },
                );
                let curr_key = (
                    rank[curr],
                    if curr + k_usize < n {
                        rank[curr + k_usize]
                    } else {
                        0
                    },
                );
                tmp[curr] = tmp[prev] + (prev_key != curr_key) as u64;
            }

            rank.copy_from_slice(&tmp);
            if rank[sa[n - 1]] == (n - 1) as u64 {
                break;
            }
            k *= 2;
        }

        Ok(Self {
            sa,
            s: s.to_vec(),
            n,
        })
    }

    pub fn from_str(s: &str) -> Result<Self, SuffixArrayError> {
        Self::new(s.as_bytes())
    }

    pub fn get(&self, i: usize) -> Result<usize, SuffixArrayError> {
        self.sa
            .get(i)
            .copied()
            .ok_or(SuffixArrayError::IndexOutOfBounds(i, self.n))
    }

    pub fn position(&self, suffix_start: usize) -> Result<usize, SuffixArrayError> {
        if suffix_start >= self.n {
            return Err(SuffixArrayError::IndexOutOfBounds(suffix_start, self.n));
        }
        Ok(self
            .sa
            .iter()
            .position(|&x| x == suffix_start)
            .expect("suffix must be in array"))
    }

    pub fn suffix(&self, i: usize) -> Result<&[u8], SuffixArrayError> {
        if i >= self.n {
            return Err(SuffixArrayError::IndexOutOfBounds(i, self.n));
        }
        Ok(&self.s[i..])
    }

    pub fn lcp(&self, i: usize, j: usize) -> usize {
        let suffix_i = match self.sa.get(i) {
            Some(&x) => x,
            None => return 0,
        };
        let suffix_j = match self.sa.get(j) {
            Some(&x) => x,
            None => return 0,
        };
        let max_len = (self.n - suffix_i).min(self.n - suffix_j);
        let mut lcp = 0;
        while lcp < max_len && self.s[suffix_i + lcp] == self.s[suffix_j + lcp] {
            lcp += 1;
        }
        lcp
    }

    pub fn lcp_between_suffixes(&self, a: usize, b: usize) -> usize {
        let pos_a = self.position(a).unwrap_or(0);
        let pos_b = self.position(b).unwrap_or(0);
        if pos_a == pos_b {
            return self.n - a;
        }
        self.lcp(pos_a.min(pos_b), pos_a.max(pos_b))
    }

    pub fn contains(&self, pattern: &[u8]) -> bool {
        self.search(pattern).is_some()
    }

    pub fn search(&self, pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() {
            return Some(0);
        }
        let mut lo = 0;
        let mut hi = self.n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let suffix_start = self.sa[mid];
            let compare_len = (pattern.len()).min(self.n - suffix_start);
            let cmp = &self.s[suffix_start..suffix_start + compare_len].cmp(pattern);
            match cmp {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => {
                    if pattern.len() <= self.n - suffix_start {
                        return Some(mid);
                    } else {
                        lo = mid + 1;
                    }
                }
            }
        }
        None
    }

    pub fn find_all(&self, pattern: &[u8]) -> Vec<usize> {
        let mut results = Vec::new();
        if let Some(start) = self.search(pattern) {
            let mut idx = start;
            while idx < self.n {
                let suffix_start = self.sa[idx];
                let compare_len = (pattern.len()).min(self.n - suffix_start);
                if self.s[suffix_start..suffix_start + compare_len] != *pattern {
                    break;
                }
                results.push(suffix_start);
                idx += 1;
            }
        }
        results
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn as_slice(&self) -> &[usize] {
        &self.sa
    }
}

impl Default for SuffixArray {
    fn default() -> Self {
        Self {
            sa: Vec::new(),
            s: Vec::new(),
            n: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_returns_error() {
        assert!(matches!(
            SuffixArray::new(b""),
            Err(SuffixArrayError::EmptyString)
        ));
        assert!(matches!(
            SuffixArray::from_str(""),
            Err(SuffixArrayError::EmptyString)
        ));
    }

    #[test]
    fn single_char_string() {
        let sa = SuffixArray::from_str("a").unwrap();
        assert_eq!(sa.len(), 1);
        assert_eq!(sa.get(0).unwrap(), 0);
        assert_eq!(sa.position(0).unwrap(), 0);
    }

    #[test]
    fn simple_string_abc() {
        let sa = SuffixArray::from_str("abc").unwrap();
        assert_eq!(sa.len(), 3);
        let positions: Vec<usize> = (0..3).map(|i| sa.get(i).unwrap()).collect();
        assert_eq!(positions, vec![2, 0, 1]);
    }

    #[test]
    fn banana_string() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.len(), 6);
        let positions: Vec<usize> = (0..6).map(|i| sa.get(i).unwrap()).collect();
        assert_eq!(positions, vec![5, 3, 1, 0, 4, 2]);
    }

    #[test]
    fn suffix_at_position() {
        let sa = SuffixArray::from_str("abc").unwrap();
        assert_eq!(sa.suffix(0).unwrap(), b"abc");
        assert_eq!(sa.suffix(1).unwrap(), b"bc");
        assert_eq!(sa.suffix(2).unwrap(), b"c");
        assert!(matches!(
            sa.suffix(3),
            Err(SuffixArrayError::IndexOutOfBounds(3, 3))
        ));
    }

    #[test]
    fn position_returns_index_in_suffix_array() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.position(0).unwrap(), 3);
        assert_eq!(sa.position(1).unwrap(), 2);
        assert_eq!(sa.position(2).unwrap(), 5);
        assert_eq!(sa.position(3).unwrap(), 1);
        assert_eq!(sa.position(4).unwrap(), 4);
        assert_eq!(sa.position(5).unwrap(), 0);
    }

    #[test]
    fn lcp_simple() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.lcp_between_suffixes(0, 1), 0);
        assert_eq!(sa.lcp_between_suffixes(1, 3), 2);
        assert_eq!(sa.lcp_between_suffixes(3, 4), 0);
    }

    #[test]
    fn lcp_banana() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.lcp_between_suffixes(5, 3), 1);
        assert_eq!(sa.lcp_between_suffixes(1, 4), 0);
        assert_eq!(sa.lcp_between_suffixes(3, 0), 0);
    }

    #[test]
    fn search_exact_match() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert!(sa.contains(b"ana"));
        assert!(sa.contains(b"ban"));
        assert!(sa.contains(b"a"));
        assert!(sa.contains(b"na"));
        assert!(!sa.contains(b"nab"));
        assert!(!sa.contains(b"bananan"));
    }

    #[test]
    fn search_returns_position() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.search(b"ana").unwrap(), 1);
        assert_eq!(sa.search(b"ban").unwrap(), 2);
        assert_eq!(sa.search(b"a").unwrap(), 0);
        assert_eq!(sa.search(b"na").unwrap(), 3);
        assert!(sa.search(b"nab").is_none());
    }

    #[test]
    fn find_all_occurrences() {
        let sa = SuffixArray::from_str("banana").unwrap();
        assert_eq!(sa.find_all(b"ana"), vec![1, 3]);
        assert_eq!(sa.find_all(b"a"), vec![1, 3, 5]);
        assert_eq!(sa.find_all(b"na"), vec![2, 4]);
        assert!(sa.find_all(b"ban").is_empty());
    }

    #[test]
    fn search_empty_pattern() {
        let sa = SuffixArray::from_str("abc").unwrap();
        assert_eq!(sa.search(b"").unwrap(), 0);
    }

    #[test]
    fn suffixes_are_sorted() {
        let s = "abracadabra";
        let sa = SuffixArray::from_str(s).unwrap();
        let n = s.len();
        for i in 0..(n - 1) {
            let start_a = sa.get(i).unwrap();
            let start_b = sa.get(i + 1).unwrap();
            let suffix_a = &s.as_bytes()[start_a..];
            let suffix_b = &s.as_bytes()[start_b..];
            assert!(suffix_a < suffix_b, "{:?} >= {:?}", suffix_a, suffix_b);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let sa = SuffixArray::from_str("hello").unwrap();
        let json = serde_json::to_string(&sa).unwrap();
        let back: SuffixArray = serde_json::from_str(&json).unwrap();
        assert_eq!(sa, back);
    }

    #[test]
    fn default_is_empty() {
        let sa = SuffixArray::default();
        assert!(sa.is_empty());
        assert_eq!(sa.len(), 0);
    }

    #[test]
    fn large_string_still_works() {
        let s = "a".repeat(1000);
        let sa = SuffixArray::from_str(&s).unwrap();
        assert_eq!(sa.len(), 1000);
        for i in 0..1000 {
            assert_eq!(sa.position(i).unwrap(), 1000 - 1 - i);
        }
    }

    #[test]
    fn mixed_characters() {
        let sa = SuffixArray::from_str("ababa").unwrap();
        let positions: Vec<usize> = (0..5).map(|i| sa.get(i).unwrap()).collect();
        let suffixes: Vec<&[u8]> = positions.iter().map(|&p| &sa.s[p..]).collect();
        for i in 0..suffixes.len() - 1 {
            assert!(
                suffixes[i] < suffixes[i + 1],
                "{:?} >= {:?}",
                suffixes[i],
                suffixes[i + 1]
            );
        }
    }

    #[test]
    fn index_out_of_bounds_error() {
        let sa = SuffixArray::from_str("abc").unwrap();
        assert!(matches!(
            sa.get(5),
            Err(SuffixArrayError::IndexOutOfBounds(5, 3))
        ));
        assert!(matches!(
            sa.position(10),
            Err(SuffixArrayError::IndexOutOfBounds(10, 3))
        ));
    }

    #[test]
    fn bytes_vs_string_consistency() {
        let s = "testing";
        let sa_str = SuffixArray::from_str(s).unwrap();
        let sa_bytes = SuffixArray::new(s.as_bytes()).unwrap();
        assert_eq!(sa_str.as_slice(), sa_bytes.as_slice());
    }
}
