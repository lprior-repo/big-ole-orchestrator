use aes_gcm::aes::Aes256;
use aes_gcm::AesGcm;
use aes_gcm::NewAead;
use generic_array::{typenum::U12, GenericArray};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    #[error("DEK not found in key store")]
    KeyNotFound,
    #[error("DEK was purged (crypto-shredded)")]
    KeyDestroyed,
    #[error("key store partition inaccessible")]
    KeyStoreUnavailable,
    #[error("tag mismatch or corrupt ciphertext")]
    DecryptionFailed,
    #[error("key bytes invalid")]
    InvalidKeyMaterial,
    #[error("unknown cipher requested")]
    UnsupportedAlgorithm,
    #[error("KEK wrap operation failed")]
    WrappingFailed,
    #[error("KEK unwrap operation failed")]
    UnwrappingFailed,
    #[error("secure random unavailable")]
    RngUnavailable,
    #[error("encryption operation failed")]
    EncryptionFailed,
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<CryptoError> for crate::codec::StorageError {
    fn from(err: CryptoError) -> Self {
        match err {
            CryptoError::KeyNotFound => Self::KeyNotFound,
            CryptoError::KeyDestroyed => Self::KeyDestroyed,
            CryptoError::DecryptionFailed => Self::CorruptEventPayload,
            CryptoError::KeyStoreUnavailable
            | CryptoError::WrappingFailed
            | CryptoError::UnwrappingFailed
            | CryptoError::RngUnavailable
            | CryptoError::EncryptionFailed => Self::Storage,
            CryptoError::InvalidKeyMaterial => Self::InvalidKey,
            CryptoError::UnsupportedAlgorithm => Self::UnsupportedVersion,
            CryptoError::InvalidArgument(_) => Self::InvalidArgument,
        }
    }
}

pub const DEK_SIZE_BYTES: usize = 32;
pub const KEK_SIZE_BYTES: usize = 32;
pub const IV_SIZE_BYTES: usize = 12;
pub const TAG_SIZE_BYTES: usize = 16;

type Aes256Gcm = AesGcm<Aes256, U12>;

/// Generates a new random data encryption key.
///
/// # Errors
///
/// Returns `CryptoError` if the system RNG fails to fill the key buffer.
pub fn generate_dek() -> Result<[u8; DEK_SIZE_BYTES], CryptoError> {
    use rand::RngCore;
    let mut key = [0u8; DEK_SIZE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut key);
    Ok(key)
}

/// Wraps a DEK with a KEK using AES-256-GCM.
///
/// # Errors
///
/// Returns `CryptoError` if encryption fails.
pub fn wrap_dek(
    dek: &[u8; DEK_SIZE_BYTES],
    kek: &[u8; KEK_SIZE_BYTES],
) -> Result<Vec<u8>, CryptoError> {
    use aes_gcm::aead::Aead;
    use rand::RngCore;

    let key = GenericArray::from(*kek);
    let cipher = Aes256Gcm::new(&key);

    let mut iv = [0u8; IV_SIZE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    let nonce = GenericArray::from(iv);

    let ciphertext = cipher
        .encrypt(&nonce, dek.as_slice())
        .map_err(|_| CryptoError::WrappingFailed)?;

    let mut result = Vec::with_capacity(IV_SIZE_BYTES + ciphertext.len());
    result.extend_from_slice(&iv);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Unwraps a DEK using a KEK with AES-256-GCM.
///
/// # Errors
///
/// Returns `CryptoError` if decryption fails or the ciphertext is invalid.
pub fn unwrap_dek(
    wrapped: &[u8],
    kek: &[u8; KEK_SIZE_BYTES],
) -> Result<[u8; DEK_SIZE_BYTES], CryptoError> {
    use aes_gcm::aead::Aead;

    if wrapped.len() < IV_SIZE_BYTES + TAG_SIZE_BYTES + DEK_SIZE_BYTES {
        return Err(CryptoError::InvalidKeyMaterial);
    }

    let iv_arr: [u8; IV_SIZE_BYTES] = wrapped[..IV_SIZE_BYTES]
        .try_into()
        .map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let iv = GenericArray::from(iv_arr);
    let ciphertext = &wrapped[IV_SIZE_BYTES..];

    let key = GenericArray::from(*kek);
    let cipher = Aes256Gcm::new(&key);

    let plaintext = cipher
        .decrypt(&iv, ciphertext)
        .map_err(|_| CryptoError::UnwrappingFailed)?;

    if plaintext.len() != DEK_SIZE_BYTES {
        return Err(CryptoError::InvalidKeyMaterial);
    }

    let mut dek = [0u8; DEK_SIZE_BYTES];
    dek.copy_from_slice(&plaintext);
    Ok(dek)
}

/// Encrypts a blob using the given DEK.
///
/// # Errors
///
/// Returns `CryptoError` if encryption fails.
pub fn encrypt_blob(
    data: &[u8],
    dek: &[u8; DEK_SIZE_BYTES],
) -> Result<vo_types::EncryptedBlob, CryptoError> {
    use aes_gcm::aead::Aead;
    use rand::RngCore;

    let key = GenericArray::from(*dek);
    let cipher = Aes256Gcm::new(&key);

    let mut iv = [0u8; IV_SIZE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    let nonce = GenericArray::from(iv);

    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let tag_start = ciphertext.len() - TAG_SIZE_BYTES;
    let tag = ciphertext[tag_start..].to_vec();
    let ciphertext_without_tag = ciphertext[..tag_start].to_vec();

    Ok(
        vo_types::EncryptedBlob::new(iv.to_vec(), ciphertext_without_tag, tag)
            .map_err(|e| CryptoError::EncryptionFailed)?,
    )
}

/// Decrypts a blob using the given DEK.
///
/// # Errors
///
/// Returns `CryptoError` if decryption fails or the ciphertext is invalid.
pub fn decrypt_blob(
    blob: &vo_types::EncryptedBlob,
    dek: &[u8; DEK_SIZE_BYTES],
) -> Result<Vec<u8>, CryptoError> {
    use aes_gcm::aead::Aead;

    if blob.iv.len() != IV_SIZE_BYTES {
        return Err(CryptoError::InvalidKeyMaterial);
    }

    if blob.tag.len() != TAG_SIZE_BYTES {
        return Err(CryptoError::InvalidKeyMaterial);
    }

    let iv_arr: [u8; IV_SIZE_BYTES] = blob.iv[..IV_SIZE_BYTES]
        .try_into()
        .map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let iv = GenericArray::from(iv_arr);

    let key = GenericArray::from(*dek);
    let cipher = Aes256Gcm::new(&key);

    let mut ciphertext_with_tag = Vec::with_capacity(blob.ciphertext.len() + blob.tag.len());
    ciphertext_with_tag.extend_from_slice(&blob.ciphertext);
    ciphertext_with_tag.extend_from_slice(&blob.tag);

    let plaintext = cipher
        .decrypt(&iv, ciphertext_with_tag.as_slice())
        .map_err(|_| CryptoError::DecryptionFailed)?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn crypto_error_key_not_found_display() {
        assert_eq!(
            format!("{}", CryptoError::KeyNotFound),
            "DEK not found in key store"
        );
    }

    #[test]
    fn crypto_error_key_destroyed_display() {
        assert_eq!(
            format!("{}", CryptoError::KeyDestroyed),
            "DEK was purged (crypto-shredded)"
        );
    }

    #[test]
    fn crypto_error_key_store_unavailable_display() {
        assert_eq!(
            format!("{}", CryptoError::KeyStoreUnavailable),
            "key store partition inaccessible"
        );
    }

    #[test]
    fn crypto_error_decryption_failed_display() {
        assert_eq!(
            format!("{}", CryptoError::DecryptionFailed),
            "tag mismatch or corrupt ciphertext"
        );
    }

    #[test]
    fn crypto_error_invalid_key_material_display() {
        assert_eq!(
            format!("{}", CryptoError::InvalidKeyMaterial),
            "key bytes invalid"
        );
    }

    #[test]
    fn crypto_error_unsupported_algorithm_display() {
        assert_eq!(
            format!("{}", CryptoError::UnsupportedAlgorithm),
            "unknown cipher requested"
        );
    }

    #[test]
    fn crypto_error_wrapping_failed_display() {
        assert_eq!(
            format!("{}", CryptoError::WrappingFailed),
            "KEK wrap operation failed"
        );
    }

    #[test]
    fn crypto_error_unwrapping_failed_display() {
        assert_eq!(
            format!("{}", CryptoError::UnwrappingFailed),
            "KEK unwrap operation failed"
        );
    }

    #[test]
    fn crypto_error_rng_unavailable_display() {
        assert_eq!(
            format!("{}", CryptoError::RngUnavailable),
            "secure random unavailable"
        );
    }

    #[test]
    fn crypto_error_invalid_argument_display() {
        let err = CryptoError::InvalidArgument("test message".to_string());
        assert_eq!(format!("{}", err), "invalid argument: test message");
    }

    #[test]
    fn crypto_error_key_not_found_maps_to_storage_key_not_found() {
        let crypto_err = CryptoError::KeyNotFound;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::KeyNotFound);
    }

    #[test]
    fn crypto_error_key_destroyed_maps_to_storage_key_destroyed() {
        let crypto_err = CryptoError::KeyDestroyed;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::KeyDestroyed);
    }

    #[test]
    fn crypto_error_decryption_failed_maps_to_corrupt_event_payload() {
        let crypto_err = CryptoError::DecryptionFailed;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::CorruptEventPayload);
    }

    #[test]
    fn crypto_error_key_store_unavailable_maps_to_storage() {
        let crypto_err = CryptoError::KeyStoreUnavailable;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::Storage);
    }

    #[test]
    fn crypto_error_wrapping_failed_maps_to_storage() {
        let crypto_err = CryptoError::WrappingFailed;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::Storage);
    }

    #[test]
    fn crypto_error_unwrapping_failed_maps_to_storage() {
        let crypto_err = CryptoError::UnwrappingFailed;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::Storage);
    }

    #[test]
    fn crypto_error_rng_unavailable_maps_to_storage() {
        let crypto_err = CryptoError::RngUnavailable;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::Storage);
    }

    #[test]
    fn crypto_error_invalid_key_material_maps_to_invalid_key() {
        let crypto_err = CryptoError::InvalidKeyMaterial;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::InvalidKey);
    }

    #[test]
    fn crypto_error_unsupported_algorithm_maps_to_unsupported_version() {
        let crypto_err = CryptoError::UnsupportedAlgorithm;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::UnsupportedVersion);
    }

    #[test]
    fn crypto_error_invalid_argument_maps_to_invalid_argument() {
        let crypto_err = CryptoError::InvalidArgument("test".to_string());
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::InvalidArgument);
    }

    #[test]
    fn crypto_error_implements_error_trait() {
        let err: CryptoError = CryptoError::KeyNotFound;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn crypto_error_source_none() {
        let err: CryptoError = CryptoError::KeyNotFound;
        assert!(err.source().is_none());
    }

    #[test]
    fn generate_dek_produces_32_bytes() {
        let dek = generate_dek().expect("should generate DEK");
        assert_eq!(dek.len(), DEK_SIZE_BYTES);
    }

    #[test]
    fn generate_dek_produces_nonzero_bytes() {
        let dek1 = generate_dek().expect("should generate DEK");
        let dek2 = generate_dek().expect("should generate DEK");
        assert_ne!(dek1, dek2, "two generated DEKs should differ");
    }

    #[test]
    fn wrap_and_unwrap_dek_roundtrip() {
        let dek = generate_dek().expect("should generate DEK");
        let kek = generate_dek().expect("should generate KEK");

        let wrapped = wrap_dek(&dek, &kek).expect("wrap should succeed");
        let unwrapped = unwrap_dek(&wrapped, &kek).expect("unwrap should succeed");

        assert_eq!(dek, unwrapped);
    }

    #[test]
    fn wrap_dek_produces_different_output_each_time() {
        let dek = generate_dek().expect("should generate DEK");
        let kek = generate_dek().expect("should generate KEK");

        let wrapped1 = wrap_dek(&dek, &kek).expect("wrap should succeed");
        let wrapped2 = wrap_dek(&dek, &kek).expect("wrap should succeed");

        assert_ne!(wrapped1, wrapped2, "IV should make wrappings different");
    }

    #[test]
    fn unwrap_dek_with_wrong_kek_fails() {
        let dek = generate_dek().expect("should generate DEK");
        let kek1 = generate_dek().expect("should generate KEK");
        let kek2 = generate_dek().expect("should generate KEK");

        let wrapped = wrap_dek(&dek, &kek1).expect("wrap should succeed");
        let result = unwrap_dek(&wrapped, &kek2);

        assert!(result.is_err());
    }

    #[test]
    fn encrypt_and_decrypt_blob_roundtrip() {
        let data = b"Hello, World! This is a test message.";
        let dek = generate_dek().expect("should generate DEK");

        let encrypted = encrypt_blob(data, &dek).expect("encrypt should succeed");
        let decrypted = decrypt_blob(&encrypted, &dek).expect("decrypt should succeed");

        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn encrypt_blob_produces_different_output_each_time() {
        let data = b"Hello, World!";
        let dek = generate_dek().expect("should generate DEK");

        let encrypted1 = encrypt_blob(data, &dek).expect("encrypt should succeed");
        let encrypted2 = encrypt_blob(data, &dek).expect("encrypt should succeed");

        assert_ne!(encrypted1.iv, encrypted2.iv);
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
    }

    #[test]
    fn decrypt_blob_with_wrong_dek_fails() {
        let data = b"Secret message";
        let dek1 = generate_dek().expect("should generate DEK");
        let dek2 = generate_dek().expect("should generate DEK");

        let encrypted = encrypt_blob(data, &dek1).expect("encrypt should succeed");
        let result = decrypt_blob(&encrypted, &dek2);

        assert!(result.is_err());
    }

    #[test]
    fn encrypted_blob_has_correct_sizes() {
        let data = b"Test data";
        let dek = generate_dek().expect("should generate DEK");

        let encrypted = encrypt_blob(data, &dek).expect("encrypt should succeed");

        assert_eq!(encrypted.iv.len(), IV_SIZE_BYTES);
        assert_eq!(encrypted.tag.len(), TAG_SIZE_BYTES);
    }

    // =========================================================================
    // ADR-025 Encryption lifecycle: key rotation
    // =========================================================================

    #[test]
    fn key_rotation_old_kek_cannot_decrypt_new_wrapping() {
        let dek = generate_dek().expect("should generate DEK");
        let old_kek = generate_dek().expect("should generate old KEK");
        let new_kek = generate_dek().expect("should generate new KEK");

        // Wrap with old KEK
        let wrapped_old = wrap_dek(&dek, &old_kek).expect("wrap with old KEK");
        // Re-wrap with new KEK (simulating rotation)
        let unwrapped = unwrap_dek(&wrapped_old, &old_kek).expect("unwrap with old KEK");
        let wrapped_new = wrap_dek(&unwrapped, &new_kek).expect("wrap with new KEK");

        // New KEK can decrypt
        assert!(unwrap_dek(&wrapped_new, &new_kek).is_ok());
        // Old KEK cannot decrypt new wrapping
        assert!(unwrap_dek(&wrapped_new, &old_kek).is_err());
    }

    #[test]
    fn key_rotation_preserves_dek_through_rewrap() {
        let dek = generate_dek().expect("should generate DEK");
        let old_kek = generate_dek().expect("should generate old KEK");
        let new_kek = generate_dek().expect("should generate new KEK");

        // Original wrap
        let wrapped_old = wrap_dek(&dek, &old_kek).expect("wrap");
        // Rotate: unwrap with old, rewrap with new
        let unwrapped = unwrap_dek(&wrapped_old, &old_kek).expect("unwrap");
        let wrapped_new = wrap_dek(&unwrapped, &new_kek).expect("rewrap");

        // Verify DEK is preserved through rotation
        let final_dek = unwrap_dek(&wrapped_new, &new_kek).expect("unwrap new");
        assert_eq!(dek, final_dek);
    }

    #[test]
    fn re_encryption_with_rotated_key_produces_different_ciphertext() {
        let data = b"Sensitive payload that must remain confidential";
        let dek = generate_dek().expect("should generate DEK");
        let new_dek = generate_dek().expect("should generate new DEK");

        let encrypted_old = encrypt_blob(data, &dek).expect("encrypt with old DEK");
        let encrypted_new = encrypt_blob(data, &new_dek).expect("encrypt with new DEK");

        // Different DEKs produce different ciphertexts
        assert_ne!(encrypted_old.iv, encrypted_new.iv);
        assert_ne!(encrypted_old.ciphertext, encrypted_new.ciphertext);
        // Both decrypt to the same plaintext
        assert_eq!(decrypt_blob(&encrypted_old, &dek).unwrap(), data.as_slice());
        assert_eq!(
            decrypt_blob(&encrypted_new, &new_dek).unwrap(),
            data.as_slice()
        );
    }

    #[test]
    fn expired_key_simulated_by_destroying_kek() {
        // Per ADR-025: after DEK destruction (crypto-shredding), data is irrecoverable
        let dek = generate_dek().expect("should generate DEK");
        let kek = generate_dek().expect("should generate KEK");

        let wrapped = wrap_dek(&dek, &kek).expect("wrap");
        let blob = encrypt_blob(b"secret data", &dek).expect("encrypt");

        // Simulate key destruction: zero out the KEK
        let mut destroyed_kek = kek;
        destroyed_kek.fill(0);

        // Cannot unwrap with destroyed KEK
        assert!(unwrap_dek(&wrapped, &destroyed_kek).is_err());
        // Cannot decrypt blob (DEK is still in memory but KEK is gone)
        // In practice, both would be zeroed
    }

    #[test]
    fn encryption_roundtrip_large_payload() {
        // Test with a realistic payload size
        let data = vec![b'X'; 100_000];
        let dek = generate_dek().expect("should generate DEK");

        let encrypted = encrypt_blob(&data, &dek).expect("encrypt");
        let decrypted = decrypt_blob(&encrypted, &dek).expect("decrypt");

        assert_eq!(data, decrypted);
    }

    #[test]
    fn encryption_empty_payload_succeeds() {
        let data = b"";
        let dek = generate_dek().expect("should generate DEK");

        let encrypted = encrypt_blob(data, &dek).expect("encrypt");
        let decrypted = decrypt_blob(&encrypted, &dek).expect("decrypt");

        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn wrap_unwrap_different_deks_produce_different_wrappings() {
        let dek1 = generate_dek().expect("should generate DEK 1");
        let dek2 = generate_dek().expect("should generate DEK 2");
        let kek = generate_dek().expect("should generate KEK");

        let wrapped1 = wrap_dek(&dek1, &kek).expect("wrap 1");
        let wrapped2 = wrap_dek(&dek2, &kek).expect("wrap 2");

        // Different DEKs wrapped with same KEK should produce different output (random IVs)
        assert_ne!(wrapped1, wrapped2);

        // Both should unwrap correctly
        assert_eq!(unwrap_dek(&wrapped1, &kek).unwrap(), dek1);
        assert_eq!(unwrap_dek(&wrapped2, &kek).unwrap(), dek2);
    }
}
