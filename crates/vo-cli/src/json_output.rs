use serde_json::Value;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn git_commit() -> &'static str {
    option_env!("GIT_HASH").unwrap_or("unknown")
}

fn base_payload(r#type: &str, command: &str, exit_code: i32) -> Value {
    serde_json::json!({
        "type": r#type,
        "command": command,
        "exit_code": exit_code,
        "version": VERSION,
        "commit": git_commit(),
    })
}

pub fn json_success_payload(command: &str, data: Value) -> String {
    let mut payload = base_payload("success", command, 0);
    payload.as_object_mut().map(|m| m.insert("data".to_string(), data));
    serde_json::to_string(&payload).unwrap_or_else(|_| r#"{"type":"success","error":{"kind":"serialization_error","message":"failed to serialize JSON output"}}"#.to_string())
}

pub fn json_error_payload(command: &str, exit_code: i32, kind: &str, message: &str) -> String {
    let mut payload = base_payload("error", command, exit_code);
    payload.as_object_mut().map(|m| {
        m.insert(
            "error".to_string(),
            serde_json::json!({
                "kind": kind,
                "message": message,
            }),
        )
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| format!(
        r#"{{"type":"error","command":"{command}","exit_code":{exit_code},"error":{{"kind":"serialization_error","message":"failed to serialize JSON output"}}}}"#
    ))
}

pub fn error_kind(err: &crate::cli::CliError) -> &'static str {
    match err {
        crate::cli::CliError::Clap(_) => "invalid_usage",
        crate::cli::CliError::InvalidNumeric(_) => "invalid_numeric",
        crate::cli::CliError::Dispatch(_) => "dispatch_error",
        crate::cli::CliError::Check(_) => "check_error",
        crate::cli::CliError::Compensate(_) => "compensate_error",
        crate::cli::CliError::Gc(_) => "gc_error",
        crate::cli::CliError::Init(_) => "init_error",
        crate::cli::CliError::Lock(_) => "lock_error",
        crate::cli::CliError::Doctor(_) => "doctor_error",
        crate::cli::CliError::Rebuild(_) => "rebuild_error",
        crate::cli::CliError::Status(_) => "status_error",
        crate::cli::CliError::WorkflowHistory(_) => "workflow_history_error",
        crate::cli::CliError::Workspace(_) => "workspace_error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_payload_is_valid_json() {
        let output = json_success_payload("health", serde_json::json!({"status": "ok"}));
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["type"], "success");
        assert_eq!(parsed["command"], "health");
        assert_eq!(parsed["exit_code"], 0);
        assert_eq!(parsed["version"], VERSION);
        assert_eq!(parsed["data"]["status"], "ok");
    }

    #[test]
    fn error_payload_is_valid_json() {
        let output = json_error_payload("status", 1, "connection_error", "Failed to connect");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["command"], "status");
        assert_eq!(parsed["exit_code"], 1);
        assert_eq!(parsed["error"]["kind"], "connection_error");
        assert_eq!(parsed["error"]["message"], "Failed to connect");
    }

    #[test]
    fn success_payload_contains_version_and_commit() {
        let output = json_success_payload("init", serde_json::json!({}));
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert!(parsed["version"].is_string());
        assert!(parsed["commit"].is_string());
    }

    #[test]
    fn error_payload_contains_version_and_commit() {
        let output = json_error_payload("gc", 1, "dispatch_error", "test");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert!(parsed["version"].is_string());
        assert!(parsed["commit"].is_string());
    }

    #[test]
    fn success_payload_no_error_field() {
        let output = json_success_payload("check", serde_json::json!({}));
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn error_payload_has_no_data_field() {
        let output = json_error_payload("lock", 1, "lock_error", "test");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert!(parsed.get("data").is_none());
    }

    #[test]
    fn error_kind_maps_clap_to_invalid_usage() {
        let err = crate::cli::CliError::Clap(clap::Error::new(clap::error::ErrorKind::InvalidValue));
        assert_eq!(error_kind(&err), "invalid_usage");
    }

    #[test]
    fn error_kind_maps_dispatch() {
        let err = crate::cli::CliError::Dispatch("test".to_string());
        assert_eq!(error_kind(&err), "dispatch_error");
    }

    #[test]
    fn json_output_has_no_control_characters() {
        let output = json_success_payload("test", serde_json::json!({"msg": "hello\nworld"}));
        assert!(!output.contains('\x00'));
        assert!(!output.contains('\x01'));
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert!(parsed["data"]["msg"].is_string());
    }

    #[test]
    fn error_payload_exit_code_nonzero() {
        let output = json_error_payload("purge", 1, "dispatch_error", "Instance is running");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_ne!(parsed["exit_code"], 0);
    }
}
