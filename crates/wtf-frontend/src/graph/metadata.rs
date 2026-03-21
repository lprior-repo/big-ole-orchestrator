use super::NodeCategory;

#[allow(clippy::too_many_lines)]
pub(super) fn node_metadata(node_type: &str) -> (NodeCategory, String, String) {
    match node_type {
        "http-handler" => (
            NodeCategory::Entry,
            "HTTP Handler".to_string(),
            "globe".to_string(),
        ),
        "kafka-handler" => (
            NodeCategory::Entry,
            "Kafka Consumer".to_string(),
            "kafka".to_string(),
        ),
        "cron-trigger" => (
            NodeCategory::Entry,
            "Cron Trigger".to_string(),
            "clock".to_string(),
        ),
        "workflow-submit" => (
            NodeCategory::Entry,
            "Workflow Submit".to_string(),
            "play-circle".to_string(),
        ),
        "run" => (
            NodeCategory::Durable,
            "Durable Step".to_string(),
            "shield".to_string(),
        ),
        "service-call" => (
            NodeCategory::Durable,
            "Service Call".to_string(),
            "arrow-right".to_string(),
        ),
        "object-call" => (
            NodeCategory::Durable,
            "Object Call".to_string(),
            "box".to_string(),
        ),
        "workflow-call" => (
            NodeCategory::Durable,
            "Workflow Call".to_string(),
            "workflow".to_string(),
        ),
        "send-message" => (
            NodeCategory::Durable,
            "Send Message".to_string(),
            "send".to_string(),
        ),
        "delayed-send" => (
            NodeCategory::Durable,
            "Delayed Message".to_string(),
            "clock-send".to_string(),
        ),
        "get-state" => (
            NodeCategory::State,
            "Get State".to_string(),
            "download".to_string(),
        ),
        "set-state" => (
            NodeCategory::State,
            "Set State".to_string(),
            "upload".to_string(),
        ),
        "clear-state" => (
            NodeCategory::State,
            "Clear State".to_string(),
            "eraser".to_string(),
        ),
        "condition" => (
            NodeCategory::Flow,
            "If / Else".to_string(),
            "git-branch".to_string(),
        ),
        "switch" => (
            NodeCategory::Flow,
            "Switch".to_string(),
            "git-fork".to_string(),
        ),
        "loop" => (
            NodeCategory::Flow,
            "Loop / Iterate".to_string(),
            "repeat".to_string(),
        ),
        "parallel" => (
            NodeCategory::Flow,
            "Parallel".to_string(),
            "layers".to_string(),
        ),
        "compensate" => (
            NodeCategory::Flow,
            "Compensate".to_string(),
            "undo".to_string(),
        ),
        "sleep" => (
            NodeCategory::Timing,
            "Sleep / Timer".to_string(),
            "timer".to_string(),
        ),
        "timeout" => (
            NodeCategory::Timing,
            "Timeout".to_string(),
            "alarm".to_string(),
        ),
        "durable-promise" => (
            NodeCategory::Signal,
            "Durable Promise".to_string(),
            "sparkles".to_string(),
        ),
        "awakeable" => (
            NodeCategory::Signal,
            "Awakeable".to_string(),
            "bell".to_string(),
        ),
        "resolve-promise" => (
            NodeCategory::Signal,
            "Resolve Promise".to_string(),
            "check-circle".to_string(),
        ),
        "signal-handler" => (
            NodeCategory::Signal,
            "Signal Handler".to_string(),
            "radio".to_string(),
        ),
        _ => (
            NodeCategory::Durable,
            "Unknown Node".to_string(),
            "help-circle".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::node_metadata;
    use crate::graph::NodeCategory;

    #[test]
    fn given_known_node_type_when_fetching_metadata_then_expected_values_are_returned() {
        let (category, label, icon) = node_metadata("http-handler");

        assert_eq!(category, NodeCategory::Entry);
        assert_eq!(label, "HTTP Handler");
        assert_eq!(icon, "globe");
    }

    #[test]
    fn given_unknown_node_type_when_fetching_metadata_then_defaults_are_returned() {
        let (category, label, icon) = node_metadata("totally-unknown");

        assert_eq!(category, NodeCategory::Durable);
        assert_eq!(label, "Unknown Node");
        assert_eq!(icon, "help-circle");
    }
}
