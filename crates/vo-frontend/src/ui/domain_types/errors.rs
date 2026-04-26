use super::node_templates::NodeTemplateId;

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

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for ValidationViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for SerializationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YamlEncodeError(msg) => write!(f, "yaml encode error: {msg}"),
            Self::JsonEncodeError(msg) => write!(f, "json encode error: {msg}"),
            Self::EmptySketch => write!(f, "cannot serialize empty sketch"),
        }
    }
}
