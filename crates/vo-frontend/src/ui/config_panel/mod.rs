mod execution;

use serde_json::Value;

pub(crate) fn get_str_val(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn get_u64_val(config: &Value, key: &str) -> Option<u64> {
    config.get(key).and_then(Value::as_u64)
}
