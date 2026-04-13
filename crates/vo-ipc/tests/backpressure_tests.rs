use std::sync::Arc;
use vo_ipc::spsc::{SpscError, SpscQueue};

#[test]
fn spsc_capacity_rounds_to_power_of_two() {
    assert_eq!(SpscQueue::<u8>::new(5).capacity(), 8);
    assert_eq!(SpscQueue::<u8>::new(1).capacity(), 1);
    assert_eq!(SpscQueue::<u8>::new(3).capacity(), 4);
    assert_eq!(SpscQueue::<u8>::new(16).capacity(), 16);
    assert_eq!(SpscQueue::<u8>::new(17).capacity(), 32);
}

#[test]
fn spsc_send_recv_fifo_order() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, rx) = q.sender();

    for i in 0..5 {
        tx.send(i).unwrap();
    }

    for i in 0..5 {
        assert_eq!(rx.recv().unwrap(), i);
    }
}

#[test]
fn spsc_full_then_drain_then_refill() {
    let q = Arc::new(SpscQueue::<i32>::new(4));
    let (tx, rx) = q.sender();

    for i in 0..4 {
        tx.send(i).unwrap();
    }
    assert!(tx.is_full());

    for i in 0..4 {
        assert_eq!(rx.recv().unwrap(), i);
    }
    assert!(rx.is_empty());

    for i in 10..14 {
        tx.send(i).unwrap();
    }

    for i in 10..14 {
        assert_eq!(rx.recv().unwrap(), i);
    }
}

#[test]
fn spsc_multiple_wraparounds() {
    let q = Arc::new(SpscQueue::<i32>::new(4));
    let (tx, rx) = q.sender();

    for round in 0..20 {
        for i in 0..4 {
            tx.send(round * 10 + i).unwrap();
        }
        for i in 0..4 {
            assert_eq!(rx.recv().unwrap(), round * 10 + i);
        }
        assert!(q.is_empty());
    }
}

#[test]
fn spsc_sender_full_after_capacity() {
    let q = Arc::new(SpscQueue::<i32>::new(2));
    let (tx, _rx) = q.sender();

    assert!(!tx.is_full());
    tx.send(1).unwrap();
    assert!(!tx.is_full());
    tx.send(2).unwrap();
    assert!(tx.is_full());
}

#[test]
fn spsc_receiver_empty_initially() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (_tx, rx) = q.sender();
    assert!(rx.is_empty());
}

#[test]
fn spsc_receiver_not_empty_after_send() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, rx) = q.sender();
    tx.send(42).unwrap();
    assert!(!rx.is_empty());
}

#[test]
fn spsc_len_tracks_send_recv() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, rx) = q.sender();

    assert_eq!(q.len(), 0);
    tx.send(1).unwrap();
    assert_eq!(q.len(), 1);
    tx.send(2).unwrap();
    assert_eq!(q.len(), 2);
    rx.recv().unwrap();
    assert_eq!(q.len(), 1);
    rx.recv().unwrap();
    assert_eq!(q.len(), 0);
}

#[test]
fn spsc_error_display() {
    assert_eq!(SpscError::Full.to_string(), "queue is full");
    assert_eq!(SpscError::Empty.to_string(), "queue is empty");
}

#[test]
fn spsc_with_string_values() {
    let q = Arc::new(SpscQueue::<String>::new(4));
    let (tx, rx) = q.sender();

    tx.send("hello".to_string()).unwrap();
    tx.send("world".to_string()).unwrap();

    assert_eq!(rx.recv().unwrap(), "hello");
    assert_eq!(rx.recv().unwrap(), "world");
}

#[test]
fn spsc_with_vec_values() {
    let q = Arc::new(SpscQueue::<Vec<i32>>::new(4));
    let (tx, rx) = q.sender();

    tx.send(vec![1, 2, 3]).unwrap();
    tx.send(vec![4, 5, 6]).unwrap();

    assert_eq!(rx.recv().unwrap(), vec![1, 2, 3]);
    assert_eq!(rx.recv().unwrap(), vec![4, 5, 6]);
}

#[test]
fn spsc_single_element_queue() {
    let q = Arc::new(SpscQueue::<i32>::new(1));
    let (tx, rx) = q.sender();

    tx.send(42).unwrap();
    assert!(tx.is_full());
    assert_eq!(tx.send(99), Err(SpscError::Full));

    assert_eq!(rx.recv().unwrap(), 42);
    assert!(rx.is_empty());

    tx.send(100).unwrap();
    assert_eq!(rx.recv().unwrap(), 100);
}

#[test]
fn spsc_large_capacity_queue() {
    let q = Arc::new(SpscQueue::<i32>::new(1024));
    let (tx, rx) = q.sender();

    for i in 0..1024 {
        tx.send(i).unwrap();
    }
    assert!(tx.is_full());
    assert_eq!(tx.send(9999), Err(SpscError::Full));

    for i in 0..1024 {
        assert_eq!(rx.recv().unwrap(), i);
    }
    assert!(rx.is_empty());
}

#[test]
fn spsc_debug_format() {
    let q = SpscQueue::<i32>::new(8);
    let debug = format!("{:?}", q);
    assert!(debug.contains("SpscQueue"));
    assert!(debug.contains("capacity"));
    assert!(debug.contains("len"));
}

#[test]
fn spsc_sender_debug_format() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, _) = q.sender();
    let debug = format!("{:?}", tx);
    assert!(debug.contains("Sender"));
}

#[test]
fn spsc_receiver_debug_format() {
    let q = Arc::new(SpscQueue::<i32>::new(8));
    let (_, rx) = q.sender();
    let debug = format!("{:?}", rx);
    assert!(debug.contains("Receiver"));
}
