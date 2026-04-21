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

    // ── QA-MANUAL: ve-es45l — additional type validation coverage ──

    #[test]
    fn qa_http_method_from_str_strict_rejects_invalid() {
        use std::str::FromStr;
        assert!(HttpMethod::from_str("GET").is_ok());
        assert!(HttpMethod::from_str("get").is_ok());
        assert!(HttpMethod::from_str("invalid").is_err());
        assert!(HttpMethod::from_str("").is_err());
        let err = HttpMethod::from_str("bogus").unwrap_err();
        assert!(err.contains("Invalid HTTP method"));
    }

    #[test]
    fn qa_http_method_display_matches_as_str() {
        for m in HttpMethod::all() {
            assert_eq!(format!("{m}"), m.as_str());
        }
    }

    #[test]
    fn qa_http_method_default_is_post() {
        assert_eq!(HttpMethod::default(), HttpMethod::Post);
    }

    #[test]
    fn qa_http_method_parse_is_same_as_from_str_ignore_case() {
        assert_eq!(HttpMethod::parse("GET"), HttpMethod::from_str_ignore_case("GET"));
        assert_eq!(HttpMethod::parse("weird"), HttpMethod::from_str_ignore_case("weird"));
    }

    #[test]
    fn qa_http_method_all_covers_all_variants() {
        use std::collections::HashSet;
        let all: HashSet<HttpMethod> = HttpMethod::all().iter().copied().collect();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&HttpMethod::Get));
        assert!(all.contains(&HttpMethod::Post));
        assert!(all.contains(&HttpMethod::Put));
        assert!(all.contains(&HttpMethod::Delete));
        assert!(all.contains(&HttpMethod::Patch));
    }

    #[test]
    fn qa_http_method_from_str_ignore_case_empty_string_defaults_post() {
        assert_eq!(HttpMethod::from_str_ignore_case(""), HttpMethod::Post);
    }

    #[test]
    fn qa_http_method_all_as_str_values_are_unique() {
        use std::collections::HashSet;
        let strs: Vec<&'static str> = HttpMethod::all().iter().map(|m| m.as_str()).collect();
        let unique: HashSet<&str> = strs.iter().copied().collect();
        assert_eq!(strs.len(), unique.len());
    }

    #[test]
    fn qa_handle_kind_parse_valid_inputs() {
        assert_eq!(HandleKind::parse("source"), Some(HandleKind::Source));
        assert_eq!(HandleKind::parse("target"), Some(HandleKind::Target));
    }

    #[test]
    fn qa_handle_kind_parse_invalid_returns_none() {
        assert_eq!(HandleKind::parse(""), None);
        assert_eq!(HandleKind::parse("Source"), None);
        assert_eq!(HandleKind::parse("SOURCE"), None);
        assert_eq!(HandleKind::parse("input"), None);
        assert_eq!(HandleKind::parse("output"), None);
    }

    #[test]
    fn qa_handle_kind_from_str_delegates_to_parse() {
        use std::str::FromStr;
        assert_eq!(HandleKind::from_str("source").unwrap(), HandleKind::Source);
        let err = HandleKind::from_str("bogus").unwrap_err();
        assert!(err.contains("Invalid handle kind"));
    }

    #[test]
    fn qa_handle_kind_display_matches_as_str() {
        assert_eq!(format!("{}", HandleKind::Source), "source");
        assert_eq!(format!("{}", HandleKind::Target), "target");
    }

    #[test]
    fn qa_node_template_labels_are_unique() {
        use std::collections::HashSet;
        let labels: Vec<&'static str> = NodeTemplateId::all().iter().map(|id| id.label()).collect();
        let unique: HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(labels.len(), unique.len(), "labels must be unique");
    }

    #[test]
    fn qa_node_template_hints_are_unique() {
        use std::collections::HashSet;
        let hints: Vec<&'static str> = NodeTemplateId::all().iter().map(|id| id.hint()).collect();
        let unique: HashSet<&str> = hints.iter().copied().collect();
        assert_eq!(hints.len(), unique.len(), "hints must be unique");
    }

    #[test]
    fn qa_node_template_display_matches_as_str() {
        for id in NodeTemplateId::all() {
            assert_eq!(format!("{id}"), id.as_str());
        }
    }

    #[test]
    fn qa_node_template_from_str_invalid_cases() {
        use std::str::FromStr;
        assert!(NodeTemplateId::from_str("nonexistent").is_err());
        assert!(NodeTemplateId::from_str("").is_err());
        assert!(NodeTemplateId::from_str("HTTP-HANDLER").is_err());
        let err = NodeTemplateId::from_str("xyz").unwrap_err();
        assert!(err.contains("Unknown node template"));
    }

    #[test]
    fn qa_template_category_all_returns_five() {
        assert_eq!(TemplateCategory::all().len(), 5);
    }

    #[test]
    fn qa_template_category_members_covers_all_templates() {
        use std::collections::HashSet;
        let all: HashSet<NodeTemplateId> = NodeTemplateId::all().into_iter().collect();
        let categorized: HashSet<NodeTemplateId> = TemplateCategory::all()
            .iter()
            .flat_map(|cat| cat.members().iter().copied())
            .collect();
        assert_eq!(all, categorized, "every template must belong to exactly one category");
    }

    #[test]
    fn qa_template_category_roundtrip_symmetry() {
        for cat in TemplateCategory::all() {
            for id in cat.members() {
                assert_eq!(id.category(), cat, "category() must match TemplateCategory::members()");
            }
        }
    }

    #[test]
    fn qa_template_category_no_duplicate_members_across_categories() {
        use std::collections::HashSet;
        let mut seen: HashSet<NodeTemplateId> = HashSet::new();
        for cat in TemplateCategory::all() {
            for id in cat.members() {
                assert!(seen.insert(*id), "template {id:?} appears in multiple categories");
            }
        }
    }

    #[test]
    fn qa_template_error_parse_error_display() {
        let err = TemplateError::ParseError {
            input: "bad".to_string(),
            expected: "valid template id",
        };
        let s = format!("{err}");
        assert!(s.contains("parse error"));
        assert!(s.contains("bad"));
        assert!(s.contains("valid template id"));
    }

    #[test]
    fn qa_template_error_validation_error_display() {
        let err = TemplateError::ValidationError {
            template_id: NodeTemplateId::HttpHandler,
            violation: ValidationViolation::MissingRequiredField("url".to_string()),
        };
        let s = format!("{err}");
        assert!(s.contains("validation error"));
        assert!(s.contains("http-handler"));
        assert!(s.contains("missing required field"));
        assert!(s.contains("url"));
    }

    #[test]
    fn qa_template_error_render_error_display() {
        let err = TemplateError::RenderError {
            template_id: NodeTemplateId::Condition,
            context: RenderContext::Canvas,
        };
        let s = format!("{err}");
        assert!(s.contains("render error"));
        assert!(s.contains("condition"));
    }

    #[test]
    fn qa_template_error_serialization_error_display() {
        let err = TemplateError::SerializationError {
            reason: SerializationReason::EmptySketch,
        };
        let s = format!("{err}");
        assert!(s.contains("serialization error"));
        assert!(s.contains("cannot serialize empty sketch"));
    }

    #[test]
    fn qa_validation_violation_invalid_combination_display() {
        let v = ValidationViolation::InvalidTemplateCombination(vec![
            NodeTemplateId::HttpHandler,
            NodeTemplateId::Timer,
        ]);
        let s = format!("{v}");
        assert!(s.contains("invalid template combination"));
        assert!(s.contains("http-handler"));
        assert!(s.contains("timer"));
    }

    #[test]
    fn qa_validation_violation_circular_dependency_display() {
        let s = format!("{}", ValidationViolation::CircularDependency);
        assert!(s.contains("circular dependency"));
    }

    #[test]
    fn qa_serialization_reason_yaml_and_json_display() {
        let yaml = SerializationReason::YamlEncodeError("bad mapping".to_string());
        assert!(format!("{yaml}").contains("yaml encode error"));
        let json = SerializationReason::JsonEncodeError("unexpected token".to_string());
        assert!(format!("{json}").contains("json encode error"));
    }

    #[test]
    fn qa_hash_consistency_http_method() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        for m in HttpMethod::all() {
            assert!(set.insert(m), "duplicate hash for {m:?}");
        }
    }

    #[test]
    fn qa_hash_consistency_handle_kind() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        assert!(set.insert(HandleKind::Source));
        assert!(set.insert(HandleKind::Target));
    }

    #[test]
    fn qa_hash_consistency_node_template_id() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        for id in NodeTemplateId::all() {
            assert!(set.insert(id), "duplicate hash for {id:?}");
        }
    }

    #[test]
    fn qa_hash_consistency_template_category() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        for cat in TemplateCategory::all() {
            assert!(set.insert(cat), "duplicate hash for {cat:?}");
        }
    }

    #[test]
    fn qa_clone_copy_correctness() {
        let m = HttpMethod::Get;
        let m2 = m;
        assert_eq!(m, m2);

        let h = HandleKind::Source;
        let h2 = h;
        assert_eq!(h, h2);

        let n = NodeTemplateId::Run;
        let n2 = n;
        assert_eq!(n, n2);

        let c = TemplateCategory::Ingress;
        let c2 = c;
        assert_eq!(c, c2);
    }

    #[test]
    fn qa_partial_eq_negatives() {
        assert_ne!(HttpMethod::Get, HttpMethod::Post);
        assert_ne!(HandleKind::Source, HandleKind::Target);
        assert_ne!(NodeTemplateId::HttpHandler, NodeTemplateId::Timer);
        assert_ne!(TemplateCategory::Ingress, TemplateCategory::State);
    }

    #[test]
    fn qa_template_descriptor_copy_and_eq() {
        let d1 = NodeTemplateId::ServiceCall.descriptor();
        let d2 = d1;
        assert_eq!(d1, d2);
        assert_eq!(d1.id, NodeTemplateId::ServiceCall);
    }

    #[test]
    fn qa_node_template_parse_case_sensitive() {
        assert_eq!(NodeTemplateId::parse("http-handler"), Some(NodeTemplateId::HttpHandler));
        assert_eq!(NodeTemplateId::parse("Http-Handler"), None);
        assert_eq!(NodeTemplateId::parse("HTTP-HANDLER"), None);
        assert_eq!(NodeTemplateId::parse("Run"), None);
        assert_eq!(NodeTemplateId::parse("run"), Some(NodeTemplateId::Run));
    }

    #[test]
    fn qa_render_context_debug_impl_exists() {
        let _ = format!("{:?}", RenderContext::Palette);
        let _ = format!("{:?}", RenderContext::CommandPalette);
        let _ = format!("{:?}", RenderContext::Canvas);
        let _ = format!("{:?}", RenderContext::Inspector);
    }

    #[test]
    fn qa_render_context_all_four_variants_match() {
        let all = [RenderContext::Palette, RenderContext::CommandPalette, RenderContext::Canvas, RenderContext::Inspector];
        assert_eq!(all.len(), 4);
    }
}
