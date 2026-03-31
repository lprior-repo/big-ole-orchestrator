use vo_types::*;
use wtf_types::*;

fn main() {
    println!("--- Testing Inconsistent Validation ---");

    // 1. Consecutive separators in WorkflowName
    let name = "a--b";
    let vo_res = vo_types::WorkflowName::parse(name);
    let wtf_res = wtf_types::WorkflowName::parse(name);
    println!("WorkflowName 'a--b': vo={:?}, wtf={:?}", vo_res, wtf_res);

    let name = "a__b";
    let vo_res = vo_types::WorkflowName::parse(name);
    let wtf_res = wtf_types::WorkflowName::parse(name);
    println!("WorkflowName 'a__b': vo={:?}, wtf={:?}", vo_res, wtf_res);

    // 2. Leading underscore in WorkflowName
    let name = "_start";
    let vo_res = vo_types::WorkflowName::parse(name);
    let wtf_res = wtf_types::WorkflowName::parse(name);
    println!("WorkflowName '_start': vo={:?}, wtf={:?}", vo_res, wtf_res);

    // 3. ULID validation
    // Max ULID starts with 7. 8... is invalid.
    let ulid_too_large = "8ZZZZZZZZZZZZZZZZZZZZZZZZZ";
    let vo_ulid = vo_types::InstanceId::parse(ulid_too_large);
    let wtf_ulid = wtf_types::InstanceId::parse(ulid_too_large);
    println!("InstanceId '8...': vo={:?}, wtf={:?}", vo_ulid, wtf_ulid);

    // 4. RetryPolicy Infinity
    let vo_retry = vo_types::RetryPolicy::new(1, 0, f32::INFINITY);
    let wtf_retry = wtf_types::RetryPolicy::new(1, 0, f32::INFINITY);
    println!("RetryPolicy INFINITY: vo={:?}, wtf={:?}", vo_retry, wtf_retry);

    // 5. RetryPolicy direct construction
    let vo_policy = vo_types::RetryPolicy {
        max_attempts: 0,
        backoff_ms: 0,
        backoff_multiplier: 0.0,
    };
    println!("vo RetryPolicy direct (max_attempts=0): {:?}", vo_policy);
}
