use vo_storage::instance_index::instance_index_upsert;
use vo_storage::codec::encode_event_key;
use vo_types::{InstanceId, InstanceStatus, TimestampMs, SequenceNumber};

fn main() {
    let fjall_path = "/home/lewis/.gemini/tmp/veloxide/fjall";
    let _ = std::fs::remove_dir_all(fjall_path);
    let _ = std::fs::create_dir_all(fjall_path);
    let keyspace = fjall::Config::new(fjall_path).open().expect("failed to open keyspace");
    
    let terminal_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("failed to parse ID");
    let ts = TimestampMs::try_from(1000u64).unwrap();
    
    // Seed terminal
    let _ = keyspace.open_partition("instances", Default::default());
    instance_index_upsert(&keyspace, &terminal_id, InstanceStatus::Completed, ts, None).unwrap();
    let events_p = keyspace.open_partition("events", Default::default()).unwrap();
    for i in 1..=5 {
        let seq = SequenceNumber::try_from(i as u64).unwrap();
        let key = encode_event_key(&terminal_id, &seq).unwrap();
        events_p.insert(key, b"event-data").unwrap();
    }
    
    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();
    println!("Seeded terminal: 01H5JYV4XHGSR2F8KZ9BWNRFMA (5 events)");
}
