// NOTE: This test file references handlers that are currently commented out in
// vo-api/src/handlers/mod.rs due to compilation errors (vo_actor::messages doesn't exist).
// These tests are preserved for reference and should be uncommented when the
// V2 actor migration is complete.
//
// The tests covered:
// - Security testing (XSS, SQL injection, shell injection, path traversal)
// - Input validation (empty/missing fields, oversized payloads, invalid characters)
// - Error response structure validation
//
// To re-enable, uncomment and fix dependencies on:
// - vo_api::handlers::{start_workflow, get_workflow, terminate_workflow, list_workflows, send_signal, get_events}
// - vo_actor::OrchestratorMsg
// - ractor::ActorRef::new (needs proper actor construction)
