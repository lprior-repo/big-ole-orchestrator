use std::sync::Arc;

use super::error::SpscError;
use super::queue::SpscQueue;

#[test]
fn spsc_queue_basic_send_recv() {
    let queue = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, rx) = queue.sender();

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();

    assert_eq!(rx.recv().unwrap(), 1);
    assert_eq!(rx.recv().unwrap(), 2);
    assert_eq!(rx.recv().unwrap(), 3);
}

#[test]
fn spsc_queue_full_error() {
    let queue = Arc::new(SpscQueue::<i32>::new(2));
    let (tx, _rx) = queue.sender();

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    assert_eq!(tx.send(3), Err(SpscError::Full));
}

#[test]
fn spsc_queue_empty_error() {
    let queue = Arc::new(SpscQueue::<i32>::new(8));
    let (_tx, rx) = queue.sender();

    assert_eq!(rx.recv(), Err(SpscError::Empty));
}

#[test]
fn spsc_queue_len() {
    let queue = Arc::new(SpscQueue::<i32>::new(8));
    let (tx, rx) = queue.sender();

    assert_eq!(queue.len(), 0);
    tx.send(1).unwrap();
    assert_eq!(queue.len(), 1);
    tx.send(2).unwrap();
    assert_eq!(queue.len(), 2);
    rx.recv().unwrap();
    assert_eq!(queue.len(), 1);
    rx.recv().unwrap();
    assert_eq!(queue.len(), 0);
}

#[test]
fn spsc_queue_wraparound() {
    let queue = Arc::new(SpscQueue::<i32>::new(4));
    let (tx, rx) = queue.sender();

    for i in 0..4 {
        tx.send(i).unwrap();
    }
    assert_eq!(queue.len(), 4);

    rx.recv().unwrap();
    rx.recv().unwrap();
    assert_eq!(queue.len(), 2);

    tx.send(100).unwrap();
    tx.send(101).unwrap();
    assert_eq!(queue.len(), 4);

    for i in 0..4 {
        let val = rx.recv().unwrap();
        if i < 2 {
            assert_eq!(val, i + 2);
        } else {
            assert_eq!(val, i - 2 + 100);
        }
    }
}

#[test]
fn spsc_queue_debug() {
    let queue = SpscQueue::<i32>::new(8);
    assert!(format!("{:?}", queue).contains("SpscQueue"));
}
