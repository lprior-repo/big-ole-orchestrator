use super::{get_event_store, split_path_id};
use crate::types::EventRecord;
use axum::{
    extract::{Extension, Path, Query},
    http::{header, StatusCode},
    response::IntoResponse,
};
use futures::stream;
use ractor::ActorRef;
use wtf_actor::OrchestratorMsg;
use wtf_common::storage::{ReplayBatch, ReplayedEvent};
use wtf_common::NamespaceId;

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    pub from_seq: Option<u64>,
}

pub async fn get_events(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let (ns, inst_id) = match split_path_id(&id) {
        Some(p) => (NamespaceId::new(p.0), p.1),
        None => return (StatusCode::BAD_REQUEST, "invalid_id").into_response(),
    };
    let store = match get_event_store(&master).await {
        Some(s) => s,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "no_store").into_response(),
    };

    let from_seq = query.from_seq.unwrap_or(0);
    match store.open_replay_stream(&ns, &inst_id, from_seq).await {
        Ok(stream) => {
            let s = stream::unfold(stream, |mut stream| async move {
                match stream.next_event().await {
                    Ok(ReplayBatch::Event(replayed)) => {
                        Some((map_replayed_event(replayed), stream))
                    }
                    _ => None,
                }
            });
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/x-ndjson")],
                axum::body::Body::from_stream(s),
            )
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn map_replayed_event(replayed: ReplayedEvent) -> Result<String, String> {
    let data = serde_json::to_value(&replayed.event).unwrap_or(serde_json::Value::Null);
    let event_type = data
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();
    let record = EventRecord {
        seq: replayed.seq,
        event_type,
        data,
        timestamp: replayed.timestamp,
    };
    Ok(serde_json::to_string(&record).map_err(|e| e.to_string())? + "\n")
}
