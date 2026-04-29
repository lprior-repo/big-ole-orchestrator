use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::{ApiKeyEntry, ApiKeyStore, ApiKeyStoreError, API_KEY_PARTITION};

pub struct FjallApiKeyStore {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
}

impl FjallApiKeyStore {
    pub fn open(db: &fjall::Database) -> Result<Self, ApiKeyStoreError> {
        let partition = db
            .keyspace(API_KEY_PARTITION, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| ApiKeyStoreError::Storage {
                reason: format!("failed to open api_key partition: {e}"),
            })?;
        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
        })
    }

    #[expect(clippy::expect_used, clippy::cast_possible_truncation)]
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect(
                "system time is guaranteed to be after UNIX epoch on properly configured systems",
            )
            .as_millis() as u64
    }

    pub fn create_key(
        &self,
        key: &str,
        name: &str,
    ) -> Result<String, ApiKeyStoreError> {
        let key_id = generate_key_id();
        let key_hash = hash_key(key);
        let now_ms = Self::now_ms();

        let entry = ApiKeyEntry::new(key_id.clone(), key_hash, name.to_string(), now_ms);
        let value_bytes = super::encode_api_key_entry(&entry)?;

        self.partition
            .insert(&key_id, &value_bytes)
            .map_err(|e| ApiKeyStoreError::Storage {
                reason: e.to_string(),
            })?;

        Ok(key_id)
    }

    pub fn list_keys(&self) -> Result<Vec<ApiKeyEntry>, ApiKeyStoreError> {
        let mut keys = Vec::new();

        let iter = self.partition.iter();
        for item in iter {
            let (_, value_bytes) = item.into_inner().map_err(|e| ApiKeyStoreError::Storage {
                reason: e.to_string(),
            })?;

            if let Ok(entry) = super::decode_api_key_entry(&value_bytes) {
                keys.push(entry);
            }
        }

        Ok(keys)
    }

    pub fn revoke_key(&self, key_id: &str) -> Result<(), ApiKeyStoreError> {
        let value_bytes = self.partition.get(key_id).map_err(|e| ApiKeyStoreError::Storage {
            reason: e.to_string(),
        })?;

        let mut entry: ApiKeyEntry = match value_bytes {
            Some(bytes) => super::decode_api_key_entry(&bytes)?,
            None => return Err(ApiKeyStoreError::NotFound),
        };

        entry.revoked = true;
        let new_value_bytes = super::encode_api_key_entry(&entry)?;

        self.partition
            .insert(key_id, &new_value_bytes)
            .map_err(|e| ApiKeyStoreError::Storage {
                reason: e.to_string(),
            })?;

        Ok(())
    }
}

impl ApiKeyStore for FjallApiKeyStore {
    fn validate_key(&self, key: &str) -> Result<(), ApiKeyStoreError> {
        let key_hash = hash_key(key);
        let now_ms = Self::now_ms();

        let iter = self.partition.iter();
        for item in iter {
            let (_, value_bytes) = item.into_inner().map_err(|e| ApiKeyStoreError::Storage {
                reason: e.to_string(),
            })?;

            if let Ok(entry) = super::decode_api_key_entry(&value_bytes) {
                if entry.key_hash == key_hash {
                    if !entry.is_valid(now_ms) {
                        if entry.revoked {
                            return Err(ApiKeyStoreError::Revoked);
                        }
                        return Err(ApiKeyStoreError::Expired);
                    }
                    return Ok(());
                }
            }
        }

        Err(ApiKeyStoreError::NotFound)
    }
}

fn generate_key_id() -> String {
    ulid::Ulid::new().to_string()
}

fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}