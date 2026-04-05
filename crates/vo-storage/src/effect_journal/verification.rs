use super::*;

/// K-01: Verify EffectId::new rejects empty intent_id.
#[kani::proof]
fn verify_effect_id_rejects_empty_intent_id() {
    let iid = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&iid, "");
    assert_eq!(result, Err(EffectJournalError::InvalidArgument));
}

/// K-02: Verify encode/decode key round-trip for any valid UTF-8 string.
#[kani::proof]
fn verify_encode_decode_key_roundtrip() {
    let eid = EffectId("verify-intent".to_string());
    let bytes = encode_effect_key(&eid);
    let recovered = decode_effect_key(&bytes);
    assert_eq!(recovered, Ok(eid));
}
