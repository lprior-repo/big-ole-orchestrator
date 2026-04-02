use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
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

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Post
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

    pub fn from_str(s: &str) -> Option<Self> {
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
        Self::from_str(s).ok_or_else(|| format!("Invalid handle kind: {s}"))
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
    Sleep,
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
            Self::Sleep => "sleep",
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
            Self::Sleep => "Sleep / Timer",
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
            Self::Sleep => "Pause execution durably",
            Self::Timeout => "Guard a step with deadline",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
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
            "sleep" => Some(Self::Sleep),
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
            Self::Sleep,
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
        Self::from_str(s).ok_or_else(|| format!("Unknown node template: {s}"))
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
}
