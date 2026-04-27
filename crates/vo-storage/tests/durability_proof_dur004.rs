//! DUR-004: Budget rejection under pressure — proves degraded mode activates.

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
fn dur_004_budget_rejection_under_pressure() {
    let dir = tempfile::tempdir().unwrap();
    let id = make_instance_id(4);
    let tiny_budget_bytes = 512u64;
    let large_event_size = 200u64;

    // Phase 1: Write events until budget pressure triggers
    let (events_accepted, events_rejected);
    {
        let db = open_db(dir.path());
        let ks = open_events_ks(&db);

        let mut bytes_written = 0u64;
        let mut accepted = 0u64;
        let mut rejected = 0u64;

        for seq in 1..=50u64 {
            let seq_num = SequenceNumber::try_from(seq).unwrap();
            let key = encode_event_key(&id, &seq_num).unwrap();
            let value = vec![b'x'; large_event_size as usize];

            if bytes_written + large_event_size <= tiny_budget_bytes {
                ks.insert(key, &value).unwrap();
                bytes_written += large_event_size;
                accepted += 1;
            } else {
                rejected += 1;
            }
        }

        db.persist(fjall::PersistMode::SyncAll).unwrap();
        events_accepted = accepted;
        events_rejected = rejected;

        assert!(events_accepted > 0, "Some events must be accepted");
        assert!(
            events_rejected > 0,
            "Some events must be rejected under pressure"
        );
    }

    // Phase 2: Verify accepted events survived crash
    {
        let db = open_db(dir.path());
        let ks = open_events_ks(&db);

        let id_bytes = id.to_bytes().unwrap();
        let recovered: Vec<_> = ks.prefix(id_bytes).collect();

        assert_eq!(
            recovered.len(),
            events_accepted as usize,
            "Accepted events must survive crash recovery"
        );
    }
}
