// PROP-02: INV-004 — Duplicate hashes never double-count
// PROP-03: INV-007 — Expired entries always evicted
// PROP-10: FailureWindow ordering invariant

use super::*;
use proptest::prelude::*;

proptest! {
    // PROP-02: Duplicate hashes never double-count
    #[test]
    fn duplicate_hashes_never_double_count(
        num_insertions in 1usize..=20,
        pool_idx in proptest::collection::vec(0usize..3, 1..=20),
    ) {
        let pool = ["aaaa0001", "aaaa0002", "aaaa0003"];
        let now = Instant::now();
        let mut window = FailureWindow::new();

        let actual_insertions = num_insertions.min(pool_idx.len());
        let mut seen = std::collections::HashSet::new();
        pool_idx.iter().take(actual_insertions).for_each(|&idx| {
            let hash_str = pool[idx % pool.len()];
            seen.insert(hash_str);
            record_failure_in_window(
                &mut window,
                make_hash(hash_str),
                now,
                Duration::from_secs(600),
            );
        });
        let count = unique_failures_in_window(
            &mut window,
            now,
            Duration::from_secs(600),
        );
        prop_assert_eq!(count, seen.len());
    }

    // PROP-03: INV-007 — Expired entries always evicted
    #[test]
    fn expired_entries_always_evicted_after_unique_failures_call(
        window_secs in 1u64..=1200,
        num_records in 1usize..=10,
    ) {
        let t0 = Instant::now();
        let window_duration = Duration::from_secs(window_secs);
        let mut window = FailureWindow::new();

        // Insert records at various times in [t0, t0 + 2*window]
        (0..num_records).for_each(|i| {
            let offset = Duration::from_secs(
                (i as u64) * window_secs * 2 / (num_records as u64).max(1),
            );
            let hash_str = format!("{:08x}", i);
            if let Ok(hash) = BinaryHash::parse(&hash_str) {
                record_failure_in_window(
                    &mut window,
                    hash,
                    t0 + offset,
                    Duration::from_secs(99999), // no eviction during setup
                );
            }
        });

        let now = t0 + Duration::from_secs(window_secs * 2);
        unique_failures_in_window(&mut window, now, window_duration);

        // After call, every remaining record must be within window
        let all_within_window = window.records().iter().all(|r| {
            let elapsed = now.duration_since(r.failed_at);
            elapsed <= window_duration
        });
        prop_assert!(all_within_window,
            "Found record outside window after eviction. Window: {window_duration:?}"
        );
    }

    // PROP-10: FailureWindow ordering invariant
    #[test]
    fn failure_window_records_sorted_by_failed_at(
        num_insertions in 1usize..=10,
    ) {
        let t0 = Instant::now();
        let mut window = FailureWindow::new();

        // Insert in reverse order to test sorting
        (0..num_insertions).rev().for_each(|i| {
            let offset = Duration::from_secs(i as u64 * 10);
            let hash_str = format!("{:08x}", i);
            if let Ok(hash) = BinaryHash::parse(&hash_str) {
                record_failure_in_window(
                    &mut window,
                    hash,
                    t0 + offset,
                    Duration::from_secs(99999),
                );
            }
        });

        // Verify sorted ascending by failed_at
        let records = window.records();
        let sorted = records.windows(2).all(|pair| {
            pair[0].failed_at <= pair[1].failed_at
        });
        prop_assert!(sorted, "Records not sorted by failed_at ascending");
    }
}
