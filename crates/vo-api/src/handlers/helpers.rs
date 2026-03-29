use vo_actor::messages::WorkflowParadigm;
use vo_common::InstanceId;

/// Split a path `<namespace>/<instance_id>` into the two parts.
///
/// Returns `None` if the path has no `/` separator.
#[must_use]
pub fn split_path_id(path: &str) -> Option<(String, InstanceId)> {
    let slash = path.find('/')?;
    let namespace = path[..slash].to_owned();
    let instance_id = InstanceId::new(path[slash + 1..].to_owned());
    Some((namespace, instance_id))
}

#[must_use]
pub fn parse_paradigm(s: &str) -> Option<WorkflowParadigm> {
    match s {
        "fsm" => Some(WorkflowParadigm::Fsm),
        "dag" => Some(WorkflowParadigm::Dag),
        "procedural" => Some(WorkflowParadigm::Procedural),
        _ => None,
    }
}

#[must_use]
pub fn paradigm_to_str(p: WorkflowParadigm) -> &'static str {
    match p {
        WorkflowParadigm::Fsm => "fsm",
        WorkflowParadigm::Dag => "dag",
        WorkflowParadigm::Procedural => "procedural",
    }
}

#[must_use]
pub fn phase_to_str(p: vo_actor::messages::InstancePhaseView) -> &'static str {
    match p {
        vo_actor::messages::InstancePhaseView::Replay => "replay",
        vo_actor::messages::InstancePhaseView::Live => "live",
    }
}
