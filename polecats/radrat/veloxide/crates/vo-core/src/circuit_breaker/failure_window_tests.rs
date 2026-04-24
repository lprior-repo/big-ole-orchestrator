use super::*;
use std::time::{Duration, Instant};
use vo_types::BinaryHash;

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

// B-11: Novel hash adds entry
#[test]
fn record_failure_in_window_returns_1_when_first_unique_hash() {
    let mut window = FailureWindow::new();
    let now = Instant::now();
    let count = record_failure_in_window(
        &mut window,
        make_hash("abcdef01"),
        now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 1);
    assert_eq!(window.len(), 1);
    assert_eq!(window.records()[0].hash, make_hash("abcdef01"));
}

// B-11 extended: second unique hash increments count
#[test]
fn record_failure_in_window_returns_2_when_second_unique_hash() {
    let mut window = FailureWindow::new();
    let now = Instant::now();
    record_failure_in_window(
        &mut window,
        make_hash("abcdef01"),
        now,
        Duration::from_secs(600),
    );
    let count = record_failure_in_window(
        &mut window,
        make_hash("abcdef02"),
        now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 2);
    assert_eq!(window.len(), 2);
}

// B-12: Duplicate hash updates timestamp, count unchanged (INV-004)
#[test]
fn record_failure_in_window_returns_same_count_when_duplicate_hash() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();
    record_failure_in_window(
        &mut window,
        make_hash("abcdef01"),
        t0,
        Duration::from_secs(600),
    );
    let t1 = t0 + Duration::from_secs(30);
    let count = record_failure_in_window(
        &mut window,
        make_hash("abcdef01"),
        t1,
        Duration::from_secs(600),
    );
    assert_eq!(count, 1);
    assert_eq!(window.len(), 1);
    assert_eq!(window.records()[0].failed_at, t1); // timestamp updated
}

// B-13: Expired entries evicted before insertion (INV-007)
#[test]
fn record_failure_in_window_evicts_expired_entries_before_insertion() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();
    // Insert 3 entries that will be expired
    let expired_time = t0; // Will be 620s before t_now
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        expired_time,
        Duration::from_secs(600),
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0002"),
        expired_time,
        Duration::from_secs(600),
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0003"),
        expired_time,
        Duration::from_secs(600),
    );
    assert_eq!(window.len(), 3);

    // Now insert a new hash at t0 + 620s (all prior entries are expired)
    let t_now = t0 + Duration::from_secs(620);
    let count = record_failure_in_window(
        &mut window,
        make_hash("aaaa0004"),
        t_now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 1); // Only h4 remains
    assert_eq!(window.len(), 1);
    assert_eq!(window.records()[0].hash, make_hash("aaaa0004"));
}

// Combinatorial: 4 existing, 5th unique = 5
#[test]
fn record_failure_in_window_returns_5_when_fifth_unique_hash() {
    let mut window = FailureWindow::new();
    let now = Instant::now();
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004", "aaaa0005"];
    hashes.iter().take(4).for_each(|h| {
        record_failure_in_window(&mut window, make_hash(h), now, Duration::from_secs(600));
    });
    let count = record_failure_in_window(
        &mut window,
        make_hash(hashes[4]),
        now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 5);
}

// Combinatorial: 4 existing, duplicate = 4
#[test]
fn record_failure_in_window_returns_4_when_duplicate_of_existing() {
    let mut window = FailureWindow::new();
    let now = Instant::now();
    let hashes = ["aaaa0001", "aaaa0002", "aaaa0003", "aaaa0004"];
    hashes.iter().for_each(|h| {
        record_failure_in_window(&mut window, make_hash(h), now, Duration::from_secs(600));
    });
    let count = record_failure_in_window(
        &mut window,
        make_hash("aaaa0002"), // duplicate
        now + Duration::from_secs(5),
        Duration::from_secs(600),
    );
    assert_eq!(count, 4);
}

// Combinatorial: mixed expired+live
#[test]
fn record_failure_in_window_returns_2_when_mixed_expired_and_live() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();
    let expired_time = t0;
    let live_time = t0 + Duration::from_secs(590); // 590s later, still within 600s from t_now

    record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        expired_time, // will be expired at t_now (620s later)
        Duration::from_secs(600),
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0002"),
        live_time, // 590s from t0, 30s from t_now -> still live
        Duration::from_secs(600),
    );

    let t_now = t0 + Duration::from_secs(620);
    let count = record_failure_in_window(
        &mut window,
        make_hash("aaaa0003"),
        t_now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 2); // h2 (live) + h3 (new)
}

// Combinatorial: all expired, duplicate re-adds fresh
#[test]
fn record_failure_in_window_readds_expired_duplicate_as_fresh() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        t0,
        Duration::from_secs(600),
    );
    // Hash was added at t0. At t0+620s, it's expired.
    // Re-add the same hash at t0+620s -> should re-add as fresh (count = 1).
    let t_now = t0 + Duration::from_secs(620);
    let count = record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        t_now,
        Duration::from_secs(600),
    );
    assert_eq!(count, 1);
    assert_eq!(window.len(), 1);
}

// B-18: unique_failures_in_window counts after eviction
#[test]
fn unique_failures_in_window_returns_2_after_evicting_1_expired() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();

    // h1 at t0 - 620s (expired), h2 at t0 - 30s (live), h3 at t0 - 10s (live)
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        t0,
        Duration::from_secs(99999), // huge window so nothing evicted during setup
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0002"),
        t0 + Duration::from_secs(590),
        Duration::from_secs(99999),
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0003"),
        t0 + Duration::from_secs(610),
        Duration::from_secs(99999),
    );
    assert_eq!(window.len(), 3);

    // Now query at t0+620s with 600s window -> h1 (at t0) is expired
    let t_now = t0 + Duration::from_secs(620);
    let count = unique_failures_in_window(&mut window, t_now, Duration::from_secs(600));
    assert_eq!(count, 2);
    assert_eq!(window.len(), 2);
}

// B-19: Empty window returns 0
#[test]
fn unique_failures_in_window_returns_0_when_empty() {
    let mut window = FailureWindow::new();
    let count = unique_failures_in_window(&mut window, Instant::now(), Duration::from_secs(600));
    assert_eq!(count, 0);
}

// B-20: All expired returns 0
#[test]
fn unique_failures_in_window_returns_0_when_all_expired() {
    let mut window = FailureWindow::new();
    let t0 = Instant::now();
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0001"),
        t0,
        Duration::from_secs(99999), // huge window for setup
    );
    record_failure_in_window(
        &mut window,
        make_hash("aaaa0002"),
        t0,
        Duration::from_secs(99999),
    );
    assert_eq!(window.len(), 2);

    let t_now = t0 + Duration::from_secs(700);
    let count = unique_failures_in_window(&mut window, t_now, Duration::from_secs(600));
    assert_eq!(count, 0);
    assert_eq!(window.len(), 0);
}

#[path = "failure_window_proptests.rs"]
mod proptests;
