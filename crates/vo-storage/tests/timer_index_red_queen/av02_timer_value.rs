use vo_storage::codec::StorageError;
use vo_storage::timer_index::TimerValue;

#[test]
fn rq_timer_value_rejects_zero_duration() {
    let result = TimerValue::new(0);
    assert!(result.is_err(), "Zero duration should be rejected");
    match result {
        Err(StorageError::InvalidArgument) => {}
        Err(e) => panic!("Expected InvalidArgument, got something else: {:?}", e),
        Ok(_) => panic!("Expected error, got Ok"),
    }

    let result = TimerValue::new(1);
    assert!(result.is_ok(), "Non-zero duration should be accepted");
}
