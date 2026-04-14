use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::FenceToken;
use crate::ParseError;

use super::errors::{IsolationLevel, PluginHotLoadError};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginName(pub(crate) String);

impl PluginName {
    pub fn new(input: &str) -> Result<Self, ParseError> {
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: "PluginName",
            });
        }
        if input.len() > super::PLUGIN_NAME_MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: "PluginName",
                max: super::PLUGIN_NAME_MAX_LEN,
                actual: input.len(),
            });
        }
        let invalid: String = input
            .chars()
            .filter(|c| !c.is_ascii_alphanumeric() && *c != '-')
            .collect();
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: "PluginName",
                invalid_chars: invalid,
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PluginVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl PluginVersion {
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub const fn major(&self) -> u32 {
        self.major
    }

    #[must_use]
    pub const fn minor(&self) -> u32 {
        self.minor
    }

    #[must_use]
    pub const fn patch(&self) -> u32 {
        self.patch
    }

    #[must_use]
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub(crate) String);

impl CapabilityId {
    #[must_use]
    pub fn new(input: &str) -> Self {
        Self(input.to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceKey(pub(crate) String);

impl InstanceKey {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl fmt::Display for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginVersionConstraint {
    pub name: PluginName,
    pub range: VersionRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionRange(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub memory_bytes: u64,
    pub cpu_units: u32,
    pub max_instances: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion(pub(crate) u16);

impl SchemaVersion {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }
}

impl From<SchemaVersion> for u16 {
    fn from(v: SchemaVersion) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactRef(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginArtifact {
    pub artifact_ref: ArtifactRef,
    pub checksum: crate::BinaryHash,
    pub schema_version: SchemaVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId {
    name: PluginName,
    version: PluginVersion,
    instance_key: InstanceKey,
}

impl PluginId {
    #[must_use]
    pub fn new(name: PluginName, version: PluginVersion, instance_key: InstanceKey) -> Self {
        Self {
            name,
            version,
            instance_key,
        }
    }

    #[must_use]
    pub fn name(&self) -> &PluginName {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> &PluginVersion {
        &self.version
    }

    #[must_use]
    pub fn instance_key(&self) -> &InstanceKey {
        &self.instance_key
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}#{}", self.name, self.version, self.instance_key.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id: PluginId,
    pub schema_version: SchemaVersion,
    pub capabilities: Vec<CapabilityId>,
    pub dependencies: Vec<PluginVersionConstraint>,
    pub resource_requirements: ResourceBudget,
    pub isolation_level: IsolationLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginFailureContext {
    pub error: PluginHotLoadError,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstance {
    pub descriptor: PluginDescriptor,
    pub state: PluginState,
    pub loaded_at: u64,
    pub load_sequence: u64,
    pub fence_token: FenceToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginState {
    Registered,
    Loading,
    Active,
    Quiescing,
    Unloaded,
    Failed(PluginFailureContext),
}

impl PluginState {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, PluginState::Unloaded | PluginState::Failed(_))
    }

    #[must_use]
    pub fn get_valid_transitions(&self) -> Vec<PluginTransition> {
        match self {
            PluginState::Registered => vec![PluginTransition::Load {
                expected_version: PluginVersion::new(0, 0, 0),
            }],
            PluginState::Loading => vec![
                PluginTransition::Activate,
                PluginTransition::Fail {
                    error: PluginHotLoadError::new(
                        super::PluginErrorCategory::LoadFailure,
                        super::PluginErrorDetail::PluginNotFound(PluginId::new(
                            PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                            PluginVersion::new(0, 0, 0),
                            InstanceKey::new(),
                        )),
                        super::PluginErrorContext::DuringLoad,
                    ),
                },
            ],
            PluginState::Active => vec![
                PluginTransition::Quiesce,
                PluginTransition::Reload {
                    new_descriptor: PluginDescriptor {
                        id: PluginId::new(
                            PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                            PluginVersion::new(0, 0, 0),
                            InstanceKey::new(),
                        ),
                        schema_version: super::SchemaVersion(0),
                        capabilities: vec![],
                        dependencies: vec![],
                        resource_requirements: super::ResourceBudget {
                            memory_bytes: 0,
                            cpu_units: 0,
                            max_instances: 0,
                        },
                        isolation_level: super::IsolationLevel::SharedRuntime,
                    },
                },
                PluginTransition::Fail {
                    error: PluginHotLoadError::new(
                        super::PluginErrorCategory::ActivationFailure,
                        super::PluginErrorDetail::PluginNotFound(PluginId::new(
                            PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                            PluginVersion::new(0, 0, 0),
                            InstanceKey::new(),
                        )),
                        super::PluginErrorContext::DuringActivation,
                    ),
                },
            ],
            PluginState::Quiescing => vec![
                PluginTransition::Unload,
                PluginTransition::Fail {
                    error: PluginHotLoadError::new(
                        super::PluginErrorCategory::QuiesceTimeout,
                        super::PluginErrorDetail::PluginNotFound(PluginId::new(
                            PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                            PluginVersion::new(0, 0, 0),
                            InstanceKey::new(),
                        )),
                        super::PluginErrorContext::DuringQuiesce,
                    ),
                },
            ],
            PluginState::Unloaded => vec![PluginTransition::Register(PluginDescriptor {
                id: PluginId::new(
                    PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                    PluginVersion::new(0, 0, 0),
                    InstanceKey::new(),
                ),
                schema_version: super::SchemaVersion(0),
                capabilities: vec![],
                dependencies: vec![],
                resource_requirements: super::ResourceBudget {
                    memory_bytes: 0,
                    cpu_units: 0,
                    max_instances: 0,
                },
                isolation_level: super::IsolationLevel::SharedRuntime,
            })],
            PluginState::Failed(_) => vec![PluginTransition::Register(PluginDescriptor {
                id: PluginId::new(
                    PluginName::new("").unwrap_or_else(|_| PluginName::new("x").unwrap()),
                    PluginVersion::new(0, 0, 0),
                    InstanceKey::new(),
                ),
                schema_version: super::SchemaVersion(0),
                capabilities: vec![],
                dependencies: vec![],
                resource_requirements: super::ResourceBudget {
                    memory_bytes: 0,
                    cpu_units: 0,
                    max_instances: 0,
                },
                isolation_level: super::IsolationLevel::SharedRuntime,
            })],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginTransition {
    Register(PluginDescriptor),
    Load { expected_version: PluginVersion },
    Activate,
    Quiesce,
    Unload,
    Reload { new_descriptor: PluginDescriptor },
    Fail { error: PluginHotLoadError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotLoadEvent {
    InstallPlugin {
        descriptor: PluginDescriptor,
        artifact: PluginArtifact,
    },
    UninstallPlugin {
        plugin_id: PluginId,
    },
    ActivatePlugin {
        plugin_id: PluginId,
    },
    DeactivatePlugin {
        plugin_id: PluginId,
    },
    ReloadPlugin {
        plugin_id: PluginId,
        new_descriptor: PluginDescriptor,
    },
    PluginHealthCheck {
        plugin_id: PluginId,
    },
}
