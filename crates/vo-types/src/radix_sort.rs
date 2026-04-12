//! Radix sort implementation for unsigned integers.
//!
//! This module provides an LSD (Least Significant Digit) radix sort implementation
//! optimized for sorting u64 values in O(k * n) time where k is the number of bytes.
//!
//! # Invariants
//! - Input slice must contain valid u64 values
//! - Output is a permutation of input (stable sort)
//!
//! # Complexity
//! - `radix_sort`: O(k * n) worst-case, where k = 8 bytes
//! - `radix_sort_by_key`: O(k * n) worst-case with custom key extractor
//! - Memory: O(n) auxiliary space for counting sort buffers

const RADIX_BITS: u32 = 8;
const RADIX_SIZE: usize = 256;
const RADIX_MASK: u64 = (1u64 << RADIX_BITS) - 1;
const NUM_PASSES: u32 = 64 / RADIX_BITS;

pub fn radix_sort(input: &mut [u64]) {
    if input.len() <= 1 {
        return;
    }

    let mut buffer = input.to_vec();
    let mut pass = 0;

    while pass < NUM_PASSES {
        counting_sort(input, &mut buffer, pass);
        if pass + 1 < NUM_PASSES {
            input.copy_from_slice(&buffer);
        }
        pass += 1;
    }
}

fn counting_sort(input: &[u64], output: &mut [u64], pass: u32) {
    let mut counts = [0u64; RADIX_SIZE];
    let shift = pass * RADIX_BITS;

    for &val in input.iter() {
        let digit = (val >> shift) & RADIX_MASK;
        counts[digit as usize] += 1;
    }

    for i in 1..RADIX_SIZE {
        counts[i] += counts[i - 1];
    }

    for &val in input.iter().rev() {
        let digit = (val >> shift) & RADIX_MASK;
        let count_idx = (counts[digit as usize] - 1) as usize;
        output[count_idx] = val;
        counts[digit as usize] -= 1;
    }
}

pub fn radix_sort_by_key<T, F>(input: &mut [T], mut key_fn: F)
where
    F: FnMut(&T) -> u64,
    T: Clone,
{
    if input.len() <= 1 {
        return;
    }

    let mut keys: Vec<u64> = input.iter().map(|item| key_fn(item)).collect();
    let mut indices: Vec<usize> = (0..input.len()).collect();

    let mut pass = 0;
    while pass < NUM_PASSES {
        counting_sort_by_key(&keys, &mut indices, pass);
        pass += 1;
    }

    let mut buffer = input.to_vec();
    for (i, &idx) in indices.iter().enumerate() {
        buffer[i] = input[idx].clone();
    }
    input.copy_from_slice(&buffer);
}

fn counting_sort_by_key(keys: &[u64], indices: &mut [usize], pass: u32) {
    let mut counts = [0u64; RADIX_SIZE];
    let shift = pass * RADIX_BITS;

    for &idx in indices.iter() {
        let key = keys[idx];
        let digit = (key >> shift) & RADIX_MASK;
        counts[digit as usize] += 1;
    }

    for i in 1..RADIX_SIZE {
        counts[i] += counts[i - 1];
    }

    let len = indices.len();
    for i in (0..len).rev() {
        let idx = indices[i];
        let key = keys[idx];
        let digit = (key >> shift) & RADIX_MASK;
        let count_idx = (counts[digit as usize] - 1) as usize;
        indices[i] = count_idx;
        counts[digit as usize] -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radix_sort_empty() {
        let mut arr: Vec<u64> = vec![];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![]);
    }

    #[test]
    fn radix_sort_single_element() {
        let mut arr = vec![42u64];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![42]);
    }

    #[test]
    fn radix_sort_already_sorted() {
        let mut arr = vec![1u64, 2, 3, 4, 5];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn radix_sort_reverse_sorted() {
        let mut arr = vec![5u64, 4, 3, 2, 1];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn radix_sort_random() {
        let mut arr = vec![3u64, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![1, 1, 2, 3, 3, 4, 5, 5, 5, 6, 9]);
    }

    #[test]
    fn radix_sort_duplicates() {
        let mut arr = vec![7u64, 7, 7, 7];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![7, 7, 7, 7]);
    }

    #[test]
    fn radix_sort_zeros() {
        let mut arr = vec![0u64, 0, 0];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![0, 0, 0]);
    }

    #[test]
    fn radix_sort_max_u64() {
        let mut arr = vec![u64::MAX, u64::MIN, 0, 1, u64::MAX - 1];
        radix_sort(&mut arr);
        assert_eq!(arr, vec![0, 1, u64::MIN, u64::MAX - 1, u64::MAX]);
    }

    #[test]
    fn radix_sort_large_array() {
        let mut arr: Vec<u64> = (0..1000).rev().collect();
        radix_sort(&mut arr);
        for i in 0..1000 {
            assert_eq!(arr[i], i as u64);
        }
    }

    #[test]
    fn radix_sort_by_key_simple() {
        #[derive(Clone)]
        struct Item(u64);

        let mut items = vec![Item(5), Item(3), Item(7), Item(1)];
        radix_sort_by_key(&mut items, |item| item.0);
        assert_eq!(
            items.iter().map(|i| i.0).collect::<Vec<_>>(),
            vec![1, 3, 5, 7]
        );
    }

    #[test]
    fn radix_sort_by_key_with_struct() {
        #[derive(Clone)]
        struct Job {
            id: u64,
            fire_at_ms: u64,
        }

        let mut jobs = vec![
            Job {
                id: 1,
                fire_at_ms: 100,
            },
            Job {
                id: 2,
                fire_at_ms: 50,
            },
            Job {
                id: 3,
                fire_at_ms: 75,
            },
        ];

        radix_sort_by_key(&mut jobs, |job| job.fire_at_ms);
        assert_eq!(jobs[0].fire_at_ms, 50);
        assert_eq!(jobs[1].fire_at_ms, 75);
        assert_eq!(jobs[2].fire_at_ms, 100);
    }

    #[test]
    fn radix_sort_preserves_stability() {
        #[derive(Clone, Debug, PartialEq)]
        struct Record {
            key: u64,
            value: char,
        }

        let mut records = vec![
            Record { key: 1, value: 'a' },
            Record { key: 2, value: 'x' },
            Record { key: 1, value: 'b' },
            Record { key: 1, value: 'c' },
            Record { key: 2, value: 'y' },
        ];

        radix_sort_by_key(&mut records, |r| r.key);
        let keys: Vec<u64> = records.iter().map(|r| r.key).collect();
        assert_eq!(keys, vec![1, 1, 1, 2, 2]);
    }
}
