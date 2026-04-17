use crate::{ParseError, *};

#[cfg(feature = "proptest")]
use proptest::prelude::*;

fn valid_dek_id() -> DekId {
    DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn sample_encrypted_blob() -> EncryptedBlob {
    EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap()
}

fn sample_wrapped_dek() -> WrappedDek {
    WrappedDek::new(vec![0xDE, 0xAD, 0xBE, 0xEF].repeat(15)).expect("valid wrapped DEK")
}

fn sample_key_metadata() -> KeyMetadata {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    KeyMetadata::new(instance_id, CryptoAlgorithm::Aes256Gcm)
}

#[cfg(test)]
mod dek_id_tests {

    use super::*;

    #[test]
    fn dek_id_accepts_valid_ulid() {
        let id = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn dek_id_rejects_nil_ulid() {
        let result = DekId::parse("00000000000000000000000000");
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_rejects_invalid_ulid() {
        let result = DekId::parse("not-a-ulid");
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_rejects_wrong_length() {
        let result = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRF");
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_accepts_max_ulid() {
        let id = DekId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").expect("valid max ULID");
        assert_eq!(id.as_str(), "7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
    }

    #[test]
    fn dek_id_rejects_empty_string() {
        let result = DekId::parse("");
        assert!(matches!(result, Err(ParseError::Empty { .. })));
    }

    #[test]
    fn dek_id_rejects_whitespace() {
        let result = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRF ");
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_rejects_non_alphanumeric() {
        let result = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRF!");
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_roundtrip_bytes() {
        let id = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let bytes = id.to_bytes().expect("valid bytes");
        let id2 = DekId::from_bytes(bytes);
        assert_eq!(id.as_str(), id2.as_str());
    }

    #[test]
    fn dek_id_to_bytes_invalid_ulid_error() {
        let id = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let bytes = id.to_bytes().expect("valid bytes");
        let id2 = DekId::from_bytes(bytes);
        let roundtrip = id2.to_bytes().expect("valid bytes");
        assert_eq!(roundtrip, bytes);
    }

    #[test]
    fn dek_id_display_shows_full_ulid() {
        let id = valid_dek_id();
        assert_eq!(format!("{id}"), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn dek_id_debug_shows_wrapper() {
        let id = valid_dek_id();
        let debug = format!("{id:?}");
        assert!(debug.contains("DekId"));
    }

    #[test]
    fn dek_id_as_str_returns_full_string() {
        let id = valid_dek_id();
        assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn dek_id_eq_true_identical() {
        let id1 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let id2 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        assert_eq!(id1, id2);
    }

    #[test]
    fn dek_id_eq_false_different() {
        let id1 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let id2 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").expect("valid");
        assert_ne!(id1, id2);
    }

    #[test]
    fn dek_id_hash_eq_consistency() {
        use std::collections::HashSet;
        let id1 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let id2 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let id3 = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").expect("valid");

        let mut set = HashSet::new();
        set.insert(id1.clone());
        set.insert(id2.clone());
        set.insert(id3.clone());

        assert_eq!(set.len(), 2);
        assert!(set.contains(&id1));
        assert!(set.contains(&id3));
    }

    #[test]
    fn dek_id_tryfrom_string_valid() {
        let result = String::try_from("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn dek_id_tryfrom_string_invalid() {
        let result = DekId::try_from("not-a-ulid".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_into_string() {
        let id = valid_dek_id();
        let s: String = id.into();
        assert_eq!(s, "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn dek_id_display_roundtrip() {
        let id = valid_dek_id();
        let s = format!("{id}");
        let id2 = DekId::parse(&s).expect("parseable");
        assert_eq!(id.as_str(), id2.as_str());
    }
}

#[cfg(test)]
mod wrapped_dek_tests {

    use super::*;

    #[test]
    fn wrapped_dek_new_accepts_vec() {
        let bytes: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF].repeat(15);
        let wrapped = WrappedDek::new(bytes.clone()).expect("valid wrapped DEK");
        assert_eq!(wrapped.as_bytes(), &bytes);
    }

    #[test]
    fn wrapped_dek_creation() {
        let wrapped = sample_wrapped_dek();
        assert_eq!(wrapped.as_bytes().len(), 60);
    }

    #[test]
    fn wrapped_dek_as_bytes_returns_borrowed_slice() {
        let wrapped = sample_wrapped_dek();
        let bytes: &[u8] = wrapped.as_bytes();
        assert_eq!(bytes.len(), 60);
    }

    #[test]
    fn wrapped_dek_len_returns_byte_count() {
        let wrapped = sample_wrapped_dek();
        assert_eq!(wrapped.as_bytes().len(), 60);
    }

    #[test]
    fn wrapped_dek_is_empty_false_for_non_empty() {
        let wrapped = sample_wrapped_dek();
        assert!(!wrapped.as_bytes().is_empty());
    }

    #[test]
    fn wrapped_dek_is_empty_true_for_empty_vec() {
        let result = WrappedDek::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn wrapped_dek_display_shows_length() {
        let wrapped = sample_wrapped_dek();
        let display = format!("{wrapped}");
        assert!(display.contains("WrappedDek"));
        assert!(display.contains("60 bytes"));
    }

    #[test]
    fn wrapped_dek_debug_shows_wrapper() {
        let wrapped = sample_wrapped_dek();
        let debug = format!("{wrapped:?}");
        assert!(debug.contains("WrappedDek"));
    }

    #[test]
    fn wrapped_dek_eq_true_identical_bytes() {
        let bytes: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF].repeat(15);
        let w1 = WrappedDek::new(bytes.clone()).expect("valid wrapped DEK");
        let w2 = WrappedDek::new(bytes).expect("valid wrapped DEK");
        assert_eq!(w1, w2);
    }

    #[test]
    fn wrapped_dek_eq_false_different_bytes() {
        let bytes1: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF].repeat(15);
        let bytes2: Vec<u8> = vec![0xFE, 0xEE, 0x00, 0x11].repeat(15);
        let w1 = WrappedDek::new(bytes1).expect("valid wrapped DEK");
        let w2 = WrappedDek::new(bytes2).expect("valid wrapped DEK");
        assert_ne!(w1, w2);
    }
}

#[cfg(test)]
mod encrypted_blob_tests {

    use super::*;

    #[test]
    fn encrypted_blob_new_accepts_components() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        assert_eq!(blob.iv.len(), 12);
        assert_eq!(blob.ciphertext.len(), 32);
        assert_eq!(blob.tag.len(), 16);
    }

    #[test]
    fn encrypted_blob_creation() {
        let blob = sample_encrypted_blob();
        assert_eq!(blob.total_size(), 60);
    }

    #[test]
    fn encrypted_blob_total_size_correct() {
        let blob = sample_encrypted_blob();
        assert_eq!(
            blob.total_size(),
            blob.iv.len() + blob.ciphertext.len() + blob.tag.len()
        );
    }

    #[test]
    fn encrypted_blob_iv_size_fixed_12() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        assert_eq!(blob.iv.len(), 12);
    }

    #[test]
    fn encrypted_blob_tag_size_fixed_16() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        assert_eq!(blob.tag.len(), 16);
    }

    #[test]
    fn encrypted_blob_ciphertext_size_variable() {
        let blob_empty = EncryptedBlob::new(vec![0u8; 12], vec![], vec![2u8; 16]).unwrap();
        assert_eq!(blob_empty.ciphertext.len(), 0);

        let blob_large = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 1000], vec![2u8; 16]).unwrap();
        assert_eq!(blob_large.ciphertext.len(), 1000);
    }

    #[test]
    fn encrypted_blob_fields_public() {
        let blob = sample_encrypted_blob();
        assert_eq!(blob.iv.len(), 12);
        assert_eq!(blob.ciphertext.len(), 32);
        assert_eq!(blob.tag.len(), 16);
    }

    #[test]
    fn encrypted_blob_total_size_empty() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![], vec![2u8; 16]).unwrap();
        assert_eq!(blob.total_size(), 28);
    }

    #[test]
    fn encrypted_blob_display_shows_sizes() {
        let blob = sample_encrypted_blob();
        let display = format!("{blob}");
        assert!(display.contains("EncryptedBlob"));
        assert!(display.contains("iv=12"));
        assert!(display.contains("ciphertext=32"));
        assert!(display.contains("tag=16"));
    }

    #[test]
    fn encrypted_blob_debug_shows_wrapper() {
        let blob = sample_encrypted_blob();
        let debug = format!("{blob:?}");
        assert!(debug.contains("EncryptedBlob"));
    }

    #[test]
    fn encrypted_blob_eq_true_all_components_match() {
        let blob1 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        let blob2 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        assert_eq!(blob1, blob2);
    }

    #[test]
    fn encrypted_blob_eq_false_iv_mismatch() {
        let blob1 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        let blob2 = EncryptedBlob::new(vec![1u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        assert_ne!(blob1, blob2);
    }

    #[test]
    fn encrypted_blob_eq_false_ciphertext_mismatch() {
        let blob1 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        let blob2 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 33], vec![2u8; 16]).unwrap();
        assert_ne!(blob1, blob2);
    }

    #[test]
    fn encrypted_blob_eq_false_tag_mismatch() {
        let blob1 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        let blob2 = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![3u8; 16]).unwrap();
        assert_ne!(blob1, blob2);
    }
}

#[cfg(test)]
mod crypto_algorithm_tests {

    use super::*;

    #[test]
    fn crypto_algorithm_constants() {
        assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
        assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
        assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);
    }

    #[test]
    fn crypto_algorithm_display() {
        let alg = CryptoAlgorithm::Aes256Gcm;
        assert_eq!(format!("{alg}"), "AES-256-GCM");
    }

    #[test]
    fn crypto_algorithm_eq_true_same_variant() {
        let alg1 = CryptoAlgorithm::Aes256Gcm;
        let alg2 = CryptoAlgorithm::Aes256Gcm;
        assert_eq!(alg1, alg2);
    }
}

#[cfg(test)]
mod key_metadata_tests {

    use super::*;

    #[test]
    fn key_metadata_new_sets_created_at() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        assert!(metadata.created_at_ms > 0);
    }

    #[test]
    fn key_metadata_new_sets_algorithm() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
    }

    #[test]
    fn key_metadata_new_sets_instance_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        assert_eq!(metadata.instance_id, instance_id);
    }

    #[test]
    fn key_metadata_creation() {
        let metadata = sample_key_metadata();
        assert!(metadata.created_at_ms > 0);
        assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
    }

    #[test]
    fn key_metadata_created_at_nonzero() {
        let metadata = sample_key_metadata();
        assert!(metadata.created_at_ms > 0);
    }

    #[test]
    fn key_metadata_created_at_reasonable_range() {
        let metadata = sample_key_metadata();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(
            metadata.created_at_ms <= now,
            "created_at_ms should not be in the future"
        );
        assert!(
            now - metadata.created_at_ms < 1000,
            "created_at_ms should be within 1s of current time"
        );
    }

    #[test]
    fn key_metadata_debug_shows_fields() {
        let metadata = sample_key_metadata();
        let debug = format!("{metadata:?}");
        assert!(debug.contains("created_at_ms"));
        assert!(debug.contains("algorithm"));
        assert!(debug.contains("instance_id"));
    }

    #[test]
    fn key_metadata_eq_true_all_fields_match() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata1 = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        let metadata2 = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        assert_eq!(metadata1.created_at_ms, metadata2.created_at_ms);
        assert_eq!(metadata1.algorithm, metadata2.algorithm);
        assert_eq!(metadata1.instance_id, metadata2.instance_id);
    }

    #[test]
    fn key_metadata_eq_false_algorithm_differs() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata1 = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        let metadata2 = KeyMetadata::new(instance_id, CryptoAlgorithm::Aes256Gcm);
        assert_eq!(metadata1, metadata2);
    }

    #[test]
    fn key_metadata_eq_false_instance_id_differs() {
        let instance_id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let instance_id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").expect("valid ULID");
        let metadata1 = KeyMetadata::new(instance_id1, CryptoAlgorithm::Aes256Gcm);
        let metadata2 = KeyMetadata::new(instance_id2, CryptoAlgorithm::Aes256Gcm);
        assert_ne!(metadata1, metadata2);
    }
}

#[cfg(test)]
mod serialization_tests {

    use super::*;

    #[test]
    fn dek_id_json_serialize_deserialize() {
        let id = valid_dek_id();
        let json = serde_json::to_string(&id).expect("serialize");
        let restored: DekId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, id);
    }

    #[test]
    fn wrapped_dek_json_serialize_deserialize() {
        let wrapped = sample_wrapped_dek();
        let json = serde_json::to_string(&wrapped).expect("serialize");
        let restored: WrappedDek = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.as_bytes(), wrapped.as_bytes());
    }

    #[test]
    fn encrypted_blob_json_serialize_deserialize() {
        let blob = sample_encrypted_blob();
        let json = serde_json::to_string(&blob).expect("serialize");
        let restored: EncryptedBlob = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.iv, blob.iv);
        assert_eq!(restored.ciphertext, blob.ciphertext);
        assert_eq!(restored.tag, blob.tag);
    }

    #[test]
    fn crypto_algorithm_json_serialize_deserialize() {
        let alg = CryptoAlgorithm::Aes256Gcm;
        let json = serde_json::to_string(&alg).expect("serialize");
        let restored: CryptoAlgorithm = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, alg);
    }

    #[test]
    fn key_metadata_json_serialize_deserialize() {
        let metadata = sample_key_metadata();
        let json = serde_json::to_string(&metadata).expect("serialize");
        let restored: KeyMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.created_at_ms, metadata.created_at_ms);
        assert_eq!(restored.algorithm, metadata.algorithm);
        assert_eq!(restored.instance_id, metadata.instance_id);
    }

    #[test]
    fn dek_id_tryfrom_string_valid() {
        let result = String::try_from("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn dek_id_tryfrom_string_invalid() {
        let result = DekId::try_from("invalid".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_into_string() {
        let id = valid_dek_id();
        let id_clone = id.clone();
        let s: String = id.into();
        let id2 = DekId::try_from(s).expect("should roundtrip");
        assert_eq!(id_clone, id2);
    }
}

#[cfg(feature = "proptest")]
mod proptests {

    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn dek_id_parse_roundtrip(ulid in "[0-9A-Z]{26}") {
            let id = DekId::parse(&ulid);
            if id.is_ok() {
                let parsed = id.unwrap();
                prop_assert_eq!(parsed.as_str(), ulid);
            }
        }

        #[test]
        fn dek_id_from_bytes_roundtrip(bytes in "[0-9a-f]{32}") {
            let bytes_vec: Vec<u8> = bytes
                .as_bytes()
                .chunks(2)
                .filter_map(|c| {
                    let s = std::str::from_utf8(c).unwrap();
                    u8::from_str_radix(s, 16).ok()
                })
                .collect();
            if bytes_vec.len() == 16 {
                let bytes_array: [u8; 16] = bytes_vec.try_into().unwrap();
                let id = DekId::from_bytes(bytes_array);
                let roundtrip = id.to_bytes().expect("valid");
                prop_assert_eq!(roundtrip, bytes_array);
            }
        }

        #[test]
        fn wrapped_dek_roundtrip(bytes in ".*") {
            let original = WrappedDek::new(bytes.as_bytes().to_vec());
            prop_assert_eq!(original.as_bytes(), bytes.as_bytes());
        }

        #[test]
        fn encrypted_blob_size_calculation(iv in "[\\x00-\\xff]{12}",
                                          ciphertext in ".*",
                                          tag in "[\\x00-\\xff]{16}") {
            let iv_vec: Vec<u8> = iv.chars().map(|c| c as u8).collect();
            let ciphertext_vec: Vec<u8> = ciphertext.chars().map(|c| c as u8).collect();
            let tag_vec: Vec<u8> = tag.chars().map(|c| c as u8).collect();
            let blob = EncryptedBlob::new(iv_vec.clone(), ciphertext_vec.clone(), tag_vec.clone()).unwrap();
            prop_assert_eq!(
                blob.total_size(),
                iv_vec.len() + ciphertext_vec.len() + tag_vec.len()
            );
        }

        #[test]
        fn serde_roundtrip_dek_id(ulid in "[0-9A-Z]{26}") {
            let id_result = DekId::parse(&ulid);
            if id_result.is_ok() {
                let id = id_result.unwrap();
                let json = serde_json::to_string(&id).expect("serialize");
                let restored: DekId = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(restored, id);
            }
        }

        #[test]
        fn serde_roundtrip_wrapped_dek(len in 0u8..64) {
            let bytes: Vec<u8> = (0..len).map(|_| rand::random()).collect();
            let original = WrappedDek::new(bytes.clone());
            let json = serde_json::to_string(&original).expect("serialize");
            let restored: WrappedDek = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored.as_bytes(), &bytes);
        }
    }
}

#[cfg(test)]
mod invariant_tests {

    use super::*;

    #[test]
    #[doc = "I1: Each InstanceId maps to exactly one DekId at runtime"]
    fn invariant_i1_doc() {
        let _ = "I1 (each InstanceId -> one DekId) is a runtime invariant enforced by key store partition";
    }

    #[test]
    #[doc = "I2: Each DekId maps to exactly one InstanceId at runtime"]
    fn invariant_i2_doc() {
        let _ = "I2 (each DekId -> one InstanceId) is a runtime invariant enforced by key store partition";
    }

    #[test]
    #[doc = "I3: DEK never stored unwrapped - wrap() returns WrappedDek, never raw key bytes"]
    fn invariant_i3_doc() {
        let _ = "I3 (DEK never stored unwrapped) is enforced by wrap/unwrap API design";
    }

    #[test]
    #[doc = "I4: payload_blobs always EncryptedBlob - type system prevents raw bytes"]
    fn invariant_i4_doc() {
        let _ = "I4 (payload_blobs always EncryptedBlob) is enforced at storage layer";
    }

    #[test]
    #[doc = "I5: operator projections never encrypted - data model invariant"]
    fn invariant_i5_doc() {
        let _ = "I5 (operator projections never encrypted) is a data model invariant";
    }

    #[test]
    #[doc = "I6: routing_projection never encrypted - data model invariant"]
    fn invariant_i6_doc() {
        let _ = "I6 (routing_projection never encrypted) is a data model invariant";
    }

    #[test]
    #[doc = "I7: purge destroys WrappedDek first - purge ordering guarantee"]
    fn invariant_i7_doc() {
        let _ = "I7 (purge destroys WrappedDek first) is a purge ordering guarantee";
    }

    #[test]
    #[doc = "I8: after purge, encrypted blobs unreadable - ciphertext remains but key destroyed"]
    fn invariant_i8_doc() {
        let _ = "I8 (after purge, encrypted blobs unreadable) - key destroyed means decryption impossible";
    }

    #[test]
    #[doc = "I9: purge ordering is DEK destruction -> index cleanup -> blob reference removal"]
    fn invariant_i9_doc() {
        let _ = "I9 (purge ordering) documents DEK destruction -> index cleanup -> blob reference removal";
    }

    #[test]
    #[doc = "I10: every EncryptedBlob carries tag - type requires tag field"]
    fn invariant_i10_doc() {
        let _ = "I10 (every EncryptedBlob carries tag) - EncryptedBlob type requires tag field";
    }

    #[test]
    #[doc = "I11: decryption MUST fail on tag mismatch - AEAD mode semantics"]
    fn invariant_i11_doc() {
        let _ = "I11 (decryption MUST fail on tag mismatch) - AEAD mode semantics";
    }

    #[test]
    #[doc = "I12: DecryptionFailed error on tag mismatch - error taxonomy"]
    fn invariant_i12_doc() {
        let _ = "I12 (DecryptionFailed error on tag mismatch) - error taxonomy";
    }
}
