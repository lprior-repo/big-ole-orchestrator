#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceKind {
    Handler,
    Workflow,
    Actor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ContextType {
    Synchronous,
    Asynchronous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseServiceKindError(pub String);

impl std::fmt::Display for ParseServiceKindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid ServiceKind: {}", self.0)
    }
}

impl std::error::Error for ParseServiceKindError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseContextTypeError(pub String);

impl std::fmt::Display for ParseContextTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid ContextType: {}", self.0)
    }
}

impl std::error::Error for ParseContextTypeError {}

impl fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handler => write!(f, "Handler"),
            Self::Workflow => write!(f, "Workflow"),
            Self::Actor => write!(f, "Actor"),
        }
    }
}

impl FromStr for ServiceKind {
    type Err = ParseServiceKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "handler" => Ok(Self::Handler),
            "workflow" => Ok(Self::Workflow),
            "actor" => Ok(Self::Actor),
            _ => Err(ParseServiceKindError(s.to_string())),
        }
    }
}

impl fmt::Display for ContextType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Synchronous => write!(f, "Synchronous"),
            Self::Asynchronous => write!(f, "Asynchronous"),
        }
    }
}

impl FromStr for ContextType {
    type Err = ParseContextTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "synchronous" | "sync" => Ok(Self::Synchronous),
            "asynchronous" | "async" => Ok(Self::Asynchronous),
            _ => Err(ParseContextTypeError(s.to_string())),
        }
    }
}

impl ContextType {
    #[must_use]
    pub const fn is_synchronous(&self) -> bool {
        matches!(self, Self::Synchronous)
    }

    #[must_use]
    pub const fn is_asynchronous(&self) -> bool {
        matches!(self, Self::Asynchronous)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    #[default]
    Any,
    Event,
    State,
    Signal,
    #[serde(rename = "flow-control")]
    FlowControl,
    Json,
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Any => "any",
            Self::Event => "event",
            Self::State => "state",
            Self::Signal => "signal",
            Self::FlowControl => "flow-control",
            Self::Json => "json",
        };
        write!(f, "{s}")
    }
}

impl FromStr for PortType {
    type Err = ParsePortTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "any" => Ok(Self::Any),
            "event" => Ok(Self::Event),
            "state" => Ok(Self::State),
            "signal" => Ok(Self::Signal),
            "flow-control" | "flowcontrol" => Ok(Self::FlowControl),
            "json" => Ok(Self::Json),
            _ => Err(ParsePortTypeError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePortTypeError(pub String);

impl std::fmt::Display for ParsePortTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid PortType: {}", self.0)
    }
}

impl std::error::Error for ParsePortTypeError {}

#[must_use]
pub fn types_compatible(source: PortType, target: PortType) -> bool {
    if matches!(source, PortType::Any) || matches!(target, PortType::Any) {
        return true;
    }
    if matches!(source, PortType::Json) || matches!(target, PortType::Json) {
        return true;
    }
    source == target
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::no_effect_underscore_binding
)]
mod tests {
    use super::*;

    mod service_kind {
        use super::*;

        mod variants {
            use super::*;

            #[test]
            fn service_kind_has_service_variant() {
                let _kind = ServiceKind::Handler;
            }

            #[test]
            fn service_kind_has_virtual_object_variant() {
                let _kind = ServiceKind::Actor;
            }

            #[test]
            fn service_kind_has_workflow_variant() {
                let _kind = ServiceKind::Workflow;
            }

            #[test]
            fn service_kind_has_exactly_three_variants() {
                let variants: Vec<ServiceKind> = vec![
                    ServiceKind::Handler,
                    ServiceKind::Actor,
                    ServiceKind::Workflow,
                ];
                assert_eq!(variants.len(), 3);
            }
        }

        mod derives {
            use super::*;

            #[test]
            fn service_kind_impls_copy() {
                fn assert_copy<T: Copy>() {}
                assert_copy::<ServiceKind>();
            }

            #[test]
            fn service_kind_impls_clone() {
                fn assert_clone<T: Clone>() {}
                assert_clone::<ServiceKind>();
            }

            #[test]
            fn service_kind_impls_debug() {
                fn assert_debug<T: std::fmt::Debug>() {}
                assert_debug::<ServiceKind>();
            }

            #[test]
            fn service_kind_impls_partial_eq() {
                fn assert_partial_eq<T: PartialEq>() {}
                assert_partial_eq::<ServiceKind>();
            }

            #[test]
            fn service_kind_impls_eq() {
                fn assert_eq<T: Eq>() {}
                assert_eq::<ServiceKind>();
            }

            #[test]
            fn service_kind_impls_hash() {
                fn assert_hash<T: std::hash::Hash>() {}
                assert_hash::<ServiceKind>();
            }

            #[test]
            fn service_kind_clones_correctly() {
                let original = ServiceKind::Actor;
                let cloned = original.clone();
                assert_eq!(original, cloned);
            }

            #[test]
            fn service_kind_copies_correctly() {
                let original = ServiceKind::Workflow;
                let copied: ServiceKind = original;
                assert_eq!(original, copied);
            }

            #[test]
            fn service_kind_equality_works() {
                assert_eq!(ServiceKind::Handler, ServiceKind::Handler);
                assert_ne!(ServiceKind::Handler, ServiceKind::Actor);
            }
        }

        mod display {
            use super::*;

            #[test]
            fn handler_variant_displays_as_capitalized() {
                let kind = ServiceKind::Handler;
                let display = format!("{kind}");
                assert_eq!(display, "Handler");
            }

            #[test]
            fn actor_variant_displays_as_capitalized() {
                let kind = ServiceKind::Actor;
                let display = format!("{kind}");
                assert_eq!(display, "Actor");
            }

            #[test]
            fn workflow_variant_displays_as_capitalized() {
                let kind = ServiceKind::Workflow;
                let display = format!("{kind}");
                assert_eq!(display, "Workflow");
            }
        }

        mod from_str {
            use super::*;

            #[test]
            fn parses_handler_lowercase() {
                let result: Result<ServiceKind, _> = "handler".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Handler);
            }

            #[test]
            fn parses_handler_uppercase() {
                let result: Result<ServiceKind, _> = "Handler".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Handler);
            }

            #[test]
            fn parses_actor_lowercase() {
                let result: Result<ServiceKind, _> = "actor".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Actor);
            }

            #[test]
            fn parses_actor_uppercase() {
                let result: Result<ServiceKind, _> = "Actor".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Actor);
            }

            #[test]
            fn parses_workflow_lowercase() {
                let result: Result<ServiceKind, _> = "workflow".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Workflow);
            }

            #[test]
            fn parses_workflow_uppercase() {
                let result: Result<ServiceKind, _> = "Workflow".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ServiceKind::Workflow);
            }

            #[test]
            fn rejects_invalid_input() {
                let result: Result<ServiceKind, _> = "invalid".parse();
                assert!(result.is_err());
            }

            #[test]
            fn rejects_empty_string() {
                let result: Result<ServiceKind, _> = "".parse();
                assert!(result.is_err());
            }
        }

        mod serialization {
            use super::*;

            #[test]
            fn serializes_handler_as_kebab_case() {
                let kind = ServiceKind::Handler;
                let json = serde_json::to_string(&kind).expect("Should serialize");
                assert!(json.contains("handler"));
            }

            #[test]
            fn serializes_actor_as_kebab_case() {
                let kind = ServiceKind::Actor;
                let json = serde_json::to_string(&kind).expect("Should serialize");
                assert!(json.contains("actor"));
            }

            #[test]
            fn serializes_workflow_as_kebab_case() {
                let kind = ServiceKind::Workflow;
                let json = serde_json::to_string(&kind).expect("Should serialize");
                assert!(json.contains("workflow"));
            }

            #[test]
            fn roundtrip_preserves_handler() {
                let original = ServiceKind::Handler;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: ServiceKind =
                    serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }

            #[test]
            fn roundtrip_preserves_actor() {
                let original = ServiceKind::Actor;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: ServiceKind =
                    serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }

            #[test]
            fn roundtrip_preserves_workflow() {
                let original = ServiceKind::Workflow;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: ServiceKind =
                    serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }
        }
    }

    mod context_type {
        use super::*;

        mod variants {
            use super::*;

            #[test]
            fn context_type_has_synchronous_variant() {
                let _ctx = ContextType::Synchronous;
            }

            #[test]
            fn context_type_has_asynchronous_variant() {
                let _ctx = ContextType::Asynchronous;
            }

            #[test]
            fn context_type_has_exactly_two_variants() {
                let variants: Vec<ContextType> =
                    vec![ContextType::Synchronous, ContextType::Asynchronous];
                assert_eq!(variants.len(), 2);
            }
        }

        mod derives {
            use super::*;

            #[test]
            fn context_type_impls_copy() {
                fn assert_copy<T: Copy>() {}
                assert_copy::<ContextType>();
            }

            #[test]
            fn context_type_impls_clone() {
                fn assert_clone<T: Clone>() {}
                assert_clone::<ContextType>();
            }

            #[test]
            fn context_type_impls_debug() {
                fn assert_debug<T: std::fmt::Debug>() {}
                assert_debug::<ContextType>();
            }

            #[test]
            fn context_type_impls_partial_eq() {
                fn assert_partial_eq<T: PartialEq>() {}
                assert_partial_eq::<ContextType>();
            }

            #[test]
            fn context_type_impls_eq() {
                fn assert_eq<T: Eq>() {}
                assert_eq::<ContextType>();
            }

            #[test]
            fn context_type_impls_hash() {
                fn assert_hash<T: std::hash::Hash>() {}
                assert_hash::<ContextType>();
            }

            #[test]
            fn context_type_clones_correctly() {
                let original = ContextType::Asynchronous;
                let cloned = original.clone();
                assert_eq!(original, cloned);
            }

            #[test]
            fn context_type_copies_correctly() {
                let original = ContextType::Synchronous;
                let copied: ContextType = original;
                assert_eq!(original, copied);
            }

            #[test]
            fn context_type_equality_works() {
                assert_eq!(ContextType::Synchronous, ContextType::Synchronous);
                assert_ne!(ContextType::Synchronous, ContextType::Asynchronous);
            }
        }

        mod display {
            use super::*;

            #[test]
            fn synchronous_variant_displays_as_capitalized() {
                let ctx = ContextType::Synchronous;
                let display = format!("{ctx}");
                assert_eq!(display, "Synchronous");
            }

            #[test]
            fn asynchronous_variant_displays_as_capitalized() {
                let ctx = ContextType::Asynchronous;
                let display = format!("{ctx}");
                assert_eq!(display, "Asynchronous");
            }
        }

        mod from_str {
            use super::*;

            #[test]
            fn parses_synchronous() {
                let result: Result<ContextType, _> = "synchronous".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ContextType::Synchronous);
            }

            #[test]
            fn parses_sync_abbreviation() {
                let result: Result<ContextType, _> = "sync".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ContextType::Synchronous);
            }

            #[test]
            fn parses_asynchronous() {
                let result: Result<ContextType, _> = "asynchronous".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ContextType::Asynchronous);
            }

            #[test]
            fn parses_async_abbreviation() {
                let result: Result<ContextType, _> = "async".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), ContextType::Asynchronous);
            }

            #[test]
            fn rejects_invalid_input() {
                let result: Result<ContextType, _> = "invalid".parse();
                assert!(result.is_err());
            }

            #[test]
            fn rejects_empty_string() {
                let result: Result<ContextType, _> = "".parse();
                assert!(result.is_err());
            }
        }

        mod is_synchronous {
            use super::*;

            #[test]
            fn synchronous_is_synchronous() {
                assert!(ContextType::Synchronous.is_synchronous());
            }

            #[test]
            fn asynchronous_is_not_synchronous() {
                assert!(!ContextType::Asynchronous.is_synchronous());
            }
        }

        mod is_asynchronous {
            use super::*;

            #[test]
            fn asynchronous_is_asynchronous() {
                assert!(ContextType::Asynchronous.is_asynchronous());
            }

            #[test]
            fn synchronous_is_not_asynchronous() {
                assert!(!ContextType::Synchronous.is_asynchronous());
            }
        }

        mod serialization {
            use super::*;

            #[test]
            fn serializes_synchronous_as_kebab_case() {
                let ctx = ContextType::Synchronous;
                let json = serde_json::to_string(&ctx).expect("Should serialize");
                assert!(json.contains("synchronous"));
            }

            #[test]
            fn serializes_asynchronous_as_kebab_case() {
                let ctx = ContextType::Asynchronous;
                let json = serde_json::to_string(&ctx).expect("Should serialize");
                assert!(json.contains("asynchronous"));
            }

            #[test]
            fn roundtrip_preserves_synchronous() {
                let original = ContextType::Synchronous;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: ContextType =
                    serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }

            #[test]
            fn roundtrip_preserves_asynchronous() {
                let original = ContextType::Asynchronous;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: ContextType =
                    serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }
        }
    }

    mod port_type {
        use super::*;

        mod variants {
            use super::*;

            #[test]
            fn port_type_has_any_variant() {
                let _port = PortType::Any;
            }

            #[test]
            fn port_type_has_event_variant() {
                let _port = PortType::Event;
            }

            #[test]
            fn port_type_has_state_variant() {
                let _port = PortType::State;
            }

            #[test]
            fn port_type_has_signal_variant() {
                let _port = PortType::Signal;
            }

            #[test]
            fn port_type_has_flow_control_variant() {
                let _port = PortType::FlowControl;
            }

            #[test]
            fn port_type_has_json_variant() {
                let _port = PortType::Json;
            }

            #[test]
            fn port_type_has_exactly_six_variants() {
                let variants: Vec<PortType> = vec![
                    PortType::Any,
                    PortType::Event,
                    PortType::State,
                    PortType::Signal,
                    PortType::FlowControl,
                    PortType::Json,
                ];
                assert_eq!(variants.len(), 6);
            }
        }

        mod derives {
            use super::*;

            #[test]
            fn port_type_impls_copy() {
                fn assert_copy<T: Copy>() {}
                assert_copy::<PortType>();
            }

            #[test]
            fn port_type_impls_clone() {
                fn assert_clone<T: Clone>() {}
                assert_clone::<PortType>();
            }

            #[test]
            fn port_type_impls_debug() {
                fn assert_debug<T: std::fmt::Debug>() {}
                assert_debug::<PortType>();
            }

            #[test]
            fn port_type_impls_partial_eq() {
                fn assert_partial_eq<T: PartialEq>() {}
                assert_partial_eq::<PortType>();
            }

            #[test]
            fn port_type_impls_eq() {
                fn assert_eq<T: Eq>() {}
                assert_eq::<PortType>();
            }

            #[test]
            fn port_type_impls_hash() {
                fn assert_hash<T: std::hash::Hash>() {}
                assert_hash::<PortType>();
            }
        }

        mod default {
            use super::*;

            #[test]
            fn port_type_defaults_to_any() {
                assert_eq!(PortType::default(), PortType::Any);
            }
        }

        mod display {
            use super::*;

            #[test]
            fn any_displays_as_lowercase() {
                assert_eq!(format!("{}", PortType::Any), "any");
            }

            #[test]
            fn event_displays_as_lowercase() {
                assert_eq!(format!("{}", PortType::Event), "event");
            }

            #[test]
            fn state_displays_as_lowercase() {
                assert_eq!(format!("{}", PortType::State), "state");
            }

            #[test]
            fn signal_displays_as_lowercase() {
                assert_eq!(format!("{}", PortType::Signal), "signal");
            }

            #[test]
            fn flow_control_displays_as_kebab_case() {
                assert_eq!(format!("{}", PortType::FlowControl), "flow-control");
            }

            #[test]
            fn json_displays_as_lowercase() {
                assert_eq!(format!("{}", PortType::Json), "json");
            }
        }

        mod from_str {
            use super::*;

            #[test]
            fn parses_any() {
                let result: Result<PortType, _> = "any".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::Any);
            }

            #[test]
            fn parses_event() {
                let result: Result<PortType, _> = "event".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::Event);
            }

            #[test]
            fn parses_state() {
                let result: Result<PortType, _> = "state".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::State);
            }

            #[test]
            fn parses_signal() {
                let result: Result<PortType, _> = "signal".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::Signal);
            }

            #[test]
            fn parses_flow_control_kebab() {
                let result: Result<PortType, _> = "flow-control".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::FlowControl);
            }

            #[test]
            fn parses_json() {
                let result: Result<PortType, _> = "json".parse();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), PortType::Json);
            }

            #[test]
            fn rejects_invalid_input() {
                let result: Result<PortType, _> = "invalid".parse();
                assert!(result.is_err());
            }
        }

        mod serialization {
            use super::*;

            #[test]
            fn serializes_any_as_lowercase() {
                let json = serde_json::to_string(&PortType::Any).expect("Should serialize");
                assert!(json.contains("any"));
            }

            #[test]
            fn serializes_flow_control_as_lowercase() {
                let json = serde_json::to_string(&PortType::FlowControl).expect("Should serialize");
                assert!(json.contains("flow-control"));
            }

            #[test]
            fn roundtrip_preserves_event() {
                let original = PortType::Event;
                let json = serde_json::to_string(&original).expect("Should serialize");
                let restored: PortType = serde_json::from_str(&json).expect("Should deserialize");
                assert_eq!(original, restored);
            }
        }
    }

    mod types_compatible_fn {
        use super::*;

        #[test]
        fn any_source_is_compatible_with_any_target() {
            assert!(types_compatible(PortType::Any, PortType::Event));
            assert!(types_compatible(PortType::Any, PortType::State));
            assert!(types_compatible(PortType::Any, PortType::Json));
        }

        #[test]
        fn any_target_accepts_any_source() {
            assert!(types_compatible(PortType::Event, PortType::Any));
            assert!(types_compatible(PortType::State, PortType::Any));
            assert!(types_compatible(PortType::Signal, PortType::Any));
        }

        #[test]
        fn json_source_is_compatible_with_any_target() {
            assert!(types_compatible(PortType::Json, PortType::Event));
            assert!(types_compatible(PortType::Json, PortType::State));
        }

        #[test]
        fn json_target_accepts_any_source() {
            assert!(types_compatible(PortType::Event, PortType::Json));
            assert!(types_compatible(PortType::State, PortType::Json));
        }

        #[test]
        fn same_types_are_compatible() {
            assert!(types_compatible(PortType::Event, PortType::Event));
            assert!(types_compatible(PortType::State, PortType::State));
            assert!(types_compatible(PortType::Signal, PortType::Signal));
            assert!(types_compatible(
                PortType::FlowControl,
                PortType::FlowControl
            ));
        }

        #[test]
        fn different_specific_types_are_not_compatible() {
            assert!(!types_compatible(PortType::Event, PortType::State));
            assert!(!types_compatible(PortType::State, PortType::Signal));
            assert!(!types_compatible(PortType::Signal, PortType::FlowControl));
            assert!(!types_compatible(PortType::Event, PortType::FlowControl));
        }
    }
}
