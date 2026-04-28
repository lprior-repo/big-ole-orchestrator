//! Fairness tracking types for workload classification.
//!
//! Re-exports the canonical `WorkloadClass` from `vo_types::workload_class`.

pub use vo_types::workload_class::{
    WorkloadClass, WorkloadClassParseError, ALL_WORKLOAD_CLASSES,
};

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
        let result: Result<WorkloadClass, WorkloadClassParseError> = "foobar".parse();
        assert!(result.is_err());
    }

    #[test]
    fn parse_rejects_empty() {
        let result: Result<WorkloadClass, WorkloadClassParseError> = "".parse();
        assert!(result.is_err());
    }

    #[test]
    fn default_is_standard() {
        assert_eq!(WorkloadClass::default(), WorkloadClass::Standard);
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
        assert_eq!(ALL_WORKLOAD_CLASSES.len(), 10);
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Recovery));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::NewInstance));
        assert!(ALL_WORKLOAD_CLASSES.contains(&WorkloadClass::Internal));
    }

    #[test]
    fn every_workload_resolves_to_exactly_one_class() {
        let classes = [
            "recovery",
            "new_instance",
            "internal",
            "exact_critical",
            "live",
            "standard",
            "timer_resume",
            "non_critical",
            "background",
            "unsafe_bulk",
        ];
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
