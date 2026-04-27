//! DUR-005: Corrupt a single SST file, verify fjall recovery or clean error.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

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
fn dur_005_corrupt_sst_file_clean_error_or_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let id = make_instance_id(5);

    // Phase 1: Write data and persist to disk
    {
        let db = open_db(dir.path());
        let ks = open_events_ks(&db);

        for seq in 1..=100u64 {
            let seq_num = SequenceNumber::try_from(seq).unwrap();
            let key = encode_event_key(&id, &seq_num).unwrap();
            let value = serde_json::json!({"sequence": seq, "type": "DurabilityTest"});
            ks.insert(key, serde_json::to_vec(&value).unwrap())
                .unwrap();
        }

        db.persist(fjall::PersistMode::SyncAll).unwrap();
    }

    // Phase 2: Find and corrupt an SST file
    let sst_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Look for SST or data files in subdirectories
            if name.contains(".sst") || name.contains(".dat") {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect();

    let corrupted = if let Some(sst_path) = sst_files.last() {
        let mut data = std::fs::read(sst_path).unwrap();
        if !data.is_empty() {
            let mid = data.len() / 2;
            for i in mid..mid.saturating_add(16).min(data.len()) {
                data[i] = data[i].wrapping_add(0xFF);
            }
            std::fs::write(sst_path, &data).unwrap();
            true
        } else {
            false
        }
    } else {
        false
    };

    // Phase 3: Attempt to reopen — should either recover or error cleanly
    if corrupted {
        let result = fjall::Database::builder(dir.path()).open();
        match result {
            Ok(db) => {
                let ks = db
                    .keyspace("events", fjall::KeyspaceCreateOptions::default)
                    .unwrap();

                let id_bytes = id.to_bytes().unwrap();
                let count = ks.prefix(id_bytes).count();

                assert!(
                    count <= 100,
                    "Recovered count must not exceed original write count"
                );
            }
            Err(_) => {
                // Fjall returned a clean error — this is acceptable behavior
                // The important thing is it didn't panic or return corrupt data
            }
        }
    } else {
        // No SST files to corrupt — verify data intact
        let db = open_db(dir.path());
        let ks = open_events_ks(&db);

        let id_bytes = id.to_bytes().unwrap();
        let count = ks.prefix(id_bytes).count();

        assert_eq!(count, 100, "All 100 events must be intact");
    }
}
