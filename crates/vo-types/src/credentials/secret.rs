use serde::{Deserialize, Serialize};

use crate::ParseError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretValue {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub key_version: u32,
}

impl SecretValue {
    pub fn new(ciphertext: Vec<u8>, nonce: [u8; 12], key_version: u32) -> Result<Self, ParseError> {
        if ciphertext.is_empty() {
            return Err(ParseError::Empty {
                type_name: "SecretValue",
            });
        }
        Ok(Self {
            ciphertext,
            nonce,
            key_version,
        })
    }

    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    #[must_use]
    pub fn nonce(&self) -> [u8; 12] {
        self.nonce
    }

    #[must_use]
    pub fn key_version(&self) -> u32 {
        self.key_version
    }
}
