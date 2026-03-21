#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::restate_types::{ContextType, PortType, ServiceKind};
use super::NodeCategory;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WorkflowNode {
    HttpHandler(HttpHandlerConfig),
    KafkaHandler(KafkaHandlerConfig),
    CronTrigger(CronTriggerConfig),
    WorkflowSubmit(WorkflowSubmitConfig),
    Run(RunConfig),
    ServiceCall(ServiceCallConfig),
    ObjectCall(ObjectCallConfig),
    WorkflowCall(WorkflowCallConfig),
    SendMessage(SendMessageConfig),
    DelayedSend(DelayedSendConfig),
    GetState(GetStateConfig),
    SetState(SetStateConfig),
    ClearState(ClearStateConfig),
    Condition(ConditionConfig),
    Switch(SwitchConfig),
    Loop(LoopConfig),
    Parallel(ParallelConfig),
    Compensate(CompensateConfig),
    Sleep(SleepConfig),
    Timeout(TimeoutConfig),
    DurablePromise(DurablePromiseConfig),
    Awakeable(AwakeableConfig),
    ResolvePromise(ResolvePromiseConfig),
    SignalHandler(SignalHandlerConfig),
    CtxActivity(CtxActivityConfig),
    CtxSleep(CtxSleepConfig),
    CtxWaitSignal(CtxWaitSignalConfig),
    CtxNow(CtxNowConfig),
}

impl Default for WorkflowNode {
    fn default() -> Self {
        Self::Run(RunConfig::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HttpHandlerConfig {
    pub path: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KafkaHandlerConfig {
    pub topic: Option<String>,
    pub consumer_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CronTriggerConfig {
    pub schedule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkflowSubmitConfig {
    pub workflow_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RunConfig {
    pub durable_step_name: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ServiceCallConfig {
    pub durable_step_name: Option<String>,
    pub service: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ObjectCallConfig {
    pub durable_step_name: Option<String>,
    pub object_name: Option<String>,
    pub handler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkflowCallConfig {
    pub durable_step_name: Option<String>,
    pub workflow_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SendMessageConfig {
    pub durable_step_name: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DelayedSendConfig {
    pub delay_ms: Option<u64>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GetStateConfig {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SetStateConfig {
    pub key: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClearStateConfig {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConditionConfig {
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SwitchConfig {
    pub expression: Option<String>,
    pub cases: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LoopConfig {
    pub iterator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ParallelConfig {
    pub branches: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CompensateConfig {
    pub target_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SleepConfig {
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TimeoutConfig {
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DurablePromiseConfig {
    pub promise_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AwakeableConfig {
    pub awakeable_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResolvePromiseConfig {
    pub promise_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SignalHandlerConfig {
    pub signal_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CtxActivityConfig {
    pub name: Option<String>,
    pub activity_type: Option<String>,
    pub input_schema: Option<String>,
}

impl Default for CtxActivityConfig {
    fn default() -> Self {
        Self {
            name: None,
            activity_type: None,
            input_schema: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CtxSleepConfig {
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CtxWaitSignalConfig {
    pub signal_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CtxNowConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionResult {
    True,
    False,
}

impl ConditionResult {
    #[must_use]
    pub const fn is_true(self) -> bool {
        matches!(self, Self::True)
    }

    #[must_use]
    pub const fn is_false(self) -> bool {
        matches!(self, Self::False)
    }

    #[must_use]
    pub const fn branch_port(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::False => "false",
        }
    }

    #[must_use]
    pub const fn opposite_port(self) -> &'static str {
        match self {
            Self::True => "false",
            Self::False => "true",
        }
    }
}

impl From<bool> for ConditionResult {
    fn from(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }
}

impl From<ConditionResult> for bool {
    fn from(value: ConditionResult) -> Self {
        value.is_true()
    }
}

impl fmt::Display for ConditionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::True => write!(f, "true"),
            Self::False => write!(f, "false"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl HttpMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = UnknownHttpMethodError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "PATCH" => Ok(Self::Patch),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            _ => Err(UnknownHttpMethodError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownHttpMethodError(pub String);

impl fmt::Display for UnknownHttpMethodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown HTTP method: {}", self.0)
    }
}

impl std::error::Error for UnknownHttpMethodError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownNodeTypeError(pub String);

impl std::fmt::Display for UnknownNodeTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown node type: {}", self.0)
    }
}

impl std::error::Error for UnknownNodeTypeError {}

impl WorkflowNode {
    #[must_use]
    pub const fn category(&self) -> NodeCategory {
        match self {
            Self::HttpHandler(_)
            | Self::KafkaHandler(_)
            | Self::CronTrigger(_)
            | Self::WorkflowSubmit(_) => NodeCategory::Entry,
            Self::Run(_)
            | Self::ServiceCall(_)
            | Self::ObjectCall(_)
            | Self::WorkflowCall(_)
            | Self::SendMessage(_)
            | Self::DelayedSend(_)
            | Self::CtxActivity(_) => NodeCategory::Durable,
            Self::GetState(_) | Self::SetState(_) | Self::ClearState(_) => NodeCategory::State,
            Self::Condition(_)
            | Self::Switch(_)
            | Self::Loop(_)
            | Self::Parallel(_)
            | Self::Compensate(_) => NodeCategory::Flow,
            Self::Sleep(_) | Self::Timeout(_) | Self::CtxSleep(_) | Self::CtxNow(_) => {
                NodeCategory::Timing
            }
            Self::DurablePromise(_)
            | Self::Awakeable(_)
            | Self::ResolvePromise(_)
            | Self::SignalHandler(_)
            | Self::CtxWaitSignal(_) => NodeCategory::Signal,
        }
    }

    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::HttpHandler(_) => "globe",
            Self::KafkaHandler(_) => "kafka",
            Self::CronTrigger(_) => "clock",
            Self::WorkflowSubmit(_) => "play",
            Self::Run(_) => "code",
            Self::ServiceCall(_) => "call",
            Self::ObjectCall(_) => "box",
            Self::WorkflowCall(_) => "git-branch",
            Self::SendMessage(_) => "send",
            Self::DelayedSend(_) => "clock",
            Self::GetState(_) => "database",
            Self::SetState(_) => "save",
            Self::ClearState(_) => "trash",
            Self::Condition(_) => "git-branch",
            Self::Switch(_) => "git-merge",
            Self::Loop(_) => "repeat",
            Self::Parallel(_) => "layers",
            Self::Compensate(_) => "undo",
            Self::Sleep(_) => "moon",
            Self::Timeout(_) => "alert-triangle",
            Self::DurablePromise(_) => "target",
            Self::Awakeable(_) => "radio",
            Self::ResolvePromise(_) => "check-circle",
            Self::SignalHandler(_) => "bell",
            Self::CtxActivity(_) => "zap",
            Self::CtxSleep(_) => "moon",
            Self::CtxWaitSignal(_) => "bell",
            Self::CtxNow(_) => "clock",
        }
    }

    #[must_use]
    pub const fn is_entry_point(&self) -> bool {
        matches!(
            self,
            Self::HttpHandler(_)
                | Self::KafkaHandler(_)
                | Self::CronTrigger(_)
                | Self::WorkflowSubmit(_)
        )
    }

    #[must_use]
    pub const fn needs_durable_step_name(&self) -> bool {
        matches!(
            self,
            Self::Run(_)
                | Self::ServiceCall(_)
                | Self::ObjectCall(_)
                | Self::WorkflowCall(_)
                | Self::SendMessage(_)
        )
    }

    #[must_use]
    pub fn compatible_service_kinds(&self) -> Vec<ServiceKind> {
        match self {
            Self::GetState(_) | Self::SetState(_) | Self::ClearState(_) => {
                vec![ServiceKind::Actor, ServiceKind::Workflow]
            }
            Self::DurablePromise(_) | Self::Awakeable(_) | Self::ResolvePromise(_) => {
                vec![ServiceKind::Workflow]
            }
            _ => vec![
                ServiceKind::Handler,
                ServiceKind::Actor,
                ServiceKind::Workflow,
            ],
        }
    }

    #[must_use]
    pub fn required_context_types(&self) -> Vec<ContextType> {
        match self {
            Self::GetState(_) | Self::SetState(_) | Self::ClearState(_) => {
                vec![ContextType::Synchronous]
            }
            Self::DurablePromise(_) | Self::Awakeable(_) | Self::ResolvePromise(_) => {
                vec![ContextType::Synchronous]
            }
            _ => vec![ContextType::Synchronous, ContextType::Asynchronous],
        }
    }

    #[must_use]
    pub const fn input_port_type(&self) -> PortType {
        match self {
            Self::SignalHandler(_) => PortType::Signal,
            _ => PortType::Any,
        }
    }

    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::HttpHandler(_) => "HTTP Handler",
            Self::KafkaHandler(_) => "Kafka Consumer",
            Self::CronTrigger(_) => "Cron Trigger",
            Self::WorkflowSubmit(_) => "Workflow Submit",
            Self::Run(_) => "Durable Step",
            Self::ServiceCall(_) => "Service Call",
            Self::ObjectCall(_) => "Object Call",
            Self::WorkflowCall(_) => "Workflow Call",
            Self::SendMessage(_) => "Send Message",
            Self::DelayedSend(_) => "Delayed Message",
            Self::GetState(_) => "Get State",
            Self::SetState(_) => "Set State",
            Self::ClearState(_) => "Clear State",
            Self::Condition(_) => "If / Else",
            Self::Switch(_) => "Switch",
            Self::Loop(_) => "Loop / Iterate",
            Self::Parallel(_) => "Parallel",
            Self::Compensate(_) => "Compensate",
            Self::Sleep(_) => "Sleep / Timer",
            Self::Timeout(_) => "Timeout",
            Self::DurablePromise(_) => "Durable Promise",
            Self::Awakeable(_) => "Awakeable",
            Self::ResolvePromise(_) => "Resolve Promise",
            Self::SignalHandler(_) => "Signal Handler",
            Self::CtxActivity(_) => "Activity",
            Self::CtxSleep(_) => "Sleep",
            Self::CtxWaitSignal(_) => "Wait Signal",
            Self::CtxNow(_) => "Now",
        }
    }

    #[must_use]
    pub const fn output_port_type(&self) -> PortType {
        match self {
            Self::HttpHandler(_) | Self::KafkaHandler(_) | Self::CronTrigger(_) => PortType::Event,
            Self::Run(_) | Self::ServiceCall(_) | Self::ObjectCall(_) | Self::WorkflowCall(_) => {
                PortType::Json
            }
            Self::GetState(_) => PortType::State,
            Self::Condition(_) | Self::Switch(_) => PortType::FlowControl,
            Self::DurablePromise(_)
            | Self::Awakeable(_)
            | Self::ResolvePromise(_)
            | Self::SignalHandler(_)
            | Self::CtxWaitSignal(_) => PortType::Signal,
            _ => PortType::Any,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn set_metadata(&mut self, _key: &str, _value: serde_json::Value) {}

    #[must_use]
    pub fn get_metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

impl fmt::Display for WorkflowNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_str = match self {
            Self::HttpHandler(_) => "http-handler",
            Self::KafkaHandler(_) => "kafka-handler",
            Self::CronTrigger(_) => "cron-trigger",
            Self::WorkflowSubmit(_) => "workflow-submit",
            Self::Run(_) => "run",
            Self::ServiceCall(_) => "service-call",
            Self::ObjectCall(_) => "object-call",
            Self::WorkflowCall(_) => "workflow-call",
            Self::SendMessage(_) => "send-message",
            Self::DelayedSend(_) => "delayed-send",
            Self::GetState(_) => "get-state",
            Self::SetState(_) => "set-state",
            Self::ClearState(_) => "clear-state",
            Self::Condition(_) => "condition",
            Self::Switch(_) => "switch",
            Self::Loop(_) => "loop",
            Self::Parallel(_) => "parallel",
            Self::Compensate(_) => "compensate",
            Self::Sleep(_) => "sleep",
            Self::Timeout(_) => "timeout",
            Self::DurablePromise(_) => "durable-promise",
            Self::Awakeable(_) => "awakeable",
            Self::ResolvePromise(_) => "resolve-promise",
            Self::SignalHandler(_) => "signal-handler",
            Self::CtxActivity(_) => "ctx-activity",
            Self::CtxSleep(_) => "ctx-sleep",
            Self::CtxWaitSignal(_) => "ctx-wait-signal",
            Self::CtxNow(_) => "ctx-now",
        };
        write!(f, "{type_str}")
    }
}

impl FromStr for WorkflowNode {
    type Err = UnknownNodeTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http-handler" => Ok(Self::HttpHandler(HttpHandlerConfig::default())),
            "kafka-handler" => Ok(Self::KafkaHandler(KafkaHandlerConfig::default())),
            "cron-trigger" => Ok(Self::CronTrigger(CronTriggerConfig::default())),
            "workflow-submit" => Ok(Self::WorkflowSubmit(WorkflowSubmitConfig::default())),
            "run" => Ok(Self::Run(RunConfig::default())),
            "service-call" => Ok(Self::ServiceCall(ServiceCallConfig::default())),
            "object-call" => Ok(Self::ObjectCall(ObjectCallConfig::default())),
            "workflow-call" => Ok(Self::WorkflowCall(WorkflowCallConfig::default())),
            "send-message" => Ok(Self::SendMessage(SendMessageConfig::default())),
            "delayed-send" => Ok(Self::DelayedSend(DelayedSendConfig::default())),
            "get-state" => Ok(Self::GetState(GetStateConfig::default())),
            "set-state" => Ok(Self::SetState(SetStateConfig::default())),
            "clear-state" => Ok(Self::ClearState(ClearStateConfig::default())),
            "condition" => Ok(Self::Condition(ConditionConfig::default())),
            "switch" => Ok(Self::Switch(SwitchConfig::default())),
            "loop" => Ok(Self::Loop(LoopConfig::default())),
            "parallel" => Ok(Self::Parallel(ParallelConfig::default())),
            "compensate" => Ok(Self::Compensate(CompensateConfig::default())),
            "sleep" => Ok(Self::Sleep(SleepConfig::default())),
            "timeout" => Ok(Self::Timeout(TimeoutConfig::default())),
            "durable-promise" => Ok(Self::DurablePromise(DurablePromiseConfig::default())),
            "awakeable" => Ok(Self::Awakeable(AwakeableConfig::default())),
            "resolve-promise" => Ok(Self::ResolvePromise(ResolvePromiseConfig::default())),
            "signal-handler" => Ok(Self::SignalHandler(SignalHandlerConfig::default())),
            "ctx-activity" => Ok(Self::CtxActivity(CtxActivityConfig::default())),
            "ctx-sleep" => Ok(Self::CtxSleep(CtxSleepConfig { duration_ms: 0 })),
            "ctx-wait-signal" => Ok(Self::CtxWaitSignal(CtxWaitSignalConfig {
                signal_name: String::new(),
            })),
            "ctx-now" => Ok(Self::CtxNow(CtxNowConfig)),
            _ => Err(UnknownNodeTypeError(s.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn all_node_types() -> &'static [&'static str] {
        &[
            "http-handler",
            "kafka-handler",
            "cron-trigger",
            "workflow-submit",
            "run",
            "service-call",
            "object-call",
            "workflow-call",
            "send-message",
            "delayed-send",
            "get-state",
            "set-state",
            "clear-state",
            "condition",
            "switch",
            "loop",
            "parallel",
            "compensate",
            "sleep",
            "timeout",
            "durable-promise",
            "awakeable",
            "resolve-promise",
            "signal-handler",
            "ctx-activity",
            "ctx-sleep",
            "ctx-wait-signal",
            "ctx-now",
        ]
    }

    mod happy_path {
        use super::*;

        #[test]
        fn given_all_28_node_types_when_parsing_then_each_maps_to_variant() {
            let node_types = all_node_types();
            assert_eq!(node_types.len(), 28, "Expected 28 node types");

            for node_type in node_types {
                let result = WorkflowNode::from_str(node_type);
                assert!(
                    result.is_ok(),
                    "Failed to parse '{}': {:?}",
                    node_type,
                    result.err()
                );
            }
        }

        #[test]
        fn given_entry_variants_when_getting_category_then_returns_entry() {
            let entry_nodes = [
                WorkflowNode::HttpHandler(HttpHandlerConfig::default()),
                WorkflowNode::KafkaHandler(KafkaHandlerConfig::default()),
                WorkflowNode::CronTrigger(CronTriggerConfig::default()),
                WorkflowNode::WorkflowSubmit(WorkflowSubmitConfig::default()),
            ];

            for node in entry_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::Entry,
                    "Expected Entry category for {node:?}"
                );
            }
        }

        #[test]
        fn given_durable_variants_when_getting_category_then_returns_durable() {
            let durable_nodes = [
                WorkflowNode::Run(RunConfig::default()),
                WorkflowNode::ServiceCall(ServiceCallConfig::default()),
                WorkflowNode::ObjectCall(ObjectCallConfig::default()),
                WorkflowNode::WorkflowCall(WorkflowCallConfig::default()),
                WorkflowNode::SendMessage(SendMessageConfig::default()),
                WorkflowNode::DelayedSend(DelayedSendConfig::default()),
            ];

            for node in durable_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::Durable,
                    "Expected Durable category for {node:?}"
                );
            }
        }

        #[test]
        fn given_state_variants_when_getting_category_then_returns_state() {
            let state_nodes = [
                WorkflowNode::GetState(GetStateConfig::default()),
                WorkflowNode::SetState(SetStateConfig::default()),
                WorkflowNode::ClearState(ClearStateConfig::default()),
            ];

            for node in state_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::State,
                    "Expected State category for {node:?}"
                );
            }
        }

        #[test]
        fn given_flow_variants_when_getting_category_then_returns_flow() {
            let flow_nodes = [
                WorkflowNode::Condition(ConditionConfig::default()),
                WorkflowNode::Switch(SwitchConfig::default()),
                WorkflowNode::Loop(LoopConfig::default()),
                WorkflowNode::Parallel(ParallelConfig::default()),
                WorkflowNode::Compensate(CompensateConfig::default()),
            ];

            for node in flow_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::Flow,
                    "Expected Flow category for {node:?}"
                );
            }
        }

        #[test]
        fn given_timing_variants_when_getting_category_then_returns_timing() {
            let timing_nodes = [
                WorkflowNode::Sleep(SleepConfig::default()),
                WorkflowNode::Timeout(TimeoutConfig::default()),
            ];

            for node in timing_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::Timing,
                    "Expected Timing category for {node:?}"
                );
            }
        }

        #[test]
        fn given_signal_variants_when_getting_category_then_returns_signal() {
            let signal_nodes = [
                WorkflowNode::DurablePromise(DurablePromiseConfig::default()),
                WorkflowNode::Awakeable(AwakeableConfig::default()),
                WorkflowNode::ResolvePromise(ResolvePromiseConfig::default()),
                WorkflowNode::SignalHandler(SignalHandlerConfig::default()),
            ];

            for node in signal_nodes {
                assert_eq!(
                    node.category(),
                    NodeCategory::Signal,
                    "Expected Signal category for {node:?}"
                );
            }
        }

        #[test]
        fn given_http_handler_when_getting_icon_then_returns_globe() {
            let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
            assert_eq!(node.icon(), "globe");
        }

        #[test]
        fn given_kafka_handler_when_getting_icon_then_returns_kafka() {
            let node = WorkflowNode::KafkaHandler(KafkaHandlerConfig::default());
            assert_eq!(node.icon(), "kafka");
        }

        #[test]
        fn given_cron_trigger_when_getting_icon_then_returns_clock() {
            let node = WorkflowNode::CronTrigger(CronTriggerConfig::default());
            assert_eq!(node.icon(), "clock");
        }

        #[test]
        fn given_workflow_node_with_config_when_serializing_then_produces_valid_json() {
            let node = WorkflowNode::HttpHandler(HttpHandlerConfig {
                path: Some("/api/test".to_string()),
                method: Some("GET".to_string()),
            });

            let json = serde_json::to_string(&node).expect("Serialization should succeed");
            assert!(
                json.contains(r#""type":"http-handler""#),
                "JSON should contain type field"
            );
            assert!(json.contains("/api/test"), "JSON should contain config");
        }

        #[test]
        fn given_valid_json_with_type_field_when_deserializing_then_produces_correct_variant() {
            let json = r#"{"type":"http-handler","path":"/api/users","method":"POST"}"#;
            let node: WorkflowNode =
                serde_json::from_str(json).expect("Deserialization should succeed");

            match node {
                WorkflowNode::HttpHandler(config) => {
                    assert_eq!(config.path, Some("/api/users".to_string()));
                    assert_eq!(config.method, Some("POST".to_string()));
                }
                _ => panic!("Expected HttpHandler variant"),
            }
        }
    }

    mod error_path {
        use super::*;

        #[test]
        fn given_empty_string_when_parsing_then_returns_error() {
            let result = WorkflowNode::from_str("");
            assert!(result.is_err(), "Empty string should return error");
        }

        #[test]
        fn given_unknown_type_when_parsing_then_returns_error() {
            let result = WorkflowNode::from_str("foo-bar");
            assert!(result.is_err(), "Unknown type should return error");
        }

        #[test]
        fn given_typo_when_parsing_then_returns_error() {
            let result = WorkflowNode::from_str("http-handlr");
            assert!(result.is_err(), "Typo should return error");
        }

        #[test]
        fn given_case_mismatch_when_parsing_then_returns_error() {
            let result = WorkflowNode::from_str("HTTP-HANDLER");
            assert!(result.is_err(), "Case mismatch should return error");
        }

        #[test]
        fn given_json_with_unknown_type_when_deserializing_then_returns_error() {
            let json = r#"{"type":"unknown-type"}"#;
            let result: Result<WorkflowNode, _> = serde_json::from_str(json);
            assert!(result.is_err(), "Unknown type in JSON should return error");
        }

        #[test]
        fn given_json_missing_type_field_when_deserializing_then_returns_error() {
            let json = r#"{"path":"/api/test"}"#;
            let result: Result<WorkflowNode, _> = serde_json::from_str(json);
            assert!(result.is_err(), "Missing type field should return error");
        }
    }

    mod edge_case {
        use super::*;

        #[test]
        fn given_empty_config_when_creating_variant_then_succeeds_with_defaults() {
            let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
            let json = serde_json::to_string(&node).expect("Should serialize");
            assert!(json.contains("http-handler"));
        }

        #[test]
        fn given_all_variants_when_counting_then_is_28() {
            assert_eq!(all_node_types().len(), 28);
        }

        #[test]
        fn given_http_handler_when_displaying_then_outputs_kebab_case() {
            let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
            let display = format!("{node}");
            assert_eq!(display, "http-handler");
        }

        #[test]
        fn given_run_when_displaying_then_outputs_kebab_case() {
            let node = WorkflowNode::Run(RunConfig::default());
            let display = format!("{node}");
            assert_eq!(display, "run");
        }

        #[test]
        fn given_durable_promise_when_displaying_then_outputs_kebab_case() {
            let node = WorkflowNode::DurablePromise(DurablePromiseConfig::default());
            let display = format!("{node}");
            assert_eq!(display, "durable-promise");
        }
    }

    mod contract {
        use super::*;

        #[test]
        fn given_variant_when_serializing_then_uses_type_tag() {
            let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
            let json = serde_json::to_string(&node).expect("Should serialize");
            assert!(
                json.contains(r#""type":"http-handler""#),
                "Should use 'type' as tag"
            );
        }

        #[test]
        fn given_http_handler_when_roundtrip_then_preserves_data() {
            let original = WorkflowNode::HttpHandler(HttpHandlerConfig {
                path: Some("/test".to_string()),
                method: Some("GET".to_string()),
            });

            let json = serde_json::to_string(&original).expect("Should serialize");
            let restored: WorkflowNode = serde_json::from_str(&json).expect("Should deserialize");

            assert_eq!(original, restored);
        }

        #[test]
        fn given_run_with_config_when_roundtrip_then_preserves_data() {
            let original = WorkflowNode::Run(RunConfig {
                durable_step_name: Some("step-1".to_string()),
                code: Some("println!(\"hello\")".to_string()),
            });

            let json = serde_json::to_string(&original).expect("Should serialize");
            let restored: WorkflowNode = serde_json::from_str(&json).expect("Should deserialize");

            assert_eq!(original, restored);
        }

        #[test]
        fn given_all_24_variants_when_roundtrip_then_all_succeed() {
            for node_type in all_node_types() {
                let original = WorkflowNode::from_str(node_type)
                    .unwrap_or_else(|_| panic!("Should parse {node_type}"));

                let json = serde_json::to_string(&original)
                    .unwrap_or_else(|_| panic!("Should serialize {node_type}"));

                let restored: WorkflowNode = serde_json::from_str(&json)
                    .unwrap_or_else(|_| panic!("Should deserialize {node_type}"));

                assert_eq!(original, restored, "Roundtrip failed for {node_type}");
            }
        }

        #[test]
        fn given_from_str_result_when_displaying_then_is_inverse() {
            for node_type in all_node_types() {
                let node = WorkflowNode::from_str(node_type)
                    .unwrap_or_else(|_| panic!("Should parse {node_type}"));

                let display = format!("{node}");
                assert_eq!(
                    &display, *node_type,
                    "Display not inverse of FromStr for {node_type}"
                );
            }
        }
    }

    mod flow_extender_integration {
        use super::*;

        #[test]
        fn given_entry_variants_when_checking_is_entry_point_then_returns_true() {
            let entry_nodes = [
                WorkflowNode::HttpHandler(HttpHandlerConfig::default()),
                WorkflowNode::KafkaHandler(KafkaHandlerConfig::default()),
                WorkflowNode::CronTrigger(CronTriggerConfig::default()),
                WorkflowNode::WorkflowSubmit(WorkflowSubmitConfig::default()),
            ];

            for node in entry_nodes {
                assert!(node.is_entry_point(), "{node:?} should be entry point");
            }
        }

        #[test]
        fn given_non_entry_variants_when_checking_is_entry_point_then_returns_false() {
            let non_entry_nodes: Vec<WorkflowNode> = vec![
                WorkflowNode::Run(RunConfig::default()),
                WorkflowNode::ServiceCall(ServiceCallConfig::default()),
                WorkflowNode::GetState(GetStateConfig::default()),
                WorkflowNode::Condition(ConditionConfig::default()),
                WorkflowNode::Sleep(SleepConfig::default()),
                WorkflowNode::DurablePromise(DurablePromiseConfig::default()),
            ];

            for node in non_entry_nodes {
                assert!(!node.is_entry_point(), "{node:?} should not be entry point");
            }
        }

        #[test]
        fn given_durable_call_variants_when_checking_needs_durable_step_name_then_returns_true() {
            let durable_call_nodes = [
                WorkflowNode::Run(RunConfig::default()),
                WorkflowNode::ServiceCall(ServiceCallConfig::default()),
                WorkflowNode::ObjectCall(ObjectCallConfig::default()),
                WorkflowNode::WorkflowCall(WorkflowCallConfig::default()),
                WorkflowNode::SendMessage(SendMessageConfig::default()),
            ];

            for node in durable_call_nodes {
                assert!(
                    node.needs_durable_step_name(),
                    "{node:?} needs durable step name"
                );
            }
        }

        #[test]
        fn given_non_durable_variants_when_checking_needs_durable_step_name_then_returns_false() {
            let non_durable_nodes = [
                WorkflowNode::HttpHandler(HttpHandlerConfig::default()),
                WorkflowNode::GetState(GetStateConfig::default()),
                WorkflowNode::Condition(ConditionConfig::default()),
                WorkflowNode::Sleep(SleepConfig::default()),
            ];

            for node in non_durable_nodes {
                assert!(
                    !node.needs_durable_step_name(),
                    "{node:?} does not need durable step name"
                );
            }
        }
    }

    mod restate_service_kinds {
        use super::*;

        mod compatible_service_kinds {
            use super::*;

            #[test]
            fn http_handler_supports_all_service_kinds() {
                let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn kafka_handler_supports_all_service_kinds() {
                let node = WorkflowNode::KafkaHandler(KafkaHandlerConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn cron_trigger_supports_all_service_kinds() {
                let node = WorkflowNode::CronTrigger(CronTriggerConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn workflow_submit_supports_all_service_kinds() {
                let node = WorkflowNode::WorkflowSubmit(WorkflowSubmitConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn get_state_supports_virtual_object_and_workflow_only() {
                let node = WorkflowNode::GetState(GetStateConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn set_state_supports_virtual_object_and_workflow_only() {
                let node = WorkflowNode::SetState(SetStateConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn clear_state_supports_virtual_object_and_workflow_only() {
                let node = WorkflowNode::ClearState(ClearStateConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn durable_promise_supports_workflow_only() {
                let node = WorkflowNode::DurablePromise(DurablePromiseConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(!kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn awakeable_supports_workflow_only() {
                let node = WorkflowNode::Awakeable(AwakeableConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(!kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn resolve_promise_supports_workflow_only() {
                let node = WorkflowNode::ResolvePromise(ResolvePromiseConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(!kinds.contains(&ServiceKind::Handler));
                assert!(!kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn run_supports_all_service_kinds() {
                let node = WorkflowNode::Run(RunConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn service_call_supports_all_service_kinds() {
                let node = WorkflowNode::ServiceCall(ServiceCallConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn object_call_supports_all_service_kinds() {
                let node = WorkflowNode::ObjectCall(ObjectCallConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn workflow_call_supports_all_service_kinds() {
                let node = WorkflowNode::WorkflowCall(WorkflowCallConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn condition_supports_all_service_kinds() {
                let node = WorkflowNode::Condition(ConditionConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn switch_supports_all_service_kinds() {
                let node = WorkflowNode::Switch(SwitchConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn loop_supports_all_service_kinds() {
                let node = WorkflowNode::Loop(LoopConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn parallel_supports_all_service_kinds() {
                let node = WorkflowNode::Parallel(ParallelConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn compensate_supports_all_service_kinds() {
                let node = WorkflowNode::Compensate(CompensateConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn sleep_supports_all_service_kinds() {
                let node = WorkflowNode::Sleep(SleepConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn timeout_supports_all_service_kinds() {
                let node = WorkflowNode::Timeout(TimeoutConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn send_message_supports_all_service_kinds() {
                let node = WorkflowNode::SendMessage(SendMessageConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn delayed_send_supports_all_service_kinds() {
                let node = WorkflowNode::DelayedSend(DelayedSendConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }

            #[test]
            fn signal_handler_supports_all_service_kinds() {
                let node = WorkflowNode::SignalHandler(SignalHandlerConfig::default());
                let kinds = node.compatible_service_kinds();
                assert!(kinds.contains(&ServiceKind::Handler));
                assert!(kinds.contains(&ServiceKind::Actor));
                assert!(kinds.contains(&ServiceKind::Workflow));
            }
        }

        mod required_context_types {
            use super::*;

            #[test]
            fn http_handler_returns_service_context() {
                let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn kafka_handler_returns_service_context() {
                let node = WorkflowNode::KafkaHandler(KafkaHandlerConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn cron_trigger_returns_service_context() {
                let node = WorkflowNode::CronTrigger(CronTriggerConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn workflow_submit_returns_service_context() {
                let node = WorkflowNode::WorkflowSubmit(WorkflowSubmitConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn get_state_returns_synchronous_context() {
                let node = WorkflowNode::GetState(GetStateConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn set_state_returns_synchronous_context() {
                let node = WorkflowNode::SetState(SetStateConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn clear_state_returns_synchronous_context() {
                let node = WorkflowNode::ClearState(ClearStateConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn durable_promise_returns_synchronous_context() {
                let node = WorkflowNode::DurablePromise(DurablePromiseConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn awakeable_returns_synchronous_context() {
                let node = WorkflowNode::Awakeable(AwakeableConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn resolve_promise_returns_synchronous_context() {
                let node = WorkflowNode::ResolvePromise(ResolvePromiseConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn signal_handler_returns_synchronous_context() {
                let node = WorkflowNode::SignalHandler(SignalHandlerConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }

            #[test]
            fn run_returns_service_context() {
                let node = WorkflowNode::Run(RunConfig::default());
                let ctx_types = node.required_context_types();
                assert!(ctx_types.contains(&ContextType::Synchronous));
            }
        }

        mod entry_nodes {
            use super::*;

            #[test]
            fn all_entry_nodes_have_all_service_kinds() {
                let entry_nodes: Vec<WorkflowNode> = vec![
                    WorkflowNode::HttpHandler(HttpHandlerConfig::default()),
                    WorkflowNode::KafkaHandler(KafkaHandlerConfig::default()),
                    WorkflowNode::CronTrigger(CronTriggerConfig::default()),
                    WorkflowNode::WorkflowSubmit(WorkflowSubmitConfig::default()),
                ];

                for node in entry_nodes {
                    let kinds = node.compatible_service_kinds();
                    assert_eq!(
                        kinds.len(),
                        3,
                        "Entry node {node:?} should support all 3 service kinds"
                    );
                    assert!(kinds.contains(&ServiceKind::Handler));
                    assert!(kinds.contains(&ServiceKind::Actor));
                    assert!(kinds.contains(&ServiceKind::Workflow));
                }
            }
        }

        mod state_nodes {
            use super::*;

            #[test]
            fn state_nodes_dont_support_service_kind() {
                let state_nodes: Vec<WorkflowNode> = vec![
                    WorkflowNode::GetState(GetStateConfig::default()),
                    WorkflowNode::SetState(SetStateConfig::default()),
                    WorkflowNode::ClearState(ClearStateConfig::default()),
                ];

                for node in state_nodes {
                    let kinds = node.compatible_service_kinds();
                    assert!(
                        !kinds.contains(&ServiceKind::Handler),
                        "State node {node:?} should not support Service kind"
                    );
                }
            }

            #[test]
            fn state_nodes_support_virtual_object_and_workflow() {
                let state_nodes: Vec<WorkflowNode> = vec![
                    WorkflowNode::GetState(GetStateConfig::default()),
                    WorkflowNode::SetState(SetStateConfig::default()),
                    WorkflowNode::ClearState(ClearStateConfig::default()),
                ];

                for node in state_nodes {
                    let kinds = node.compatible_service_kinds();
                    assert!(
                        kinds.contains(&ServiceKind::Actor),
                        "State node {node:?} should support VirtualObject kind"
                    );
                    assert!(
                        kinds.contains(&ServiceKind::Workflow),
                        "State node {node:?} should support Workflow kind"
                    );
                }
            }
        }

        mod promise_nodes {
            use super::*;

            #[test]
            fn promise_nodes_only_support_workflow() {
                let promise_nodes: Vec<WorkflowNode> = vec![
                    WorkflowNode::DurablePromise(DurablePromiseConfig::default()),
                    WorkflowNode::Awakeable(AwakeableConfig::default()),
                ];

                for node in promise_nodes {
                    let kinds = node.compatible_service_kinds();
                    assert_eq!(
                        kinds.len(),
                        1,
                        "Promise node {node:?} should only support Workflow kind"
                    );
                    assert!(kinds.contains(&ServiceKind::Workflow));
                    assert!(!kinds.contains(&ServiceKind::Handler));
                    assert!(!kinds.contains(&ServiceKind::Actor));
                }
            }
        }
    }

    mod port_types {
        use super::*;

        mod input_port_type {
            use super::*;

            #[test]
            fn http_handler_input_is_any() {
                let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
                assert_eq!(node.input_port_type(), PortType::Any);
            }

            #[test]
            fn signal_handler_input_is_signal() {
                let node = WorkflowNode::SignalHandler(SignalHandlerConfig::default());
                assert_eq!(node.input_port_type(), PortType::Signal);
            }

            #[test]
            fn condition_input_is_any() {
                let node = WorkflowNode::Condition(ConditionConfig::default());
                assert_eq!(node.input_port_type(), PortType::Any);
            }

            #[test]
            fn get_state_input_is_any() {
                let node = WorkflowNode::GetState(GetStateConfig::default());
                assert_eq!(node.input_port_type(), PortType::Any);
            }
        }

        mod output_port_type {
            use super::*;

            #[test]
            fn http_handler_output_is_event() {
                let node = WorkflowNode::HttpHandler(HttpHandlerConfig::default());
                assert_eq!(node.output_port_type(), PortType::Event);
            }

            #[test]
            fn kafka_handler_output_is_event() {
                let node = WorkflowNode::KafkaHandler(KafkaHandlerConfig::default());
                assert_eq!(node.output_port_type(), PortType::Event);
            }

            #[test]
            fn cron_trigger_output_is_event() {
                let node = WorkflowNode::CronTrigger(CronTriggerConfig::default());
                assert_eq!(node.output_port_type(), PortType::Event);
            }

            #[test]
            fn condition_output_is_flow_control() {
                let node = WorkflowNode::Condition(ConditionConfig::default());
                assert_eq!(node.output_port_type(), PortType::FlowControl);
            }

            #[test]
            fn switch_output_is_flow_control() {
                let node = WorkflowNode::Switch(SwitchConfig::default());
                assert_eq!(node.output_port_type(), PortType::FlowControl);
            }

            #[test]
            fn get_state_output_is_state() {
                let node = WorkflowNode::GetState(GetStateConfig::default());
                assert_eq!(node.output_port_type(), PortType::State);
            }

            #[test]
            fn durable_promise_output_is_signal() {
                let node = WorkflowNode::DurablePromise(DurablePromiseConfig::default());
                assert_eq!(node.output_port_type(), PortType::Signal);
            }

            #[test]
            fn awakeable_output_is_signal() {
                let node = WorkflowNode::Awakeable(AwakeableConfig::default());
                assert_eq!(node.output_port_type(), PortType::Signal);
            }

            #[test]
            fn signal_handler_output_is_signal() {
                let node = WorkflowNode::SignalHandler(SignalHandlerConfig::default());
                assert_eq!(node.output_port_type(), PortType::Signal);
            }

            #[test]
            fn run_output_is_json() {
                let node = WorkflowNode::Run(RunConfig::default());
                assert_eq!(node.output_port_type(), PortType::Json);
            }

            #[test]
            fn service_call_output_is_json() {
                let node = WorkflowNode::ServiceCall(ServiceCallConfig::default());
                assert_eq!(node.output_port_type(), PortType::Json);
            }
        }
    }
}
