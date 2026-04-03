use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_SUPPORTED_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct State {
    pub version: u16,
}
impl Default for State {
    fn default() -> Self {
        unimplemented!()
    }
}
impl State {
    pub fn version(&self) -> u16 {
        unimplemented!()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct WorkflowSpec {
    pub version: u16,
}
impl Default for WorkflowSpec {
    fn default() -> Self {
        unimplemented!()
    }
}
impl WorkflowSpec {
    pub fn version(&self) -> u16 {
        unimplemented!()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub version: u16,
}
impl Default for Snapshot {
    fn default() -> Self {
        unimplemented!()
    }
}
impl Snapshot {
    pub fn version(&self) -> u16 {
        unimplemented!()
    }
}

pub fn extract_schema_version(
    payload: &serde_json::Value,
    fallback_policy: Option<u16>,
) -> Result<u16, crate::events::Error> {
    unimplemented!()
}
