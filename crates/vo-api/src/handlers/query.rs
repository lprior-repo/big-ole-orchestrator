//! Query API handlers for workflow state inspection (ADR-007).

use std::sync::Arc;

use axum::{
    extract::{Path, Query as AxumQuery, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use vo_storage::event_log::replay_events_in_namespace;
use vo_types::search::{QueryParser, SearchEngine, SearchResult};

use crate::types::v3::*;
use crate::types::ApiError;
use vo_types::workspace::{WorkspaceId, WorkspaceIndex};

use super::split_path_id;

/// Shared state for query handlers.
#[derive(Clone)]
pub struct QueryState {
    pub db: Arc<fjall::Database>,
    pub workspace_index: Arc<std::sync::RwLock<WorkspaceIndex>>,
    pub search_engine: Arc<std::sync::RwLock<SearchEngine>>,
}

impl QueryState {
    pub fn new(
        db: Arc<fjall::Database>,
        workspace_index: Arc<std::sync::RwLock<WorkspaceIndex>>,
        search_engine: Arc<std::sync::RwLock<SearchEngine>>,
    ) -> Self {
        Self {
            db,
            workspace_index,
            search_engine,
        }
    }

    pub fn index_workspace(&self, id: WorkspaceId, text: &str, tags: &[String]) {
        if let Ok(mut engine) = self.search_engine.write() {
            engine.index_workspace(id, text, tags);
        }
    }

    pub fn remove_from_index(&self, id: WorkspaceId) {
        if let Ok(mut engine) = self.search_engine.write() {
            engine.remove_workspace(id);
        }
    }

    pub fn build_engine_from_workspace_index(&self) {
        if let Ok(workspace) = self.workspace_index.read() {
            if let Ok(mut engine) = self.search_engine.write() {
                *engine = SearchEngine::new();
                for (id, node) in &workspace.nodes {
                    let text = node.name.to_string();
                    let tags: Vec<String> = node
                        .metadata
                        .entries
                        .iter()
                        .flat_map(|(k, v)| [k.clone(), v.clone()])
                        .collect();
                    engine.index_workspace(*id, &text, &tags);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GET /api/v1/workflows/:id/timeline
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all)]
pub async fn get_timeline(
    Path(id): Path<String>,
    State(state): State<QueryState>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let iter = replay_events_in_namespace(&state.db, &namespace, &instance_id);
    let mut entries = Vec::new();
    let mut total_replayed = 0usize;

    for result in iter {
        total_replayed += 1;
        match result {
            Ok(envelope) => {
                let event_type = envelope
                    .payload
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map_or("unknown", |value| value)
                    .to_string();
                entries.push(TimelineEntry {
                    sequence: envelope.sequence,
                    timestamp_ms: envelope.timestamp_ms,
                    event_type,
                    payload: envelope.payload,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, seq = total_replayed, "timeline replay stopped");
                break;
            }
        }
    }

    (
        StatusCode::OK,
        Json(TimelineResponse {
            instance_id: id,
            entries,
            total_replayed,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// GET /api/v1/workflows/:id/history
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all)]
pub async fn get_history(
    Path(id): Path<String>,
    State(state): State<QueryState>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let iter = replay_events_in_namespace(&state.db, &namespace, &instance_id);
    let mut entries = Vec::new();

    for result in iter {
        match result {
            Ok(envelope) => {
                let event_type = envelope
                    .payload
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map_or("unknown", |value| value)
                    .to_string();
                let step_id = envelope
                    .payload
                    .get("step_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let error = envelope
                    .payload
                    .get("error")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let output = envelope.payload.get("output").cloned();

                entries.push(HistoryEntry {
                    sequence: envelope.sequence,
                    timestamp_ms: envelope.timestamp_ms,
                    event_type,
                    step_id,
                    error,
                    output,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "history replay stopped");
                break;
            }
        }
    }

    (
        StatusCode::OK,
        Json(HistoryResponse {
            instance_id: id,
            entries,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// GET /api/v1/workflows/:id/effect-journal
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all)]
pub async fn get_effect_journal(
    Path(id): Path<String>,
    State(state): State<QueryState>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let iter = replay_events_in_namespace(&state.db, &namespace, &instance_id);
    let mut entries = Vec::new();

    for result in iter {
        match result {
            Ok(envelope) => {
                let event_type = envelope
                    .payload
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map_or("unknown", |value| value)
                    .to_string();

                let semantics = envelope
                    .metadata
                    .annotations
                    .get("semantics")
                    .and_then(|v| v.as_str())
                    .map(|s| {
                        if s == "exact" {
                            EffectSemantics::Exact
                        } else {
                            EffectSemantics::Unsafe
                        }
                    })
                    .map_or(EffectSemantics::Unsafe, |value| value);

                entries.push(EffectJournalEntry {
                    sequence: envelope.sequence,
                    timestamp_ms: envelope.timestamp_ms,
                    event_type,
                    semantics,
                    payload: envelope.payload,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "effect journal replay stopped");
                break;
            }
        }
    }

    (
        StatusCode::OK,
        Json(EffectJournalResponse {
            instance_id: id,
            entries,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// GET /api/v1/workflows/:id/version
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all)]
pub async fn get_workflow_version(
    Path(id): Path<String>,
    State(state): State<QueryState>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let iter = replay_events_in_namespace(&state.db, &namespace, &instance_id);
    let mut event_count = 0u64;
    let mut last_sequence = None;
    let mut last_timestamp_ms = None;
    let mut schema_version = 1u8;

    for result in iter {
        match result {
            Ok(envelope) => {
                schema_version = envelope.schema_version;
                event_count += 1;
                last_sequence = Some(envelope.sequence);
                last_timestamp_ms = Some(envelope.timestamp_ms);
            }
            Err(e) => {
                tracing::warn!(error = %e, "version replay stopped");
                break;
            }
        }
    }

    (
        StatusCode::OK,
        Json(WorkflowVersionResponse {
            instance_id: id,
            schema_version,
            event_count,
            last_sequence,
            last_timestamp_ms,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// GET /api/v1/search?q=<query>&limit=<limit>
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all)]
pub async fn search(
    AxumQuery(params): AxumQuery<SearchRequest>,
    State(state): State<QueryState>,
) -> impl IntoResponse {
    let query_text = params.query.trim();
    if query_text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("empty_query", "query string cannot be empty")),
        )
            .into_response();
    }

    let parsed_query = match QueryParser::new().parse(query_text) {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_query", e.to_string())),
            )
                .into_response();
        }
    };

    let engine = match state.search_engine.read() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!(error = %e, "search engine lock poisoned");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("search_error", "search engine unavailable")),
            )
                .into_response();
        }
    };

    let results: Result<Vec<vo_types::search::SearchResult>, (StatusCode, Json<ApiError>)> =
        engine.search(&parsed_query).map_err(|e| {
            tracing::error!(error = %e, "search failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("search_error", &e.to_string())),
            )
        });

    let results = match results {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };

    let limit = params.limit.unwrap_or(10).min(100);
    let results: Vec<SearchResultEntry> = results
        .into_iter()
        .take(limit)
        .map(|r| SearchResultEntry {
            workspace_id: r.workspace_id.to_string(),
            score: r.score,
            matched_terms: r.matched_terms,
        })
        .collect();

    (
        StatusCode::OK,
        Json(SearchResponse {
            query: params.query,
            results,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_id_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, _id) = result.unwrap();
        assert_eq!(ns, "payments");
    }

    #[test]
    fn split_path_id_no_slash_returns_none() {
        assert!(split_path_id("no-slash").is_none());
    }

    #[test]
    fn split_path_id_empty_returns_none() {
        assert!(split_path_id("").is_none());
    }
}
