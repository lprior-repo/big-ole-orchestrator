use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ParseError;
use crate::DurationMs;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CredentialKind {
    ApiKey,
    Password,
    Token,
    Certificate,
    SigningKey,
    EncryptionKey,
    Custom(String),
}

impl CredentialKind {
    #[must_use]
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::ApiKey,
            Self::Password,
            Self::Token,
            Self::Certificate,
            Self::SigningKey,
            Self::EncryptionKey,
        ]
    }
}

impl fmt::Display for CredentialKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey => write!(f, "ApiKey"),
            Self::Password => write!(f, "Password"),
            Self::Token => write!(f, "Token"),
            Self::Certificate => write!(f, "Certificate"),
            Self::SigningKey => write!(f, "SigningKey"),
            Self::EncryptionKey => write!(f, "EncryptionKey"),
            Self::Custom(s) => write!(f, "Custom({s})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialStatus {
    Active,
    Rotating,
    Expired,
    Revoked,
    Superseded,
}

impl CredentialStatus {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Expired | Self::Revoked | Self::Superseded)
    }
}

impl fmt::Display for CredentialStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Rotating => write!(f, "Rotating"),
            Self::Expired => write!(f, "Expired"),
            Self::Revoked => write!(f, "Revoked"),
            Self::Superseded => write!(f, "Superseded"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationPolicy {
    Manual,
    TimeBased {
        interval: DurationMs,
        overlap_window: DurationMs,
    },
    UsageBased {
        max_uses: u64,
        overlap_window: DurationMs,
    },
    EventBased {
        trigger_events: Vec<String>,
        overlap_window: DurationMs,
    },
}

impl RotationPolicy {
    pub fn validate(&self) -> Result<(), ParseError> {
        const MIN_OVERLAP_MS: u64 = 60_000;
        match self {
            Self::Manual => Ok(()),
            Self::TimeBased {
                interval,
                overlap_window,
            } => {
                if interval.0 == 0 {
                    return Err(ParseError::ZeroValue {
                        type_name: "interval",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
            Self::UsageBased {
                max_uses,
                overlap_window,
            } => {
                if *max_uses == 0 {
                    return Err(ParseError::ZeroValue {
                        type_name: "max_uses",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
            Self::EventBased {
                trigger_events,
                overlap_window,
            } => {
                if trigger_events.is_empty() {
                    return Err(ParseError::Empty {
                        type_name: "trigger_events",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationStatus {
    Idle,
    Rotating,
    WaitingForOverlap,
    Failed(String),
}

impl fmt::Display for RotationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Rotating => write!(f, "Rotating"),
            Self::WaitingForOverlap => write!(f, "WaitingForOverlap"),
            Self::Failed(reason) => write!(f, "Failed({reason})"),
        }
    }
}
