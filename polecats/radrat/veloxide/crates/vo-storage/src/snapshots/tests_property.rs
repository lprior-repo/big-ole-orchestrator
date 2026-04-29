#![allow(clippy::unwrap_used)]

use super::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn encode_decode_roundtrip(
        id_bytes in any::<[u8; 16]>(),
        seq in any::<u64>()
    ) {
        let id = InstanceId::from_bytes(id_bytes);
        let encoded = encode_snapshot_key(&id, seq).unwrap();
        let decoded = decode_snapshot_key(&encoded).unwrap();
        prop_assert_eq!(decoded.0, id);
        prop_assert_eq!(decoded.1, seq);
    }

    #[test]
    fn key_ordering_preserves_sequence(
        id_bytes in any::<[u8; 16]>(),
        seq1 in any::<u64>(),
        seq2 in any::<u64>()
    ) {
        prop_assume!(seq1 < seq2);
        let id = InstanceId::from_bytes(id_bytes);
        let encoded1 = encode_snapshot_key(&id, seq1).unwrap();
        let encoded2 = encode_snapshot_key(&id, seq2).unwrap();
        prop_assert!(encoded1 < encoded2);
    }
}
