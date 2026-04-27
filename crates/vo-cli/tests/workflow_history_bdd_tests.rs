use vo_cli::{
    interpret_cli_from, Command, WorkflowHistoryConfig, WorkflowHistoryEntry,
    WorkflowHistoryResponse,
};
use vo_types::{apply_redaction, RedactionKind, RedactionRule};

fn make_sensitive_history_entries() -> Vec<WorkflowHistoryEntry> {
    vec![
        WorkflowHistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "workflow_started".to_string(),
            step_id: None,
            error: None,
            output: Some(serde_json::json!({
                "workflow_name": "payment",
                "input": {"amount": 100}
            })),
        },
        WorkflowHistoryEntry {
            sequence: 2,
            timestamp_ms: 1700000001000,
            event_type: "step_completed".to_string(),
            step_id: Some("charge-card".to_string()),
            error: None,
            output: Some(serde_json::json!({
                "result": "charged",
                "secrets": {"api_key": "sk-live-12345"},
                "card_token": "tok_abc"
            })),
        },
        WorkflowHistoryEntry {
            sequence: 3,
            timestamp_ms: 1700000002000,
            event_type: "step_completed".to_string(),
            step_id: Some("send-receipt".to_string()),
            error: None,
            output: Some(serde_json::json!({
                "result": "sent",
                "password": "super-secret-pass",
                "email": "user@example.com"
            })),
        },
    ]
}

fn default_ai_redaction_rules() -> Vec<RedactionRule> {
    vec![
        RedactionRule::new(vec!["secrets".to_string()], RedactionKind::Remove),
        RedactionRule::new(vec!["api_key".to_string()], RedactionKind::Remove),
        RedactionRule::new(vec!["token".to_string()], RedactionKind::Remove),
        RedactionRule::new(vec!["password".to_string()], RedactionKind::Remove),
    ]
}

#[test]
fn given_ai_runs_history_json_when_instance_exists_then_redacted_workflow_history_is_returned() {
    let entries = make_sensitive_history_entries();

    let rules = default_ai_redaction_rules();

    for entry in &entries {
        if let Some(ref output) = entry.output {
            let (redacted, redacted_paths) = apply_redaction(output, &rules);

            assert!(
                !serde_json::to_string(&redacted)
                    .unwrap()
                    .contains("sk-live-12345"),
                "PII (api_key value) must not appear in redacted output"
            );
            assert!(
                !serde_json::to_string(&redacted)
                    .unwrap()
                    .contains("super-secret-pass"),
                "PII (password value) must not appear in redacted output"
            );

            let _ = redacted_paths;
        }
    }

    let response = WorkflowHistoryResponse {
        instance_id: "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        entries: entries.clone(),
        redacted_fields: Some(vec![
            vec!["secrets".to_string()],
            vec!["password".to_string()],
        ]),
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    let parsed_back: WorkflowHistoryResponse = serde_json::from_str(&json).unwrap();

    assert_eq!(
        parsed_back.instance_id,
        "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV"
    );
    assert_eq!(parsed_back.entries.len(), 3);

    assert!(
        json.contains("\"instance_id\""),
        "JSON must contain instance_id"
    );
    assert!(json.contains("\"entries\""), "JSON must contain entries");
    assert!(
        json.contains("\"sequence\""),
        "JSON must contain sequence numbers"
    );
    assert!(
        json.contains("\"event_type\""),
        "JSON must contain event types"
    );
    assert!(
        json.contains("\"timestamp_ms\""),
        "JSON must contain timestamps"
    );
    assert!(
        json.contains("\"redacted_fields\""),
        "JSON must contain redacted_fields metadata"
    );

    assert!(
        json.contains("workflow_started"),
        "Event type must be present"
    );
    assert!(
        json.contains("step_completed"),
        "Event type must be present"
    );
    assert!(json.contains("charge-card"), "Step ID must be present");
    assert!(json.contains("send-receipt"), "Step ID must be present");
}

#[test]
fn given_ai_runs_history_json_cli_when_parsed_then_history_command_returned() {
    let cli = interpret_cli_from(vec![
        "vo",
        "history",
        "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "--json",
    ])
    .expect("valid CLI args");

    match &cli.command {
        Command::History {
            instance_id,
            engine_url,
            json,
            ..
        } => {
            assert_eq!(instance_id, "payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(*json, "--json flag must be true");
        }
        _ => panic!("expected History command, got {:?}", cli.command),
    }
}

#[test]
fn given_ai_runs_history_without_json_flag_then_json_is_false() {
    let cli =
        interpret_cli_from(vec!["vo", "history", "ns/test-instance"]).expect("valid CLI args");

    match &cli.command {
        Command::History { json, .. } => {
            assert!(!json, "--json should default to false");
        }
        _ => panic!("expected History command"),
    }
}

#[test]
fn given_ai_runs_history_with_custom_engine_url_then_url_propagated() {
    let cli = interpret_cli_from(vec![
        "vo",
        "history",
        "ns/inst-1",
        "--engine-url",
        "http://engine:9000",
    ])
    .expect("valid CLI args");

    match &cli.command {
        Command::History { engine_url, .. } => {
            assert_eq!(engine_url, "http://engine:9000");
        }
        _ => panic!("expected History command"),
    }
}

#[test]
fn given_history_response_when_redacted_then_sensitive_fields_removed_from_entries() {
    let entries = make_sensitive_history_entries();

    let rules = default_ai_redaction_rules();

    let redacted_entries: Vec<WorkflowHistoryEntry> = entries
        .into_iter()
        .map(|entry| {
            let redacted_output = entry.output.as_ref().map(|output| {
                let (redacted, _) = apply_redaction(output, &rules);
                redacted
            });
            WorkflowHistoryEntry {
                output: redacted_output,
                ..entry
            }
        })
        .collect();

    assert_eq!(redacted_entries.len(), 3);

    let charge_output = redacted_entries[1].output.as_ref().unwrap();
    assert!(
        !charge_output.as_object().unwrap().contains_key("secrets"),
        "secrets field must be removed from step_completed entry"
    );
    assert_eq!(
        charge_output["result"], "charged",
        "non-sensitive fields preserved"
    );

    let receipt_output = redacted_entries[2].output.as_ref().unwrap();
    assert!(
        !receipt_output.as_object().unwrap().contains_key("password"),
        "password field must be removed"
    );
    assert_eq!(
        receipt_output["email"], "user@example.com",
        "non-sensitive fields preserved"
    );
}

#[test]
fn given_history_response_when_serialized_then_json_structure_is_stable() {
    let response = WorkflowHistoryResponse {
        instance_id: "payments/01ARZ3NDEK".to_string(),
        entries: vec![WorkflowHistoryEntry {
            sequence: 1,
            timestamp_ms: 1700000000000,
            event_type: "workflow_started".to_string(),
            step_id: Some("start".to_string()),
            error: None,
            output: Some(serde_json::json!({"status": "ok"})),
        }],
        redacted_fields: None,
    };

    let json_str = serde_json::to_string(&response).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.is_object(), "root must be a JSON object");
    assert!(parsed["instance_id"].is_string());
    assert!(parsed["entries"].is_array());
    assert!(parsed["entries"][0]["sequence"].is_number());
    assert!(parsed["entries"][0]["timestamp_ms"].is_number());
    assert!(parsed["entries"][0]["event_type"].is_string());
    assert!(parsed["entries"][0]["step_id"].is_string());
    assert!(parsed["entries"][0]["output"].is_object());

    let roundtrip: WorkflowHistoryResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(roundtrip.instance_id, response.instance_id);
    assert_eq!(roundtrip.entries.len(), response.entries.len());
}
