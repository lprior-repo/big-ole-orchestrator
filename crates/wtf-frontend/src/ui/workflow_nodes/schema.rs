use serde::{Deserialize, Serialize};
use std::fmt;

fn deserialize_empty_to_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.filter(|s| !s.is_empty()))
}

fn default_delay_ms() -> u64 {
    60_000
}

fn default_duration_ms() -> u64 {
    1_000
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryKey(pub String);

impl MemoryKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MemoryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for MemoryKey {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceName(pub String);

impl ServiceName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ServiceName {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandlerName(pub String);

impl HandlerName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HandlerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for HandlerName {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectKey(pub String);

impl ObjectKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ObjectKey {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromiseName(pub String);

impl PromiseName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PromiseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for PromiseName {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AwakeableId(pub String);

impl AwakeableId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AwakeableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for AwakeableId {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HttpPath(pub String);

impl HttpPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HttpPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for HttpPath {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CronExpression(pub String);

impl CronExpression {
    pub fn new(expr: impl Into<String>) -> Self {
        Self(expr.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CronExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for CronExpression {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RouterBranchId(pub String);

impl RouterBranchId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RouterBranchId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Condition(pub String);

impl Condition {
    pub fn new(expr: impl Into<String>) -> Self {
        Self(expr.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl Default for Condition {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchName(pub String);

impl BranchName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for BranchName {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CodeContent(pub String);

impl CodeContent {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CodeContent {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowNode {
    HttpTrigger(HttpTriggerConfig),
    ScheduleTrigger(ScheduleTriggerConfig),
    ServiceCall(ServiceCallConfig),
    SendMessage(SendMessageConfig),
    DelayedMessage(DelayedMessageConfig),
    SaveToMemory(SaveToMemoryConfig),
    LoadFromMemory(LoadFromMemoryConfig),
    Delay(DelayConfig),
    Router(RouterConfig),
    WaitForWebhook(WaitForWebhookConfig),
    WaitForSignal(WaitForSignalConfig),
    RunCode(RunCodeConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTriggerConfig {
    pub path: HttpPath,
    pub method: HttpMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleTriggerConfig {
    pub schedule: CronExpression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCallConfig {
    #[serde(default)]
    pub target_type: TargetType,
    pub service_name: ServiceName,
    #[serde(default, deserialize_with = "deserialize_empty_to_none")]
    pub key: Option<ObjectKey>,
    pub handler_name: HandlerName,
    pub input: serde_json::Value,
    #[serde(default, deserialize_with = "deserialize_empty_to_none")]
    pub condition: Option<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageConfig {
    pub target_type: TargetType,
    pub service_name: ServiceName,
    pub key: Option<ObjectKey>,
    pub handler_name: HandlerName,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayedMessageConfig {
    pub target_type: TargetType,
    pub service_name: ServiceName,
    pub key: Option<ObjectKey>,
    pub handler_name: HandlerName,
    pub input: serde_json::Value,
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetType {
    #[default]
    Service,
    VirtualObject,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveToMemoryConfig {
    pub key: MemoryKey,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadFromMemoryConfig {
    pub key: MemoryKey,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayConfig {
    #[serde(default = "default_duration_ms")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub branches: Vec<RouterBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterBranch {
    #[serde(default)]
    pub id: RouterBranchId,
    pub name: BranchName,
    pub condition: Condition,
    pub next_node_id: Option<NodeId>,
}

impl RouterBranch {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: RouterBranchId::new(),
            name: BranchName::new(name),
            condition: Condition::default(),
            next_node_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForWebhookConfig {
    pub awakeable_id: AwakeableId,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForSignalConfig {
    pub promise_name: PromiseName,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCodeConfig {
    pub code: CodeContent,
    pub language: CodeLanguage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeLanguage {
    JavaScript,
    Python,
    Expression,
}

impl Default for WorkflowNode {
    fn default() -> Self {
        WorkflowNode::HttpTrigger(HttpTriggerConfig {
            path: HttpPath::default(),
            method: HttpMethod::POST,
        })
    }
}
