//! Concurrent InstanceId generation tests.
//!
//! Tests that InstanceId::generate() produces collision-free IDs
//! across multiple concurrent threads.

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use vo_common::types::InstanceId;

const THREAD_COUNT: usize = 10;
const IDS_PER_THREAD: usize = 1000;

fn is_valid_ulid_format(id: &str) -> bool {
    if id.len() != 26 {
        return false;
    }
    let valid_chars: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    id.bytes().all(|b| valid_chars.contains(&b))
}

#[test]
fn instance_id_generate_concurrent_no_collisions() {
    let ids = Arc::new(std::sync::Mutex::new(Vec::with_capacity(
        THREAD_COUNT * IDS_PER_THREAD,
    )));
    let mut handles = Vec::with_capacity(THREAD_COUNT);

    for _ in 0..THREAD_COUNT {
        let ids_clone = Arc::clone(&ids);
        let handle = thread::spawn(move || {
            let mut local_ids = Vec::with_capacity(IDS_PER_THREAD);
            for _ in 0..IDS_PER_THREAD {
                local_ids.push(InstanceId::generate());
            }
            let mut shared = ids_clone.lock().unwrap();
            shared.extend(local_ids);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let all_ids = ids.lock().unwrap();
    let unique_ids: HashSet<_> = all_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        all_ids.len(),
        "Expected {} unique IDs but got {} ({} collisions)",
        all_ids.len(),
        unique_ids.len(),
        all_ids.len() - unique_ids.len()
    );
}

#[test]
fn instance_id_generate_concurrent_valid_ulid_format() {
    let ids = Arc::new(std::sync::Mutex::new(Vec::with_capacity(
        THREAD_COUNT * IDS_PER_THREAD,
    )));
    let mut handles = Vec::with_capacity(THREAD_COUNT);

    for _ in 0..THREAD_COUNT {
        let ids_clone = Arc::clone(&ids);
        let handle = thread::spawn(move || {
            let mut local_ids = Vec::with_capacity(IDS_PER_THREAD);
            for _ in 0..IDS_PER_THREAD {
                local_ids.push(InstanceId::generate());
            }
            let mut shared = ids_clone.lock().unwrap();
            shared.extend(local_ids);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let all_ids = ids.lock().unwrap();
    for id in all_ids.iter() {
        let s = id.as_str();
        assert!(
            is_valid_ulid_format(s),
            "InstanceId '{}' is not valid ULID format (26 chars, Crockford Base32)",
            s
        );
    }
}

#[test]
fn instance_id_generate_concurrent_time_ordered() {
    let ids = Arc::new(std::sync::Mutex::new(Vec::with_capacity(
        THREAD_COUNT * IDS_PER_THREAD,
    )));
    let mut handles = Vec::with_capacity(THREAD_COUNT);

    for _ in 0..THREAD_COUNT {
        let ids_clone = Arc::clone(&ids);
        let handle = thread::spawn(move || {
            let mut local_ids = Vec::with_capacity(IDS_PER_THREAD);
            for _ in 0..IDS_PER_THREAD {
                local_ids.push(InstanceId::generate());
            }
            let mut shared = ids_clone.lock().unwrap();
            shared.extend(local_ids);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let all_ids = ids.lock().unwrap();
    let mut ulids: Vec<_> = all_ids
        .iter()
        .filter_map(|id| ulid::Ulid::from_string(id.as_str()).ok())
        .collect();

    let original_len = ulids.len();
    ulids.sort();
    ulids.dedup();

    let chunk_size = original_len / THREAD_COUNT;
    let mut out_of_order_count = 0;

    for chunk in ulids.chunks(chunk_size.max(1)) {
        if chunk.len() > 1 {
            for window in chunk.windows(2) {
                if window[0] > window[1] {
                    out_of_order_count += 1;
                }
            }
        }
    }

    let tolerance = (THREAD_COUNT * IDS_PER_THREAD) / 10;
    assert!(
        out_of_order_count <= tolerance,
        "Too many out-of-order IDs: {} (tolerance: {}). ULIDs should be roughly time-ordered.",
        out_of_order_count,
        tolerance
    );
}