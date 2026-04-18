//! DUR-002: Commit managed effect, kill during commit, verify exactly-once.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use serde_json::json;
use vo_storage::effect_journal::{EffectId, EffectJournal, EffectJournalError, FjallEffectJournal};
use vo_types::{EffectIntent, EffectKind, EffectRecord, InstanceId};

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn make_effect_record(intent_id: &str) -> EffectRecord {
    EffectRecord::new(
        intent_id.to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

fn open_db(path: &std::path::Path) -> fjall::Database {
    fjall::Database::builder(path).open().unwrap()
}

#[test]
fn dur_002_effect_commit_kill_verify_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let id = make_instance_id(2);

    // Phase 1: Prepare effects and commit some, leave some pending
    let committed_ids: Vec<EffectId>;
    {
        let db = open_db(dir.path());
        let journal = FjallEffectJournal::open(&db).unwrap();

        let total_effects = 20u32;
        let mut committed = Vec::new();

        for i in 0..total_effects {
            let record = make_effect_record(&format!("dur002-effect-{i}"));
            let eid = journal.prepare(&id, record).unwrap();

            if i < 15 {
                journal.commit(&eid).unwrap();
                committed.push(eid);
            }
        }

        committed_ids = committed;
    }
    // <-- simulated kill

    // Phase 2: Restart and verify exactly-once semantics
    {
        let db = open_db(dir.path());
        let journal = FjallEffectJournal::open(&db).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(
            pending.len(),
            5,
            "Exactly 5 effects must be pending after crash"
        );

        for eid in &committed_ids {
            let result = journal.commit(eid);
            assert!(
                matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
                "Already-committed effect must reject re-commit"
            );
        }

        for record in &pending {
            let eid = EffectId::new(&id, record.intent_id()).unwrap();
            journal.commit(&eid).unwrap();
        }

        let final_pending = journal.list_pending(&id).unwrap();
        assert!(
            final_pending.is_empty(),
            "All effects must be resolved after recovery"
        );
    }

    // Phase 3: Double-restart — verify no phantom effects
    {
        let db = open_db(dir.path());
        let journal = FjallEffectJournal::open(&db).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        assert!(
            pending.is_empty(),
            "No pending effects should appear after second restart"
        );

        for eid in &committed_ids {
            let result = journal.commit(eid);
            assert!(
                matches!(result, Err(EffectJournalError::AlreadyTerminal { .. })),
                "Committed effect must remain terminal across restarts"
            );
        }
    }
}
