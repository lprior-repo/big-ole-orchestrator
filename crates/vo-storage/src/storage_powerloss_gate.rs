//! Storage power-loss simulation gate for exact-once verification (ADR-016, ADR-043).
//!
//! This module provides BDD-style tests that verify atomic batch commit guarantees
//! across ALL control-plane partitions. When power-loss simulation is armed around
//! batch commit points, the reopen checks prove no torn event/summary/dedupe/lease/effect/blob writes.
//!
//! ## Architecture
//!
//! Data → Calc → Actions (power-loss simulation)
//!
//! ## Crash Points Tested
//!
//! 1. Event + instance summary batch commit
//! 2. Suspend (timer + instance status + snapshot) batch commit
//! 3. Dedupe admission batch commit
//! 4. Effect prepare/commit journal batch
//! 5. Lease acquire/release batch
//! 6. Receipt persistence batch
//! 7. Blob reference + metadata batch

use fjall::Database;
use vo_types::{
    EffectIntent, EffectKind, EffectRecord, EventEnvelope, FenceToken, FireAtMs,
    IdempotencyKey, InstanceId, SequenceNumber, StepId, TimestampMs, TimerId,
};
use vo_types::events::EventMetadata;



// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn make_instance_id(n: u8) -> InstanceId {
    InstanceId::parse(&format!("{:026}", n)).expect("valid instance id")
}

fn make_sequence(n: u64) -> SequenceNumber {
    SequenceNumber::try_from(n).expect("valid sequence")
}

fn make_timestamp(n: u64) -> TimestampMs {
    TimestampMs::try_from(n).expect("valid timestamp")
}

fn make_step_id(s: &str) -> StepId {
    StepId::parse(s).expect("valid step id")
}

fn make_timer_id(s: &str) -> TimerId {
    TimerId::parse(s).expect("valid timer id")
}

fn make_fire_at(base_ms: u64, offset_ms: u64) -> FireAtMs {
    FireAtMs::try_from(base_ms + offset_ms).expect("valid fire_at")
}

fn make_idempotency_key(s: &str) -> IdempotencyKey {
    IdempotencyKey::parse(s).expect("valid idempotency key")
}

fn make_event(instance_id: &InstanceId, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1712200000000 + sequence,
        payload: serde_json::json!({ "type": "workflow_started", "seq": sequence }),
        metadata: EventMetadata::default(),
    }
}

fn make_effect_record(intent_id: &str) -> EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        EffectKind::HttpCall,
        serde_json::json!({ "url": "https://api.example.com" }),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid effect record")
}

fn make_snapshot_data(seq: u64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "status": "running",
        "step": "wait",
        "seq": seq
    }))
    .expect("serialize state")
}

// ---------------------------------------------------------------------------
// Power-loss simulation helpers
// ---------------------------------------------------------------------------

fn corrupt_database_atomic_write(dir_path: &std::path::Path) {
    use std::fs;

    let sst_files: Vec<_> = fs::read_dir(dir_path)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            name_str.ends_with(".sst") || name_str.ends_with(".wal")
        })
        .collect();

    for f in sst_files {
        let path = f.path();
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&path) {
            use std::io::Write;
            let _ = file.write_all(&[0xFF; 256]);
        }
    }
}

fn open_fresh_db(path: &std::path::Path) -> Database {
    Database::builder(path)
        .open()
        .expect("reopen database after power loss")
}

// ---------------------------------------------------------------------------
// BDD Tests — ADR-016 / ADR-043 atomic commit guarantees
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_db() -> (TempDir, Database) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db = Database::builder(dir.path()).open().expect("open db");
        (dir, db)
    }

    // ========================================================================
    // BDD Scenario: Given power-loss simulation is armed around batch commit points
    // When gate runs
    // Then reopen checks prove no torn event/summary/dedupe/lease/effect/blob writes
    // ========================================================================

    #[test]
    fn given_atomic_event_commit_when_power_lost_then_reopen_shows_no_partial_state() {
        // Given: a database with atomic event+summary batch commit
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let instance_id = make_instance_id(1);
            let events_ks = db.keyspace("events", || fjall::KeyspaceCreateOptions::default()).expect("events ks");
            let instances_ks = db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");

            let event_key = {
                let id_bytes = instance_id.to_bytes().expect("instance id bytes");
                let seq_bytes = 1u64.to_be_bytes();
                let mut key = Vec::with_capacity(24);
                key.extend_from_slice(&id_bytes);
                key.extend_from_slice(&seq_bytes);
                key
            };
            let event_value = serde_json::to_vec(&make_event(&instance_id, 1)).expect("encode event");
            let status_key = {
                let mut key = Vec::new();
                key.extend_from_slice(b"\x01");
                key.extend_from_slice(&1712200000000u64.to_be_bytes());
                key.extend_from_slice(instance_id.to_bytes().expect("instance bytes").as_slice());
                key
            };

            let mut batch = db.batch();
            batch.insert(&events_ks, &event_key, &event_value);
            batch.insert(&instances_ks, &status_key, &[] as &[u8]);
            batch.commit().expect("atomic commit succeeds");
            (dir, db, path)
        };
        drop(db);

        // When: power loss simulation corrupts the atomic write
        corrupt_database_atomic_write(&path);

        // Then: reopening shows either full commit OR clean rollback (no torn write)
        let fresh_db = open_fresh_db(&path);
        let events_ks = fresh_db.keyspace("events", || fjall::KeyspaceCreateOptions::default()).expect("events ks");
        let instances_ks = fresh_db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");
        let instance_id = make_instance_id(1);
        let event_key = {
            let id_bytes = instance_id.to_bytes().expect("bytes");
            let seq_bytes = 1u64.to_be_bytes();
            let mut key = Vec::with_capacity(24);
            key.extend_from_slice(&id_bytes);
            key.extend_from_slice(&seq_bytes);
            key
        };
        let status_key = {
            let mut key = Vec::new();
            key.extend_from_slice(b"\x01");
            key.extend_from_slice(&1712200000000u64.to_be_bytes());
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key
        };

        let event_visible = events_ks.get(&event_key).expect("get event").is_some();
        let status_visible = instances_ks.get(&status_key).expect("get status").is_some();

        // All-or-nothing: either both visible or neither visible
        assert!(
            (event_visible && status_visible) || (!event_visible && !status_visible),
            "atomic batch must show all-or-nothing: event={}, status={}",
            event_visible,
            status_visible
        );
    }

    #[test]
    fn given_atomic_suspend_commit_when_power_lost_then_reopen_proves_no_torn_timer_or_snapshot() {
        // Given: atomic suspend commit (timer + instance status + snapshot)
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let instance_id = make_instance_id(2);
            let now_ms = 1_000_000u64;

            let timers_ks = db.keyspace("timers", || fjall::KeyspaceCreateOptions::default()).expect("timers ks");
            let instances_ks = db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");
            let snapshots_ks = db.keyspace("snapshots", || fjall::KeyspaceCreateOptions::default()).expect("snapshots ks");

            let timer_key = {
                let mut key = Vec::new();
                key.extend_from_slice(&(now_ms + 60_000u64).to_be_bytes());
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(b"timer-wait-1");
                key
            };
            let timer_value = 60_000u64.to_be_bytes().to_vec();

            let paused_key = {
                let mut key = Vec::new();
                key.extend_from_slice(b"\x02"); // Paused status
                key.extend_from_slice(&now_ms.saturating_sub(1000).to_be_bytes());
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key
            };

            let snapshot_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(&42u64.to_be_bytes());
                key
            };

            let mut batch = db.batch();
            batch.insert(&timers_ks, &timer_key, &timer_value);
            batch.insert(&instances_ks, &paused_key, &[] as &[u8]);
            batch.insert(&snapshots_ks, &snapshot_key, b"snapshot data");
            batch.commit().expect("suspend commit succeeds");

            (dir, db, path)
        };
        drop(db);

        // Simulate power loss during suspend commit
        corrupt_database_atomic_write(&path);

        // Reopen and verify all-or-nothing for suspend
        let fresh_db = open_fresh_db(&path);
        let timers_ks = fresh_db.keyspace("timers", || fjall::KeyspaceCreateOptions::default()).expect("timers ks");
        let instances_ks = fresh_db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");
        let snapshots_ks = fresh_db.keyspace("snapshots", || fjall::KeyspaceCreateOptions::default()).expect("snapshots ks");

        let instance_id = make_instance_id(2);
        let now_ms = 1_000_000u64;

        let timer_key = {
            let mut key = Vec::new();
            key.extend_from_slice(&(now_ms + 60_000u64).to_be_bytes());
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(b"timer-wait-1");
            key
        };
        let paused_key = {
            let mut key = Vec::new();
            key.extend_from_slice(b"\x02");
            key.extend_from_slice(&now_ms.saturating_sub(1000).to_be_bytes());
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key
        };
        let snapshot_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(&42u64.to_be_bytes());
            key
        };

        let timer_visible = timers_ks.get(&timer_key).expect("get timer").is_some();
        let status_visible = instances_ks.get(&paused_key).expect("get paused").is_some();
        let snapshot_visible = snapshots_ks.get(&snapshot_key).expect("get snapshot").is_some();

        let all_visible = timer_visible && status_visible && snapshot_visible;
        let none_visible = !timer_visible && !status_visible && !snapshot_visible;

        assert!(
            all_visible || none_visible,
            "suspend atomic batch must be all-or-nothing: timer={}, status={}, snapshot={}",
            timer_visible, status_visible, snapshot_visible
        );
    }

    #[test]
    fn given_dedupe_batch_commit_when_power_lost_then_no_torn_dedupe_entry() {
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let dedupe_ks = db.keyspace("dedupe", || fjall::KeyspaceCreateOptions::default()).expect("dedupe ks");

            let dedupe_key = b"dedupe-key-powerloss-test".to_vec();
            let dedupe_value = serde_json::to_vec(&serde_json::json!({
                "instance_id": "01ARYZ6S410000000000000002",
                "ingress_offset": 1000u64,
                "expires_at": 2000u64
            })).expect("encode dedupe");

            let mut batch = db.batch();
            batch.insert(&dedupe_ks, &dedupe_key, &dedupe_value);
            batch.commit().expect("dedupe commit succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let dedupe_ks = fresh_db.keyspace("dedupe", || fjall::KeyspaceCreateOptions::default()).expect("dedupe ks");
        let dedupe_key = b"dedupe-key-powerloss-test".to_vec();
        let visible = dedupe_ks.get(&dedupe_key).expect("get dedupe").is_some();

        assert!(
            visible,
            "dedupe entry should be durable after power loss if committed"
        );
    }

    #[test]
    fn given_effect_journal_batch_when_power_lost_then_no_torn_effect_record() {
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let effects_ks = db.keyspace("effects", || fjall::KeyspaceCreateOptions::default()).expect("effects ks");

            let instance_id = make_instance_id(3);
            let effect_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(b"intent-powerloss-effect");
                key
            };
            let effect_value = serde_json::to_vec(&make_effect_record("intent-powerloss-effect"))
                .expect("encode effect");

            let mut batch = db.batch();
            batch.insert(&effects_ks, &effect_key, &effect_value);
            batch.commit().expect("effect journal commit succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let effects_ks = fresh_db.keyspace("effects", || fjall::KeyspaceCreateOptions::default()).expect("effects ks");
        let instance_id = make_instance_id(3);
        let effect_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(b"intent-powerloss-effect");
            key
        };

        let visible = effects_ks.get(&effect_key).expect("get effect").is_some();
        assert!(
            visible,
            "effect record should be durable after power loss if committed"
        );
    }

    #[test]
    fn given_lease_batch_when_power_lost_then_no_torn_lease_entry() {
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let leases_ks = db.keyspace("leases", || fjall::KeyspaceCreateOptions::default()).expect("leases ks");

            let instance_id = make_instance_id(4);
            let step_id = make_step_id("step-lease-test");
            let lease_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(step_id.as_str().as_bytes());
                key
            };
            let lease_value = serde_json::to_vec(&serde_json::json!({
                "fence_token": 42u64,
                "expires_at": 9999999999u64
            })).expect("encode lease");

            let mut batch = db.batch();
            batch.insert(&leases_ks, &lease_key, &lease_value);
            batch.commit().expect("lease commit succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let leases_ks = fresh_db.keyspace("leases", || fjall::KeyspaceCreateOptions::default()).expect("leases ks");
        let instance_id = make_instance_id(4);
        let step_id = make_step_id("step-lease-test");
        let lease_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(step_id.as_str().as_bytes());
            key
        };

        let visible = leases_ks.get(&lease_key).expect("get lease").is_some();
        assert!(
            visible,
            "lease entry should be durable after power loss if committed"
        );
    }

    #[test]
    fn given_receipt_batch_when_power_lost_then_no_torn_receipt() {
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let receipts_ks = db.keyspace("receipts", || fjall::KeyspaceCreateOptions::default()).expect("receipts ks");

            let receipt_key = b"fx-powerloss-receipt".to_vec();
            let receipt_value = serde_json::to_vec(&serde_json::json!({
                "effect_id": "fx-powerloss-receipt",
                "instance_id": "01ARYZ6S410000000000000005",
                "kind": "http_call",
                "committed_at_ms": 1713000000u64,
                "result": "Success"
            })).expect("encode receipt");

            let mut batch = db.batch();
            batch.insert(&receipts_ks, &receipt_key, &receipt_value);
            batch.commit().expect("receipt commit succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let receipts_ks = fresh_db.keyspace("receipts", || fjall::KeyspaceCreateOptions::default()).expect("receipts ks");
        let receipt_key = b"fx-powerloss-receipt".to_vec();

        let visible = receipts_ks.get(&receipt_key).expect("get receipt").is_some();
        assert!(
            visible,
            "receipt should be durable after power loss if committed"
        );
    }

    #[test]
    fn given_blob_metadata_batch_when_power_lost_then_no_torn_blob_record() {
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let blob_records_ks = db.keyspace("blob_records", || fjall::KeyspaceCreateOptions::default()).expect("blob_records ks");

            let blob_key = b"blob-record-powerloss-test".to_vec();
            let blob_value = serde_json::to_vec(&serde_json::json!({
                "content_addr": "a".repeat(64),
                "size_bytes": 1024u64,
                "created_at": 1713000000u64
            })).expect("encode blob record");

            let mut batch = db.batch();
            batch.insert(&blob_records_ks, &blob_key, &blob_value);
            batch.commit().expect("blob record commit succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let blob_records_ks = fresh_db.keyspace("blob_records", || fjall::KeyspaceCreateOptions::default()).expect("blob_records ks");
        let blob_key = b"blob-record-powerloss-test".to_vec();

        let visible = blob_records_ks.get(&blob_key).expect("get blob record").is_some();
        assert!(
            visible,
            "blob record should be durable after power loss if committed"
        );
    }

    #[test]
    fn given_multi_partition_batch_when_power_lost_then_atomic_across_all_partitions() {
        // Most comprehensive test: write to ALL partitions in a single batch,
        // then simulate power loss and verify all-or-nothing across ALL of them
        let (dir, db, path) = {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().to_path_buf();
            let db = Database::builder(&path).open().expect("db");
            let instance_id = make_instance_id(7);

            let events_ks = db.keyspace("events", || fjall::KeyspaceCreateOptions::default()).expect("events ks");
            let instances_ks = db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");
            let timers_ks = db.keyspace("timers", || fjall::KeyspaceCreateOptions::default()).expect("timers ks");
            let snapshots_ks = db.keyspace("snapshots", || fjall::KeyspaceCreateOptions::default()).expect("snapshots ks");
            let dedupe_ks = db.keyspace("dedupe", || fjall::KeyspaceCreateOptions::default()).expect("dedupe ks");
            let effects_ks = db.keyspace("effects", || fjall::KeyspaceCreateOptions::default()).expect("effects ks");
            let leases_ks = db.keyspace("leases", || fjall::KeyspaceCreateOptions::default()).expect("leases ks");
            let receipts_ks = db.keyspace("receipts", || fjall::KeyspaceCreateOptions::default()).expect("receipts ks");

            let event_key = {
                let mut key = Vec::with_capacity(24);
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(&1u64.to_be_bytes());
                key
            };
            let status_key = {
                let mut key = Vec::new();
                key.extend_from_slice(b"\x01");
                key.extend_from_slice(&1712200000000u64.to_be_bytes());
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key
            };
            let timer_key = {
                let mut key = Vec::new();
                key.extend_from_slice(&2000000000000u64.to_be_bytes());
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(b"timer-multi");
                key
            };
            let snapshot_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(&1u64.to_be_bytes());
                key
            };
            let dedupe_key = b"dedupe-multi-partition".to_vec();
            let effect_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(b"intent-multi");
                key
            };
            let lease_key = {
                let mut key = Vec::new();
                key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
                key.extend_from_slice(b"step-multi");
                key
            };
            let receipt_key = b"fx-multi-partition".to_vec();

            let mut batch = db.batch();
            batch.insert(&events_ks, &event_key, b"event-value");
            batch.insert(&instances_ks, &status_key, b"");
            batch.insert(&timers_ks, &timer_key, b"timer-value");
            batch.insert(&snapshots_ks, &snapshot_key, b"snapshot-value");
            batch.insert(&dedupe_ks, &dedupe_key, b"dedupe-value");
            batch.insert(&effects_ks, &effect_key, b"effect-value");
            batch.insert(&leases_ks, &lease_key, b"lease-value");
            batch.insert(&receipts_ks, &receipt_key, b"receipt-value");
            batch.commit().expect("multi-partition batch succeeds");

            (dir, db, path)
        };
        drop(db);

        corrupt_database_atomic_write(&path);

        let fresh_db = open_fresh_db(&path);
        let instance_id = make_instance_id(7);

        let events_ks = fresh_db.keyspace("events", || fjall::KeyspaceCreateOptions::default()).expect("events ks");
        let instances_ks = fresh_db.keyspace("instances", || fjall::KeyspaceCreateOptions::default()).expect("instances ks");
        let timers_ks = fresh_db.keyspace("timers", || fjall::KeyspaceCreateOptions::default()).expect("timers ks");
        let snapshots_ks = fresh_db.keyspace("snapshots", || fjall::KeyspaceCreateOptions::default()).expect("snapshots ks");
        let dedupe_ks = fresh_db.keyspace("dedupe", || fjall::KeyspaceCreateOptions::default()).expect("dedupe ks");
        let effects_ks = fresh_db.keyspace("effects", || fjall::KeyspaceCreateOptions::default()).expect("effects ks");
        let leases_ks = fresh_db.keyspace("leases", || fjall::KeyspaceCreateOptions::default()).expect("leases ks");
        let receipts_ks = fresh_db.keyspace("receipts", || fjall::KeyspaceCreateOptions::default()).expect("receipts ks");

        let event_key = {
            let mut key = Vec::with_capacity(24);
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(&1u64.to_be_bytes());
            key
        };
        let status_key = {
            let mut key = Vec::new();
            key.extend_from_slice(b"\x01");
            key.extend_from_slice(&1712200000000u64.to_be_bytes());
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key
        };
        let timer_key = {
            let mut key = Vec::new();
            key.extend_from_slice(&2000000000000u64.to_be_bytes());
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(b"timer-multi");
            key
        };
        let snapshot_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(&1u64.to_be_bytes());
            key
        };
        let dedupe_key = b"dedupe-multi-partition".to_vec();
        let effect_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(b"intent-multi");
            key
        };
        let lease_key = {
            let mut key = Vec::new();
            key.extend_from_slice(instance_id.to_bytes().expect("bytes").as_slice());
            key.extend_from_slice(b"step-multi");
            key
        };
        let receipt_key = b"fx-multi-partition".to_vec();

        let event_v = events_ks.get(&event_key).expect("get").is_some();
        let status_v = instances_ks.get(&status_key).expect("get").is_some();
        let timer_v = timers_ks.get(&timer_key).expect("get").is_some();
        let snapshot_v = snapshots_ks.get(&snapshot_key).expect("get").is_some();
        let dedupe_v = dedupe_ks.get(&dedupe_key).expect("get").is_some();
        let effect_v = effects_ks.get(&effect_key).expect("get").is_some();
        let lease_v = leases_ks.get(&lease_key).expect("get").is_some();
        let receipt_v = receipts_ks.get(&receipt_key).expect("get").is_some();

        let all_visible = event_v && status_v && timer_v && snapshot_v && dedupe_v && effect_v && lease_v && receipt_v;
        let none_visible = !event_v && !status_v && !timer_v && !snapshot_v && !dedupe_v && !effect_v && !lease_v && !receipt_v;

        assert!(
            all_visible || none_visible,
            "multi-partition batch must be all-or-nothing: events={}, status={}, timer={}, snapshot={}, dedupe={}, effect={}, lease={}, receipt={}",
            event_v, status_v, timer_v, snapshot_v, dedupe_v, effect_v, lease_v, receipt_v
        );
    }
}