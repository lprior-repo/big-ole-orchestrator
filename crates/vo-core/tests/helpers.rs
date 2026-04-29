use vo_types::{BinaryHash, WorkflowName};

pub fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

pub fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}
