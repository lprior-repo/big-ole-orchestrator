//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: concurrent AtomicBool guards.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::json;

use crate::tests::{
    read_input_inner_with_atomic_guard as read_input_inner_atomic,
    write_success_inner_with_state as write_success_inner,
};

use super::valid_envelope;

#[test]
fn concurrent_write_success_only_one_succeeds() {
    let guard = Arc::new(AtomicBool::new(false));
    let success_count = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let guard = Arc::clone(&guard);
        let success_count = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            let mut buf: Vec<u8> = Vec::new();
            let mut local_guard = false;
            if guard
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                local_guard = true;
            }
            let result = write_success_inner(&mut buf, &json!("ok"), &mut local_guard);
            if result.is_ok() {
                success_count.store(true, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let succeeded = success_count.load(Ordering::SeqCst);
    assert_eq!(succeeded, true, "exactly one write should succeed");
}

#[test]
fn concurrent_read_input_only_one_succeeds() {
    let guard = Arc::new(AtomicBool::new(false));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let guard = Arc::clone(&guard);
        let success_count = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            let payload = valid_envelope("key-abc", &json!(null));
            let mut cursor = Cursor::new(payload);
            let result = read_input_inner_atomic(&mut cursor, &guard);
            if result.is_ok() {
                success_count.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let count = success_count.load(Ordering::SeqCst);
    assert_eq!(count, 1, "exactly one read should succeed");
}
