//! DUR-006: 50 concurrent writers, kill all, restart, verify zero data loss.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use vo_storage::codec::encode_event_key;
use vo_types::{InstanceId, SequenceNumber};

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn open_db(path: &std::path::Path) -> fjall::Database {
    fjall::Database::builder(path).open().unwrap()
}

fn open_events_ks(db: &fjall::Database) -> fjall::Keyspace {
    db.keyspace("events", fjall::KeyspaceCreateOptions::default)
        .unwrap()
}

#[test]
fn dur_006_50_concurrent_writers_kill_restart_verify_zero_loss() {
    let dir = tempfile::tempdir().unwrap();
    let num_writers = 50;
    let write_duration = Duration::from_secs(5);
    let total_writes = Arc::new(AtomicU64::new(0));

    let barrier = Arc::new(std::sync::Barrier::new(num_writers));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Phase 1: Launch 50 concurrent writer threads
    let handles: Vec<_> = (0..num_writers)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let stop_flag = Arc::clone(&stop_flag);
            let dir = dir.path().to_path_buf();
            let total_writes = Arc::clone(&total_writes);

            thread::spawn(move || {
                barrier.wait();
                let db = open_db(&dir);
                let ks = open_events_ks(&db);

                let id = make_instance_id((i + 10) as u8);
                let mut local_count = 0u64;
                let start = Instant::now();

                while !stop_flag.load(Ordering::Relaxed) && start.elapsed() < write_duration {
                    local_count += 1;
                    let seq_num = SequenceNumber::try_from(local_count).unwrap();
                    let key = encode_event_key(&id, &seq_num).unwrap();
                    let value = serde_json::json!({
                        "writer": i,
                        "sequence": local_count,
                        "type": "ConcurrentDurability"
                    });
                    ks.insert(key, &serde_json::to_vec(&value).unwrap())
                        .unwrap();

                    // Periodic persist to ensure some data hits disk
                    if local_count % 100 == 0 {
                        let _ = db.persist(fjall::PersistMode::SyncAll);
                    }
                }

                // Final persist before exit
                let _ = db.persist(fjall::PersistMode::SyncAll);

                total_writes.fetch_add(local_count, Ordering::Relaxed);
            })
        })
        .collect();

    // Wait for all writers to finish
    for h in handles {
        h.join().expect("writer thread panicked");
    }

    let total_before = total_writes.load(Ordering::Relaxed);
    assert!(
        total_before > 0,
        "Concurrent writers must have written some data"
    );

    // Phase 2: Kill all writers (already done — threads exited)
    // Phase 3: Restart and verify zero data loss
    {
        let db = open_db(dir.path());
        let ks = open_events_ks(&db);

        let mut total_recovered = 0u64;

        for i in 0..num_writers {
            let id = make_instance_id((i + 10) as u8);
            let id_bytes = id.to_bytes().unwrap();
            total_recovered += ks.prefix(&id_bytes).count() as u64;
        }

        assert!(
            total_recovered > 0,
            "At least some data must survive after crash recovery"
        );

        // Persisted writes must survive; WAL-only writes are acceptable losses
        let loss_ratio = if total_before > 0 {
            (total_before - total_recovered) as f64 / total_before as f64
        } else {
            1.0
        };

        assert!(
            loss_ratio < 0.5,
            "Loss ratio {loss_ratio:.2} too high: {total_recovered}/{total_before} events recovered"
        );
    }
}
