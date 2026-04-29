use sha2::Digest;
use vo_storage::blob_store::ContentAddress;

#[test]
fn red_queen_concurrent_publish_same_content_deduplication() {
    let store = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        ContentAddress,
        Vec<u8>,
    >::new()));

    let data = b"identical content for deduplication".to_vec();
    let num_threads = 16;
    let barrier = std::sync::Barrier::new(num_threads);
    let data_clone = data.clone();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Barrier::new(num_threads);
            std::thread::spawn({
                let value = data_clone.clone();
                move || {
                    barrier.wait();
                    let mut guard = store.lock().unwrap();
                    let content_addr =
                        ContentAddress::from_bytes(&sha2::Sha256::digest(&value).into());
                    if !guard.contains_key(&content_addr) {
                        guard.insert(content_addr.clone(), value.clone());
                    }
                    guard.len()
                }
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let unique_inserts = results.iter().min().unwrap();
    let max_results = results.iter().max().unwrap();
    assert_eq!(
        *unique_inserts, 1,
        "BUG: Only one thread should have inserted the content"
    );
    assert_eq!(
        *max_results, 1,
        "BUG: Final count must be exactly 1 (dedup)"
    );
}

#[test]
fn red_queen_concurrent_publish_different_content_no_dedup() {
    let store = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        ContentAddress,
        Vec<u8>,
    >::new()));

    let num_threads = 16;
    let barrier = std::sync::Barrier::new(num_threads);

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Barrier::new(num_threads);
            std::thread::spawn(move || {
                barrier.wait();
                let data = format!("unique content {}", i).into_bytes();
                let content_addr = ContentAddress::from_bytes(&sha2::Sha256::digest(&data).into());
                let mut guard = store.lock().unwrap();
                guard.insert(content_addr, data);
                guard.len()
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let final_count = *results.iter().max().unwrap();
    assert_eq!(
        final_count, num_threads as usize,
        "BUG: Each unique content must be stored separately"
    );
}

#[test]
fn red_queen_concurrent_ref_count_increment_thread_safety() {
    let counter = std::sync::Arc::new(std::sync::Mutex::new(0u64));
    let num_threads = 16;
    let iterations = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let counter = std::sync::Arc::clone(&counter);
            std::thread::spawn(move || {
                for _ in 0..iterations {
                    let mut guard = counter.lock().unwrap();
                    *guard += 1;
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let final_count = *counter.lock().unwrap();
    assert_eq!(
        final_count,
        (num_threads * iterations) as u64,
        "BUG: All increments must be accounted for"
    );
}