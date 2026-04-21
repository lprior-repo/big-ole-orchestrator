use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HttpMethod {
    Get,
    #[default]
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
        }
    }

    pub fn from_str_ignore_case(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "PATCH" => Self::Patch,
            _ => Self::Post,
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self::from_str_ignore_case(s)
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Get, Self::Post, Self::Put, Self::Delete, Self::Patch]
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "PATCH" => Ok(Self::Patch),
            _ => Err(format!("Invalid HTTP method: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleKind {
    Source,
    Target,
}

impl HandleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Target => "target",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "source" => Some(Self::Source),
            "target" => Some(Self::Target),
            _ => None,
        }
    }
}

impl fmt::Display for HandleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HandleKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Invalid handle kind: {s}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeTemplateId {
    HttpHandler,
    KafkaHandler,
    CronTrigger,
    WorkflowSubmit,
    Run,
    ServiceCall,
    ObjectCall,
    SendMessage,
    GetState,
    SetState,
    Condition,
    Parallel,
    Timer,
    Timeout,
}

impl NodeTemplateId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HttpHandler => "http-handler",
            Self::KafkaHandler => "kafka-handler",
            Self::CronTrigger => "cron-trigger",
            Self::WorkflowSubmit => "workflow-submit",
            Self::Run => "run",
            Self::ServiceCall => "service-call",
            Self::ObjectCall => "object-call",
            Self::SendMessage => "send-message",
            Self::GetState => "get-state",
            Self::SetState => "set-state",
            Self::Condition => "condition",
            Self::Parallel => "parallel",
            Self::Timer => "timer",
            Self::Timeout => "timeout",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::HttpHandler => "HTTP Handler",
            Self::KafkaHandler => "Kafka Consumer",
            Self::CronTrigger => "Cron Trigger",
            Self::WorkflowSubmit => "Workflow Submit",
            Self::Run => "Durable Step",
            Self::ServiceCall => "Service Call",
            Self::ObjectCall => "Object Call",
            Self::SendMessage => "Send Message",
            Self::GetState => "Get State",
            Self::SetState => "Set State",
            Self::Condition => "If / Else",
            Self::Parallel => "Parallel",
            Self::Timer => "Timer / Wait",
            Self::Timeout => "Timeout",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::HttpHandler => "Handle HTTP or gRPC requests",
            Self::KafkaHandler => "Consume events from a topic",
            Self::CronTrigger => "Schedule periodic workflow runs",
            Self::WorkflowSubmit => "Start another workflow instance",
            Self::Run => "Run persisted side effects",
            Self::ServiceCall => "Request-response service invocation",
            Self::ObjectCall => "Invoke a virtual object handler",
            Self::SendMessage => "Fire-and-forget one-way call",
            Self::GetState => "Read persisted state",
            Self::SetState => "Write persisted state",
            Self::Condition => "Branch by condition",
            Self::Parallel => "Run branches concurrently",
            Self::Timer => "Pause execution durably",
            Self::Timeout => "Guard a step with deadline",
        }
    }

    pub fn default_config(self) -> serde_json::Value {
        match self {
            Self::HttpHandler => serde_json::json!({
                "port": 8080,
                "method": "GET",
                "path": "/"
            }),
            Self::KafkaHandler => serde_json::json!({
                "topic": "my-topic",
                "group_id": "my-group",
                "brokers": ["localhost:9092"]
            }),
            Self::CronTrigger => serde_json::json!({
                "schedule": "0 * * * *"
            }),
            Self::WorkflowSubmit => serde_json::json!({
                "workflow_name": "my-workflow",
                "input": {}
            }),
            Self::Run => serde_json::json!({
                "command": "./run.sh"
            }),
            Self::ServiceCall => serde_json::json!({
                "service": "my-service",
                "method": "invoke",
                "timeout_ms": 5000
            }),
            Self::ObjectCall => serde_json::json!({
                "object_id": "my-object",
                "operation": "handle"
            }),
            Self::SendMessage => serde_json::json!({
                "channel": "my-channel",
                "payload": {}
            }),
            Self::GetState => serde_json::json!({
                "key": "my-key"
            }),
            Self::SetState => serde_json::json!({
                "key": "my-key",
                "value": null
            }),
            Self::Condition => serde_json::json!({
                "expression": "true"
            }),
            Self::Parallel => serde_json::json!({
                "branches": []
            }),
            Self::Timer => serde_json::json!({
                "duration_ms": 1000
            }),
            Self::Timeout => serde_json::json!({
                "duration_ms": 30000
            }),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "http-handler" => Some(Self::HttpHandler),
            "kafka-handler" => Some(Self::KafkaHandler),
            "cron-trigger" => Some(Self::CronTrigger),
            "workflow-submit" => Some(Self::WorkflowSubmit),
            "run" => Some(Self::Run),
            "service-call" => Some(Self::ServiceCall),
            "object-call" => Some(Self::ObjectCall),
            "send-message" => Some(Self::SendMessage),
            "get-state" => Some(Self::GetState),
            "set-state" => Some(Self::SetState),
            "condition" => Some(Self::Condition),
            "parallel" => Some(Self::Parallel),
            "timer" => Some(Self::Timer),
            "timeout" => Some(Self::Timeout),
            _ => None,
        }
    }

    pub const fn all() -> [Self; 14] {
        [
            Self::HttpHandler,
            Self::KafkaHandler,
            Self::CronTrigger,
            Self::WorkflowSubmit,
            Self::Run,
            Self::ServiceCall,
            Self::ObjectCall,
            Self::SendMessage,
            Self::GetState,
            Self::SetState,
            Self::Condition,
            Self::Parallel,
            Self::Timer,
            Self::Timeout,
        ]
    }
}

impl fmt::Display for NodeTemplateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for NodeTemplateId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown node template: {s}"))
    }
}

// ── TemplateDescriptor ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateDescriptor {
    pub id: NodeTemplateId,
    pub as_str: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

impl NodeTemplateId {
    pub fn descriptor(self) -> TemplateDescriptor {
        TemplateDescriptor {
            id: self,
            as_str: self.as_str(),
            label: self.label(),
            hint: self.hint(),
        }
    }
}

// ── TemplateCategory ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateCategory {
    Ingress,
    Execution,
    State,
    Control,
    Workflow,
}

impl TemplateCategory {
    pub fn members(self) -> &'static [NodeTemplateId] {
        match self {
            Self::Ingress => &[
                NodeTemplateId::HttpHandler,
                NodeTemplateId::KafkaHandler,
                NodeTemplateId::CronTrigger,
            ],
            Self::Execution => &[
                NodeTemplateId::Run,
                NodeTemplateId::ServiceCall,
                NodeTemplateId::ObjectCall,
                NodeTemplateId::SendMessage,
            ],
            Self::State => &[NodeTemplateId::GetState, NodeTemplateId::SetState],
            Self::Control => &[
                NodeTemplateId::Condition,
                NodeTemplateId::Parallel,
                NodeTemplateId::Timer,
                NodeTemplateId::Timeout,
            ],
            Self::Workflow => &[NodeTemplateId::WorkflowSubmit],
        }
    }

    pub const fn all() -> [Self; 5] {
        [
            Self::Ingress,
            Self::Execution,
            Self::State,
            Self::Control,
            Self::Workflow,
        ]
    }
}

impl NodeTemplateId {
    pub fn category(self) -> TemplateCategory {
        match self {
            Self::HttpHandler | Self::KafkaHandler | Self::CronTrigger => TemplateCategory::Ingress,
            Self::Run | Self::ServiceCall | Self::ObjectCall | Self::SendMessage => {
                TemplateCategory::Execution
            }
            Self::GetState | Self::SetState => TemplateCategory::State,
            Self::Condition | Self::Parallel | Self::Timer | Self::Timeout => {
                TemplateCategory::Control
            }
            Self::WorkflowSubmit => TemplateCategory::Workflow,
        }
    }
}

// ── Error Taxonomy ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    ParseError {
        input: String,
        expected: &'static str,
    },
    ValidationError {
        template_id: NodeTemplateId,
        violation: ValidationViolation,
    },
    RenderError {
        template_id: NodeTemplateId,
        context: RenderContext,
    },
    SerializationError {
        reason: SerializationReason,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError { input, expected } => {
                write!(f, "parse error: {input}: expected {expected}")
            }
            Self::ValidationError {
                template_id,
                violation,
            } => {
                write!(f, "validation error for {template_id}: {violation}")
            }
            Self::RenderError {
                template_id,
                context,
            } => {
                write!(f, "render error for {template_id} in {context:?}")
            }
            Self::SerializationError { reason } => {
                write!(f, "serialization error: {reason}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationViolation {
    MissingRequiredField(String),
    InvalidTemplateCombination(Vec<NodeTemplateId>),
    CircularDependency,
}

impl fmt::Display for ValidationViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredField(field) => write!(f, "missing required field: {field}"),
            Self::InvalidTemplateCombination(ids) => {
                let names: Vec<String> = ids.iter().map(|id| id.as_str().to_string()).collect();
                write!(f, "invalid template combination: {}", names.join(", "))
            }
            Self::CircularDependency => write!(f, "circular dependency detected"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderContext {
    Palette,
    CommandPalette,
    Canvas,
    Inspector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationReason {
    YamlEncodeError(String),
    JsonEncodeError(String),
    EmptySketch,
}

impl fmt::Display for SerializationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::YamlEncodeError(msg) => write!(f, "yaml encode error: {msg}"),
            Self::JsonEncodeError(msg) => write!(f, "json encode error: {msg}"),
            Self::EmptySketch => write!(f, "cannot serialize empty sketch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_valid_http_method_when_parsing_case_insensitive_then_correct_variant() {
        assert_eq!(HttpMethod::from_str_ignore_case("get"), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str_ignore_case("POST"), HttpMethod::Post);
        assert_eq!(HttpMethod::from_str_ignore_case("Patch"), HttpMethod::Patch);
    }

    #[test]
    fn given_invalid_http_method_when_parsing_then_defaults_to_post() {
        assert_eq!(
            HttpMethod::from_str_ignore_case("invalid"),
            HttpMethod::Post
        );
    }

    #[test]
    fn given_handle_kind_when_converting_to_string_then_correct_output() {
        assert_eq!(HandleKind::Source.as_str(), "source");
        assert_eq!(HandleKind::Target.as_str(), "target");
    }

    #[test]
    fn given_all_node_templates_when_counting_then_returns_14() {
        assert_eq!(NodeTemplateId::all().len(), 14);
    }

    #[test]
    fn given_node_template_when_getting_label_then_returns_readable_name() {
        assert_eq!(NodeTemplateId::HttpHandler.label(), "HTTP Handler");
        assert_eq!(NodeTemplateId::Condition.label(), "If / Else");
    }

    #[test]
    fn given_all_templates_when_collecting_as_str_then_all_are_unique() {
        let strs: Vec<&'static str> = NodeTemplateId::all().iter().map(|id| id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = strs.iter().copied().collect();
        assert_eq!(strs.len(), unique.len(), "as_str values must be unique");
    }

    #[test]
    fn given_all_templates_when_roundtripping_through_str_then_identity_holds() {
        for id in NodeTemplateId::all() {
            let s = id.as_str();
            let recovered = NodeTemplateId::from_str(s)
                .unwrap_or_else(|_| panic!("from_str({s:?}) returned None for {id:?}"));
            assert_eq!(recovered, id, "from_str(as_str({id:?})) != {id:?}");
        }
    }

    #[test]
    fn given_invalid_string_when_parsing_node_template_then_returns_none() {
        assert_eq!(NodeTemplateId::parse("nonexistent"), None);
        assert_eq!(NodeTemplateId::parse(""), None);
        assert_eq!(NodeTemplateId::parse("HTTP-HANDLER"), None);
    }

    #[test]
    fn given_all_templates_when_checking_labels_then_none_are_empty() {
        for id in NodeTemplateId::all() {
            assert!(
                !id.label().is_empty(),
                "label() for {id:?} must not be empty"
            );
        }
    }

    #[test]
    fn given_all_templates_when_checking_hints_then_none_are_empty() {
        for id in NodeTemplateId::all() {
            assert!(!id.hint().is_empty(), "hint() for {id:?} must not be empty");
        }
    }

    #[test]
    fn given_node_template_when_getting_descriptor_then_fields_match_template() {
        for id in NodeTemplateId::all() {
            let desc = id.descriptor();
            assert_eq!(desc.id, id);
            assert_eq!(desc.as_str, id.as_str());
            assert_eq!(desc.label, id.label());
            assert_eq!(desc.hint, id.hint());
        }
    }
}
