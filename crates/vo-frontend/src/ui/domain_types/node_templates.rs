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

impl std::str::FromStr for NodeTemplateId {
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
