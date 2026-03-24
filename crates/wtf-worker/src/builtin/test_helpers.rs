use bytes::Bytes;
use wtf_common::{ActivityId, InstanceId, NamespaceId, RetryPolicy};

use crate::queue::ActivityTask;

/// Build a minimal [`ActivityTask`] for testing.
pub(crate) fn make_task(activity_type: &str, payload: &[u8]) -> ActivityTask {
    ActivityTask {
        activity_id: ActivityId::new("act-test"),
        activity_type: activity_type.to_owned(),
        payload: Bytes::copy_from_slice(payload),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-test"),
        attempt: 1,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    }
}
