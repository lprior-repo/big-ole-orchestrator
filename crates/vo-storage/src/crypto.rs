//! Crypto primitives for encryption/decryption (ADR-025, ADR-040).
//!
//! Architecture: Data (`CryptoError`) → Calc (wrap/unwrap/encrypt/decrypt)
//!             → Actions (public API functions).

use aes_gcm::aes::Aes256;
use aes_gcm::AesGcm;
use aes_gcm::NewAead;
use generic_array::{typenum::U12, GenericArray};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq)]
pub struct SecretDek([u8; DEK_SIZE_BYTES]);

impl Drop for SecretDek {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::ops::Deref for SecretDek {
    type Target = [u8; DEK_SIZE_BYTES];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<[u8; DEK_SIZE_BYTES]> for SecretDek {
    fn eq(&self, other: &[u8; DEK_SIZE_BYTES]) -> bool {
        &self.0 == other
    }
}

impl PartialEq<SecretDek> for [u8; DEK_SIZE_BYTES] {
    fn eq(&self, other: &SecretDek) -> bool {
        self == &other.0
    }
}

// ---------------------------------------------------------------------------
// Data layer - error enum
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DEK_SIZE_BYTES: usize = 32;
pub const KEK_SIZE_BYTES: usize = 32;
pub const IV_SIZE_BYTES: usize = 12;
pub const TAG_SIZE_BYTES: usize = 16;

type Aes256Gcm = AesGcm<Aes256, U12>;

/// Generates a new random DEK (Data Encryption Key).
///
/// # Errors
///
/// Returns `CryptoError::RngUnavailable` if the OS random number generator fails.
pub fn generate_dek() -> Result<[u8; DEK_SIZE_BYTES], CryptoError> {
    use rand::RngCore;
    let mut key = [0u8; DEK_SIZE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut key);
    Ok(key)
}

/// Wraps a DEK with a KEK (Key Encryption Key) using AES-256-GCM.
///
/// # Errors
///
/// Returns `CryptoError::WrappingFailed` if encryption fails.
/// Returns `CryptoError::RngUnavailable` if random IV generation fails.
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

/// Unwraps a DEK using a KEK via AES-256-GCM decryption.
///
/// # Errors
///
/// Returns `CryptoError::InvalidKeyMaterial` if the wrapped data is malformed.
/// Returns `CryptoError::UnwrappingFailed` if decryption fails.
pub fn unwrap_dek(
    wrapped: &[u8],
    kek: &[u8; KEK_SIZE_BYTES],
) -> Result<SecretDek, CryptoError> {
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
        let mut p = plaintext;
        p.zeroize();
        return Err(CryptoError::InvalidKeyMaterial);
    }

    let mut dek = [0u8; DEK_SIZE_BYTES];
    dek.copy_from_slice(&plaintext);
    let mut p = plaintext;
    p.zeroize();
    Ok(SecretDek(dek))
}

/// Encrypts data using AES-256-GCM with the given DEK.
///
/// # Errors
///
/// Returns `CryptoError::EncryptionFailed` if encryption fails.
/// Returns `CryptoError::RngUnavailable` if random IV generation fails.
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
            .map_err(|_| CryptoError::EncryptionFailed)?,
    )
}

/// Decrypts an encrypted blob using AES-256-GCM with the given DEK.
///
/// # Errors
///
/// Returns `CryptoError::InvalidKeyMaterial` if the blob is malformed.
/// Returns `CryptoError::DecryptionFailed` if decryption fails.
pub fn decrypt_blob(
    blob: &vo_types::EncryptedBlob,
    dek: &[u8; DEK_SIZE_BYTES],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
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

    ciphertext_with_tag.zeroize();

    Ok(Zeroizing::new(plaintext))
}
