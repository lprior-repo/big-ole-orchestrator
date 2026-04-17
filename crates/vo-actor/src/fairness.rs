//! Fairness tracking types for workload classification.
//!
//! Provides the `WorkloadClass` taxonomy used by the admission controller
//! and permit budget system (ADR-033). Every workload resolves to exactly
//! one class, ensuring all work is subject to fairness accounting.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WorkloadClass {
    #[default]
    Recovery,
    NewInstance,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkloadClassParseError {
    #[error(
        "unknown workload class: \"{input}\". Valid classes: recovery, new_instance, internal"
    )]
    Unknown { input: String },
}

impl FromStr for WorkloadClass {
    type Err = WorkloadClassParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "recovery" => Ok(Self::Recovery),
            "new_instance" | "newinstance" => Ok(Self::NewInstance),
            "internal" => Ok(Self::Internal),
            _ => Err(WorkloadClassParseError::Unknown {
                input: s.to_string(),
            }),
        }
    }
}

impl fmt::Display for WorkloadClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recovery => write!(f, "recovery"),
            Self::NewInstance => write!(f, "new_instance"),
            Self::Internal => write!(f, "internal"),
        }
    }
}

pub const ALL_WORKLOAD_CLASSES: [WorkloadClass; 3] = [
    WorkloadClass::Recovery,
    WorkloadClass::NewInstance,
    WorkloadClass::Internal,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recovery() {
        assert_eq!("recovery".parse(), Ok(WorkloadClass::Recovery));
    }

    #[test]
    fn parse_new_instance() {
        assert_eq!("new_instance".parse(), Ok(WorkloadClass::NewInstance));
    }

    #[test]
    fn parse_internal() {
        assert_eq!("internal".parse(), Ok(WorkloadClass::Internal));
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("Recovery".parse(), Ok(WorkloadClass::Recovery));
        assert_eq!("RECOVERY".parse(), Ok(WorkloadClass::Recovery));
        assert_eq!("New_Instance".parse(), Ok(WorkloadClass::NewInstance));
        assert_eq!("INTERNAL".parse(), Ok(WorkloadClass::Internal));
    }

    #[test]
    fn parse_newinstance_without_underscore() {
        assert_eq!("newinstance".parse(), Ok(WorkloadClass::NewInstance));
    }

    #[test]
    fn parse_rejects_unknown() {
        let result: Result<WorkloadClass, WorkloadClassParseError> = "background".parse();
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            matches!(err, WorkloadClassParseError::Unknown { ref input } if input == "background")
        );
        assert!(err.to_string().contains("background"));
        assert!(err.to_string().contains("recovery"));
    }

    #[test]
    fn parse_rejects_empty() {
        let result: Result<WorkloadClass, WorkloadClassParseError> = "".parse();
        assert!(result.is_err());
    }

    #[test]
    fn default_is_internal() {
        assert_eq!(WorkloadClass::default(), WorkloadClass::Internal);
    }

    #[test]
    fn display_format() {
        assert_eq!(WorkloadClass::Recovery.to_string(), "recovery");
        assert_eq!(WorkloadClass::NewInstance.to_string(), "new_instance");
        assert_eq!(WorkloadClass::Internal.to_string(), "internal");
    }

    #[test]
    fn roundtrip_display_from_str() {
        for class in ALL_WORKLOAD_CLASSES {
            let s = class.to_string();
            let parsed: WorkloadClass = s.parse().unwrap();
            assert_eq!(class, parsed);
        }
    }

    #[test]
    fn all_classes_contains_all_variants() {
        assert_eq!(ALL_WORKLOAD_CLASSES.len(), 3);
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Recovery));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::NewInstance));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Internal));
    }

    #[test]
    fn every_workload_resolves_to_exactly_one_class() {
        let classes = ["recovery", "new_instance", "internal"];
        for input in classes {
            let class: WorkloadClass = input.parse().unwrap();
            let count = ALL_WORKLOAD_CLASSES.iter().filter(|&&c| c == class).count();
            assert_eq!(
                count, 1,
                "class from '{}' mapped to exactly one variant",
                input
            );
        }
    }
}
