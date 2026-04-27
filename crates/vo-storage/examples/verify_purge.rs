use vo_types::InstanceId;

fn main() {
    let fjall_path = "/home/lewis/.gemini/tmp/veloxide/fjall";
    let keyspace = fjall::Database::builder(fjall_path)
        .open()
        .expect("failed to open keyspace");

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let id_bytes = instance_id.to_bytes().unwrap();

    let events_p = keyspace
        .keyspace("events", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let event_count = events_p.prefix(id_bytes).count();

    let instances_p = keyspace
        .keyspace("instances", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let instance_count = instances_p.prefix([]).count();

    if event_count == 0 && instance_count == 0 {
        println!("Verification successful: instance 01H5JYV4XHGSR2F8KZ9BWNRFMA is purged.");
    } else {
        eprintln!("Verification FAILED: events={event_count}, instances={instance_count}");
        std::process::exit(1);
    }
}
