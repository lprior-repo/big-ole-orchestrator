use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_due_timers, timer_delete, timer_set,
    MockStorage,
};

#[test]
fn rq_timer_delete_nonexistent_succeeds() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_delete(&mut storage, &instance_id, timer_id, 1000);
    assert!(result.is_ok(), "Deleting non-existent timer should succeed");
}

#[test]
fn rq_timer_delete_propagates_storage_failure() {
    let mut storage = MockStorage::with_fail("delete");
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_delete(&mut storage, &instance_id, timer_id, 1000);
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::Storage),
        "Storage failure should be propagated"
    );
}

#[test]
fn rq_timer_delete_removes_timer() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 3000);
    assert_eq!(result.unwrap().len(), 1);

    timer_delete(&mut storage, &instance_id, timer_id, 2000).unwrap();

    let result = scan_due_timers(&storage, &instance_id, 3000);
    assert!(result.unwrap().is_empty());
}