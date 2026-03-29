pub mod helpers;
pub mod workflow;
pub mod signal;
pub mod events;

pub use workflow::*;
pub use signal::*;
pub use events::*;

#[cfg(test)]
mod tests {
    use super::helpers::*;
    use vo_actor::messages::WorkflowParadigm;

    #[test]
    fn split_path_id_returns_namespace_and_id_when_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        if let Some((ns, id)) = result {
            assert_eq!(ns, "payments");
            assert_eq!(id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        }
    }

    #[test]
    fn split_path_id_returns_none_when_missing_slash() {
        let result = split_path_id("no-slash-here");
        assert!(result.is_none());
    }

    #[test]
    fn split_path_id_splits_on_first_slash_when_multiple_present() {
        let result = split_path_id("ns/id/extra");
        assert!(result.is_some());
        if let Some((ns, id)) = result {
            assert_eq!(ns, "ns");
            assert_eq!(id.as_str(), "id/extra");
        }
    }

    #[test]
    fn parse_paradigm_returns_fsm_when_fsm_string() {
        assert_eq!(parse_paradigm("fsm"), Some(WorkflowParadigm::Fsm));
    }

    #[test]
    fn parse_paradigm_returns_dag_when_dag_string() {
        assert_eq!(parse_paradigm("dag"), Some(WorkflowParadigm::Dag));
    }

    #[test]
    fn parse_paradigm_returns_procedural_when_procedural_string() {
        assert_eq!(
            parse_paradigm("procedural"),
            Some(WorkflowParadigm::Procedural)
        );
    }

    #[test]
    fn parse_paradigm_returns_none_when_invalid_string() {
        assert!(parse_paradigm("").is_none());
        assert!(parse_paradigm("FSM").is_none());
        assert!(parse_paradigm("state_machine").is_none());
    }

    #[test]
    fn paradigm_to_str_roundtrips_through_parse() {
        for p in [
            WorkflowParadigm::Fsm,
            WorkflowParadigm::Dag,
            WorkflowParadigm::Procedural,
        ] {
            let s = paradigm_to_str(p);
            assert_eq!(parse_paradigm(s), Some(p));
        }
    }
}
