pub(crate) use crate::signal_buffer::{
    apply_policy, can_buffer, BufferResult, BufferedSignal, SignalBuffer, SignalBufferConfig,
};
pub(crate) use crate::WaitKey;
pub(crate) use vo_types::InstanceId;
pub(crate) use vo_types::{BufferPolicy, SignalDelivery};
pub(crate) use vo_types::SignalName;

pub(crate) fn instance_id_a() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

pub(crate) fn instance_id_b() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap()
}

pub(crate) fn wait_key_approval() -> WaitKey {
    WaitKey::parse("approval").unwrap()
}

pub(crate) fn wait_key_notif() -> WaitKey {
    WaitKey::parse("notification").unwrap()
}

pub(crate) fn make_signal(signal_id: &str) -> BufferedSignal {
    BufferedSignal::new(
        signal_id.to_string(),
        crate::SignalPayload::empty(),
        vo_types::TimestampMs::now(),
    )
}

pub(crate) fn default_config() -> SignalBufferConfig {
    SignalBufferConfig::default()
}
