pub const API_KEY_PARTITION: &str = "api_keys";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiKeyStoreError {
    #[error("api key storage error: {reason}")]
    Storage { reason: String },
    #[error("api key codec error: {reason}")]
    Codec { reason: String },
    #[error("invalid api key argument")]
    InvalidArgument,
    #[error("api key not found")]
    NotFound,
    #[error("api key revoked")]
    Revoked,
    #[error("api key expired")]
    Expired,
}

pub trait ApiKeyStore: Send + Sync {
    fn validate_key(&self, key: &str) -> Result<(), ApiKeyStoreError>;
}

pub mod fjall_api_key;
pub use fjall_api_key::FjallApiKeyStore;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApiKeyEntry {
    pub key_id: String,
    pub key_hash: String,
    pub name: String,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub revoked: bool,
}

impl ApiKeyEntry {
    pub fn new(key_id: String, key_hash: String, name: String, created_at: u64) -> Self {
        Self {
            key_id,
            key_hash,
            name,
            created_at,
            expires_at: None,
            revoked: false,
        }
    }

    pub fn with_expiry(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    #[must_use]
    pub fn is_valid(&self, now_ms: u64) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(expires_at) = self.expires_at {
            if now_ms >= expires_at {
                return false;
            }
        }
        true
    }
}

pub fn encode_api_key_entry(entry: &ApiKeyEntry) -> Result<Vec<u8>, ApiKeyStoreError> {
    serde_json::to_vec(entry).map_err(|e| ApiKeyStoreError::Codec {
        reason: e.to_string(),
    })
}

pub fn decode_api_key_entry(bytes: &[u8]) -> Result<ApiKeyEntry, ApiKeyStoreError> {
    serde_json::from_slice(bytes).map_err(|e| ApiKeyStoreError::Codec {
        reason: e.to_string(),
    })
}