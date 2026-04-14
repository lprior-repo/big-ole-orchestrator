use serde::{Deserialize, Serialize};

use crate::ParseError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DekId(pub(crate) String);

impl DekId {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "DekId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        let ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        if ulid.0 == 0 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: "invalid ULID validation: nil value not permitted".to_string(),
            });
        }
        Ok(Self(ulid.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(ulid::Ulid(u128::from_be_bytes(bytes)).to_string())
    }

    pub fn to_bytes(&self) -> Result<[u8; 16], ParseError> {
        ulid::Ulid::from_string(&self.0)
            .map(|u| u.0.to_be_bytes())
            .map_err(|e| ParseError::InvalidFormat {
                type_name: "DekId",
                reason: format!("cannot convert to bytes: {e}"),
            })
    }
}

impl std::fmt::Display for DekId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DekId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<DekId> for String {
    fn from(value: DekId) -> String {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedDek(pub Vec<u8>);

impl WrappedDek {
    #[must_use]
    pub fn new(wrapped_bytes: Vec<u8>) -> Self {
        Self(wrapped_bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Display for WrappedDek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WrappedDek({} bytes)", self.0.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub iv: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag: Vec<u8>,
}

impl EncryptedBlob {
    pub fn new(iv: Vec<u8>, ciphertext: Vec<u8>, tag: Vec<u8>) -> Result<Self, EncryptionError> {
        if iv.len() != CryptoAlgorithm::IV_SIZE_BYTES {
            return Err(EncryptionError::InvalidIvLength {
                expected: CryptoAlgorithm::IV_SIZE_BYTES,
                actual: iv.len(),
            });
        }
        if tag.len() != CryptoAlgorithm::TAG_SIZE_BYTES {
            return Err(EncryptionError::InvalidTagLength {
                expected: CryptoAlgorithm::TAG_SIZE_BYTES,
                actual: tag.len(),
            });
        }
        Ok(Self {
            iv,
            ciphertext,
            tag,
        })
    }

    #[must_use]
    pub fn total_size(&self) -> usize {
        self.iv.len() + self.ciphertext.len() + self.tag.len()
    }
}

impl std::fmt::Display for EncryptedBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EncryptedBlob(iv={}, ciphertext={}, tag={})",
            self.iv.len(),
            self.ciphertext.len(),
            self.tag.len()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CryptoAlgorithm {
    Aes256Gcm,
}

impl CryptoAlgorithm {
    pub const IV_SIZE_BYTES: usize = 12;
    pub const TAG_SIZE_BYTES: usize = 16;
    pub const KEY_SIZE_BYTES: usize = 32;
}

impl std::fmt::Display for CryptoAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aes256Gcm => write!(f, "AES-256-GCM"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub created_at_ms: u64,
    pub algorithm: CryptoAlgorithm,
    pub instance_id: crate::InstanceId,
}

impl KeyMetadata {
    pub fn new(instance_id: crate::InstanceId, algorithm: CryptoAlgorithm) -> Self {
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            created_at_ms,
            algorithm,
            instance_id,
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn dek_id_roundtrip_bytes() {
        let id = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let bytes = id.to_bytes().expect("valid bytes");
        let id2 = DekId::from_bytes(bytes);
        assert_eq!(id.as_str(), id2.as_str());
    }

    #[test]
    fn wrapped_dek_creation() {
        let wrapped = WrappedDek::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(wrapped.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn encrypted_blob_creation() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]);
        assert_eq!(blob.total_size(), 60);
    }

    #[test]
    fn crypto_algorithm_constants() {
        assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
        assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
        assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);
    }

    #[test]
    fn key_metadata_creation() {
        let instance_id =
            crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let metadata = KeyMetadata::new(instance_id, CryptoAlgorithm::Aes256Gcm);
        assert_eq!(
            metadata.instance_id,
            crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
        );
        assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
        assert!(metadata.created_at_ms > 0);
    }
}
