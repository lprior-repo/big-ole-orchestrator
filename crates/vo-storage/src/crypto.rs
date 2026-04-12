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
            | CryptoError::RngUnavailable => Self::Storage,
            CryptoError::InvalidKeyMaterial => Self::InvalidKey,
            CryptoError::UnsupportedAlgorithm => Self::UnsupportedVersion,
            CryptoError::InvalidArgument(_) => Self::InvalidArgument,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypto_error_display() {
        assert_eq!(
            format!("{}", CryptoError::KeyNotFound),
            "DEK not found in key store"
        );
        assert_eq!(
            format!("{}", CryptoError::KeyDestroyed),
            "DEK was purged (crypto-shredded)"
        );
        assert_eq!(
            format!("{}", CryptoError::DecryptionFailed),
            "tag mismatch or corrupt ciphertext"
        );
    }

    #[test]
    fn crypto_error_to_storage_error() {
        let crypto_err = CryptoError::KeyNotFound;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::KeyNotFound);

        let crypto_err = CryptoError::DecryptionFailed;
        let storage_err: crate::codec::StorageError = crypto_err.into();
        assert_eq!(storage_err, crate::codec::StorageError::CorruptEventPayload);
    }
}
