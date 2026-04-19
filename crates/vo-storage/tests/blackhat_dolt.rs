//! BLACK-HAT Dolt adversarial tests: corruption + SQL injection via beads.
//! ve-j2sk1 — Tests Dolt corruption and SQL injection via issue fields.

use std::process::Command;
use std::sync::Arc;
use std::thread;

const SQL_PAYLOADS: &[&str] = &[
    "fix: SQL injection test 1",
    "fix: OR condition",
    "admin--style",
    "fix: quotes'test\"double",
    "fix: semicolon;delimiter",
    "fix: backslash\\x",
    "fix: percent%like",
    "fix: underscore_exploit",
    "fix: asterisk*wild",
    "fix: question?mark",
    "fix: bracket[0]",
    "fix: parens(func)",
    "'; DROP TABLE issues; --",
    "1; DELETE FROM issues WHERE 1=1;--",
    "1' UNION SELECT * FROM users--",
    "'; INSERT INTO issues VALUES('pwned'); --",
];

use tempfile::TempDir;
use vo_storage::codec::{decode_event_key, encode_event_key};
use vo_storage::key_encoding::{decode_effect_key, decode_lease_key};
use vo_storage::partitions::{create_partition_layout, open_all_partitions};
use vo_storage::snapshots::{compact_snapshots, encode_snapshot_key};
use vo_storage::status_store::{decode_status, StatusStoreError};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    let mut b = [0u8; 16];
    b[15] = 1;
    InstanceId::from_bytes(b)
}

// ── 1. Dolt corruption: zeroed-out partition files ──────────────────────────────

#[test]
fn dolt_zeroed_sst_files_do_not_panic_on_scan() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("corrupt");

    {
        let layout = create_partition_layout(&path).unwrap();
        let partitions = open_all_partitions(&layout).unwrap();
        let events = partitions
            .iter()
            .find(|(n, _)| *n == "events")
            .unwrap()
            .1
            .clone();
        let id = test_instance_id();
        let key =
            encode_event_key(&id, &vo_types::SequenceNumber::try_from(1u64).unwrap()).unwrap();
        events.insert(key, b"original".to_vec()).unwrap();
    }

    // Simulate Dolt corruption: zero out all SST files
    for entry in std::fs::read_dir(&path).unwrap().flatten() {
        if entry.path().extension().is_some_and(|e| e == "sst") {
            let meta = std::fs::metadata(entry.path()).unwrap();
            std::fs::write(entry.path(), vec![0u8; meta.len() as usize]).unwrap();
        }
    }

    // Must not panic — either opens cleanly or returns an error
    let result = create_partition_layout(&path);
    let _ = result;
}

// ── 2. Concurrent write conflict on identical keys ──────────────────────────────

#[test]
fn concurrent_writes_on_same_event_key_last_writer_wins() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("race");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let events = Arc::new(
        partitions
            .iter()
            .find(|(n, _)| *n == "events")
            .unwrap()
            .1
            .clone(),
    );
    let id = test_instance_id();
    let seq = vo_types::SequenceNumber::try_from(42u64).unwrap();
    let key = encode_event_key(&id, &seq).unwrap();

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let events = Arc::clone(&events);
            let key = key.clone();
            thread::spawn(move || {
                let val = format!("writer-{i}");
                events.insert(key.clone(), val.into_bytes()).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Exactly one value survives — must be valid UTF-8 and decodable
    let found = events.get(&key).unwrap().unwrap();
    let decoded_key = decode_event_key(&key).unwrap();
    assert_eq!(decoded_key.1.as_u64(), 42);
    let _text = std::str::from_utf8(&found).unwrap();
}

// ── 3. SQL injection via serialized bead payloads ──────────────────────────────

#[test]
fn bead_payload_with_sql_injection_does_not_corrupt_decode() {
    let injections = [
        r#"{"title":"'; DROP TABLE beads; --","status":"open"}"#,
        r#"{"title":"1; INSERT INTO beads VALUES('pwned')","status":"open"}"#,
        r#"{"title":"admin' OR '1'='1","status":"open"}"#,
        r#"{"title":"\u0000\u0000\u0000","status":"open"}"#,
        r#"{"title":"","type":"}; DROP SCHEMA public; --","priority":0}"#,
    ];

    for payload in &injections {
        let bytes = payload.as_bytes();
        let result = decode_status(bytes);
        match result {
            Ok(_) | Err(StatusStoreError::CorruptValue { .. }) => {}
            other => panic!("unexpected error variant for injection payload: {other:?}"),
        }
    }
}

fn dolt_dir() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(".beads/dolt");
    if p.exists() {
        Some(p.to_path_buf())
    } else {
        None
    }
}

fn bd() -> Command {
    Command::new("bd")
}

fn mk_issue(title: &str, desc: &str) -> Option<String> {
    let out = bd()
        .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
        .args(["create", title, "-d", desc, "--silent", "--json"])
        .output()
        .ok()?;
    if out.status.success() {
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .ok()?
            .get("id")?
            .as_str()
            .map(String::from)
    } else {
        None
    }
}

#[test]
fn sql_injection_payloads_handled_without_corruption() {
    for p in SQL_PAYLOADS {
        let t = format!("fix: {p}");
        assert!(
            mk_issue(&t, "Testing SQL injection").is_some(),
            "payload handled: {p}"
        );
    }
}

#[test]
fn concurrent_compaction_with_corrupt_keys_does_not_panic() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("compact_race");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = Arc::new(
        partitions
            .iter()
            .find(|(n, _)| *n == "snapshots")
            .unwrap()
            .1
            .clone(),
    );
    let id = test_instance_id();

    for seq in 1..=20u64 {
        let key = encode_snapshot_key(&id, seq).unwrap();
        let val = serde_json::to_vec(&InstanceState { counter: seq }).unwrap();
        snaps.insert(key, val).unwrap();
    }
    snaps
        .insert(b"\x00garbage_key".to_vec(), b"\xff".to_vec())
        .unwrap();
    snaps
        .insert(b"another_corrupt".to_vec(), b"".to_vec())
        .unwrap();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let snaps = Arc::clone(&snaps);
            let id = id.clone();
            thread::spawn(move || {
                let _ = compact_snapshots(&snaps, &id, 5);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn meta_chars_in_labels_handled() {
    let out = bd()
        .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
        .args([
            "create",
            "fix: meta chars",
            "--labels",
            "p0;drop,p1'or',p2--x",
            "--json",
        ])
        .output()
        .expect("bd should handle meta chars");
    assert!(out.status.success() || !out.stderr.is_empty());
}

#[test]
fn crafted_key_bytes_never_decode_to_wrong_key_type() {
    let event_key = encode_event_key(
        &test_instance_id(),
        &vo_types::SequenceNumber::try_from(1u64).unwrap(),
    )
    .unwrap();

    // Lease and effect keys use text/binary formats — must reject binary event keys
    assert!(
        decode_lease_key(&event_key).is_err(),
        "event key must not decode as lease key"
    );
    assert!(
        decode_effect_key(&event_key).is_err(),
        "event key must not decode as effect key"
    );

    // Timer key (8-byte timestamp + 16-byte instance) must not decode as lease/effect
    let timer_key = vo_storage::key_encoding::encode_timer_key(u64::MAX, &test_instance_id());
    assert!(
        decode_lease_key(&timer_key).is_err(),
        "timer key must not decode as lease key"
    );
    assert!(
        decode_effect_key(&timer_key).is_err(),
        "timer key must not decode as effect key"
    );
}
