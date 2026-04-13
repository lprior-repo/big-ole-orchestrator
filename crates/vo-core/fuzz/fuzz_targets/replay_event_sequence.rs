#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_core::replay::ReplayEngine;
use vo_types::events::EventEnvelope;

const MAX_EVENTS: usize = 1000;
const MAX_EVENT_SIZE: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(events_json) = serde_json::from_str::<serde_json::Value>(s) else {
        return;
    };

    let Some(events_array) = events_json.as_array() else {
        return;
    };

    if events_array.is_empty() {
        let engine = ReplayEngine::new();
        let _ = engine.replay(&[]);
        return;
    }

    let mut envelopes = Vec::with_capacity(events_array.len().min(MAX_EVENTS));

    for (i, event_val) in events_array.iter().take(MAX_EVENTS).enumerate() {
        let Some(obj) = event_val.as_object() else {
            continue;
        };

        let version = obj
            .get("version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(1);

        let instance_id = obj
            .get("instance_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("fuzz-instance");

        let sequence = obj
            .get("sequence")
            .and_then(|v| v.as_u64())
            .unwrap_or((i + 1) as u64);

        let timestamp_ms = obj
            .get("timestamp_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        let payload = obj.get("payload").cloned().unwrap_or_else(|| {
            serde_json::json!({
                "type": "WorkflowStarted",
                "workflow_id": "fuzz-wf",
                "binary_hash": "abc",
                "version": 1
            })
        });

        let payload_size = serde_json::to_string(&payload)
            .map(|s| s.len())
            .unwrap_or(0);

        if payload_size > MAX_EVENT_SIZE {
            continue;
        }

        envelopes.push(EventEnvelope {
            schema_version: version,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms,
            payload,
            metadata: vo_types::events::EventMetadata::default(),
        });
    }

    if envelopes.is_empty() {
        return;
    }

    let engine = ReplayEngine::new();
    let _ = engine.replay(&envelopes);
});
