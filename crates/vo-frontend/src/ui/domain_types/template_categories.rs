use super::node_templates::NodeTemplateId;

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
