use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use wtf_actor::OrchestratorMsg;
use wtf_common::{
    storage::{ReplayBatch, ReplayedEvent},
    WorkflowEvent,
};

use crate::types::{ApiError, JournalEntry, JournalEntryType, JournalResponse};

use super::{get_event_store, split_path_id};

/// GET /api/v1/workflows/:id/journal — replay workflow journal entries.
pub async fn get_journal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (ns, inst_id) = match parse_journal_request_id(&id) {
        Ok(parts) => parts,
        Err(err) => return err.into_response(),
    };

    let store = match get_event_store(&master).await {
        Some(store) => store,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("actor_error", "event store unavailable")),
            )
                .into_response();
        }
    };

    let mut replay = match store.open_replay_stream(&ns, &inst_id, 1).await {
        Ok(stream) => stream,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiError::new("not_found", format!("{id}"))),
            )
                .into_response();
        }
    };

    let mut entries: Vec<JournalEntry> = Vec::new();

    loop {
        match replay.next_event().await {
            Ok(ReplayBatch::Event(replayed)) => entries.push(map_replayed_event(replayed)),
            Ok(ReplayBatch::TailReached) => break,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("journal_error", error.to_string())),
                )
                    .into_response();
            }
        }
    }

    let entries = sort_entries_by_seq(entries);

    (
        StatusCode::OK,
        Json(JournalResponse::new(id, entries)),
    )
        .into_response()
}

fn parse_journal_request_id(id: &str) -> Result<(wtf_common::NamespaceId, wtf_common::InstanceId), (StatusCode, Json<ApiError>)> {
    if id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_id", "empty invocation id")),
        ));
    }

    match split_path_id(id) {
        Some((ns, inst_id)) => Ok((wtf_common::NamespaceId::new(ns), inst_id)),
        None => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_id", "bad id")),
        )),
    }
}

fn map_replayed_event(replayed: ReplayedEvent) -> JournalEntry {
    let (entry_type, name, input, output, duration_ms, status) = map_event_fields(&replayed.event);

    let seq = match u32::try_from(replayed.seq) {
        Ok(seq) => seq,
        Err(_) => u32::MAX,
    };

    JournalEntry {
        seq,
        entry_type,
        name,
        input,
        output,
        timestamp: Some(replayed.timestamp.to_rfc3339()),
        duration_ms,
        fire_at: None,
        status,
    }
}

fn map_event_fields(
    event: &WorkflowEvent,
) -> (
    JournalEntryType,
    Option<String>,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<u64>,
    Option<String>,
) {
    match event {
        WorkflowEvent::ActivityDispatched {
            activity_type,
            payload,
            ..
        } => (
            JournalEntryType::Run,
            Some(activity_type.clone()),
            serde_json::from_slice::<serde_json::Value>(payload.as_ref()).ok(),
            None,
            None,
            Some("dispatched".to_owned()),
        ),
        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            duration_ms,
        } => (
            JournalEntryType::Run,
            Some(activity_id.clone()),
            None,
            serde_json::from_slice::<serde_json::Value>(result.as_ref()).ok(),
            Some(*duration_ms),
            Some("completed".to_owned()),
        ),
        WorkflowEvent::ActivityFailed {
            activity_id, error, ..
        } => (
            JournalEntryType::Run,
            Some(activity_id.clone()),
            None,
            Some(serde_json::json!({ "error": error })),
            None,
            Some("failed".to_owned()),
        ),
        WorkflowEvent::TimerScheduled { timer_id, fire_at } => (
            JournalEntryType::Wait,
            Some(timer_id.clone()),
            None,
            Some(serde_json::json!({ "fire_at": fire_at.to_rfc3339() })),
            None,
            Some("scheduled".to_owned()),
        ),
        WorkflowEvent::TimerFired { timer_id } => (
            JournalEntryType::Wait,
            Some(timer_id.clone()),
            None,
            None,
            None,
            Some("fired".to_owned()),
        ),
        WorkflowEvent::SignalReceived {
            signal_name,
            payload,
        } => (
            JournalEntryType::Run,
            Some(signal_name.clone()),
            serde_json::from_slice::<serde_json::Value>(payload.as_ref()).ok(),
            None,
            None,
            Some("signal".to_owned()),
        ),
        _ => (
            JournalEntryType::Run,
            Some("event".to_owned()),
            None,
            None,
            None,
            Some("recorded".to_owned()),
        ),
    }
}

fn sort_entries_by_seq(entries: Vec<JournalEntry>) -> Vec<JournalEntry> {
    let mut sorted = entries;
    sorted.sort_by_key(|entry| entry.seq);
    sorted
}

#[cfg(test)]
mod tests {
    use crate::types::{JournalEntry, JournalEntryType};

    use super::{parse_journal_request_id, sort_entries_by_seq};

    #[test]
    fn given_empty_id_when_parsed_then_error() {
        let parsed = parse_journal_request_id("");
        assert!(parsed.is_err());
    }

    #[test]
    fn given_whitespace_id_when_parsed_then_error() {
        let parsed = parse_journal_request_id("   ");
        assert!(parsed.is_err());
    }

    #[test]
    fn given_valid_namespaced_id_when_parsed_then_ok() {
        let parsed = parse_journal_request_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(parsed.is_ok());
    }

    #[test]
    fn given_journal_entries_out_of_order_when_sorted_then_entries_are_ascending_by_seq() {
        let entries = vec![
            JournalEntry {
                seq: 4,
                entry_type: JournalEntryType::Run,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
            JournalEntry {
                seq: 2,
                entry_type: JournalEntryType::Wait,
                name: None,
                input: None,
                output: None,
                timestamp: None,
                duration_ms: None,
                fire_at: None,
                status: None,
            },
        ];

        let sorted = sort_entries_by_seq(entries);
        assert_eq!(sorted[0].seq, 2);
        assert_eq!(sorted[1].seq, 4);
    }
}
