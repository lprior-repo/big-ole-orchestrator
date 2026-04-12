use aes_gcm::aes::Aes256;
use aes_gcm::AesGcm;
use aes_gcm::NewAead;
use generic_array::{typenum::U12, GenericArray};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    KeyNotFound,
    KeyDestroyed,
    KeyStoreUnavailable,
    DecryptionFailed,
    InvalidKeyMaterial,
    UnsupportedAlgorithm,
    WrappingFailed,
    UnwrappingFailed,
    RngUnavailable,
    EncryptionFailed,
    InvalidArgument(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyNotFound => write!(f, "DEK not found in key store"),
            Self::KeyDestroyed => write!(f, "DEK was purged (crypto-shredded)"),
            Self::KeyStoreUnavailable => write!(f, "key store partition inaccessible"),
            Self::DecryptionFailed => write!(f, "tag mismatch or corrupt ciphertext"),
            Self::InvalidKeyMaterial => write!(f, "key bytes invalid"),
            Self::UnsupportedAlgorithm => write!(f, "unknown cipher requested"),
            Self::WrappingFailed => write!(f, "KEK wrap operation failed"),
            Self::UnwrappingFailed => write!(f, "KEK unwrap operation failed"),
            Self::RngUnavailable => write!(f, "secure random unavailable"),
            Self::EncryptionFailed => write!(f, "encryption operation failed"),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
        }
    }
}

impl std::error::Error for CryptoError {}

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

pub fn generate_dek() -> Result<[u8; DEK_SIZE_BYTES], CryptoError> {
    use rand::RngCore;
    let mut key = [0u8; DEK_SIZE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut key);
    Ok(key)
}

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

    Ok(vo_types::EncryptedBlob::new(
        iv.to_vec(),
        ciphertext_without_tag,
        tag,
    ))
}

pub fn decrypt_blob(
    blob: &vo_types::EncryptedBlob,
    dek: &[u8; DEK_SIZE_BYTES],
) -> Result<Vec<u8>, CryptoError> {
    use aes_gcm::aead::Aead;

    if blob.iv.len() != IV_SIZE_BYTES {
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
}
