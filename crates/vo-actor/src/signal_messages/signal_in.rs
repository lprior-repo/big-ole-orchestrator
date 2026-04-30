use vo_types::InstanceId;
pub use vo_types::TimestampMs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    Running,
    Failed,
    Completed,
    Cancelled,
    WaitingForSignal,
}

impl LifecycleState {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

pub trait StateLookup: Send + Sync {
    fn derive_lifecycle_state(&self, instance_id: &InstanceId) -> LifecycleState;
    fn derive_error_type(&self, instance_id: &InstanceId) -> Option<&'static str>;
}

#[derive(Debug, Clone)]
pub struct TestStateLookup;

impl TestStateLookup {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestStateLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl StateLookup for TestStateLookup {
    fn derive_lifecycle_state(&self, instance_id: &InstanceId) -> LifecycleState {
        let id_str = instance_id.as_str();
        id_str
            .chars()
            .nth(22)
            .map_or(LifecycleState::Running, |c| match c {
                'C' => LifecycleState::Completed,
                'X' => LifecycleState::Cancelled,
                'F' => LifecycleState::Failed,
                'W' => LifecycleState::WaitingForSignal,
                _ => LifecycleState::Running,
            })
    }

    fn derive_error_type(&self, instance_id: &InstanceId) -> Option<&'static str> {
        let id_str = instance_id.as_str();
        id_str.chars().nth(20).and_then(|c| match c {
            'A' => Some("lock"),
            'S' => Some("storage"),
            'M' => Some("missing"),
            'N' => Some("nodenotfound"),
            'P' => Some("nopathtoterminal"),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretId(pub String);

impl SecretId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WaitKey(String);

impl WaitKey {
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.is_empty() {
            return Err("WaitKey cannot be empty".to_string());
        }
        if input.len() > 256 {
            return Err(format!("WaitKey exceeds 256 characters: {}", input.len()));
        }
        Ok(Self(input.to_string()))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<vo_types::WaitKey> for WaitKey {
    fn from(value: vo_types::WaitKey) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<&vo_types::WaitKey> for WaitKey {
    fn from(value: &vo_types::WaitKey) -> Self {
        Self(value.as_str().to_string())
    }
}

impl From<&WaitKey> for WaitKey {
    fn from(value: &WaitKey) -> Self {
        value.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPayload(Vec<u8>);

impl SignalPayload {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err(format!(
                "SignalPayload exceeds 64 KiB: {} bytes",
                bytes.len()
            ));
        }
        if bytes.contains(&0) {
            return Err("SignalPayload contains null byte".to_string());
        }
        Ok(Self(bytes))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn new_unchecked(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalName(String);

impl SignalName {
    pub fn parse(input: &str) -> Result<Self, String> {
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err("SignalName cannot be empty".to_string());
        }
        if input.len() > MAX_LEN {
            return Err(format!(
                "SignalName exceeds {} characters: {}",
                MAX_LEN,
                input.len()
            ));
        }
        if input.contains('\0') {
            return Err("SignalName contains null byte".to_string());
        }
        let invalid = input
            .chars()
            .filter(|c| !c.is_alphanumeric() && *c != '-' && *c != '_' && *c != '.')
            .collect::<String>();
        if !invalid.is_empty() {
            return Err(format!(
                "SignalName contains invalid characters: {}",
                invalid
            ));
        }
        Ok(Self(input.to_string()))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SignalName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for SignalName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for SignalName {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl PartialEq<String> for SignalName {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SignalName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}