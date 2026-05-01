//! Key encoding utilities for the event store.

use vo_types::InstanceId;

const META_SEQ_KEY_PREFIX: u8 = 0x00;

pub(crate) fn encode_sequence_key(instance_id: &InstanceId) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 16);
    key.push(META_SEQ_KEY_PREFIX);
    key.extend_from_slice(&instance_id.to_bytes().unwrap_or([0u8; 16]));
    key
}
