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
}
