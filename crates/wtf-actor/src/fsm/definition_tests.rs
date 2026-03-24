use super::*;

// -- Happy Paths ----------------------------------------------------------

#[test]
fn hp1_parse_single_transition() {
    let input = r#"{
        "transitions": [
            {"from": "Pending", "event": "Authorize", "to": "Authorized", "effects": []}
        ]
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");
    let result = def.transition("Pending", "Authorize");
    assert!(result.is_some());
    let (to, effects) = result.unwrap();
    assert_eq!(to, "Authorized");
    assert!(effects.is_empty());
}

#[test]
fn hp2_parse_transitions_with_effects() {
    let input = r#"{
        "transitions": [{
            "from": "Pending",
            "event": "Charge",
            "to": "Charged",
            "effects": [{"effect_type": "CallPayment", "payload": ""}]
        }]
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");
    let (_, effects) = def.transition("Pending", "Charge").unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect_type, "CallPayment");
    assert!(effects[0].payload.is_empty());
}

#[test]
fn hp3_parse_with_terminal_states() {
    let input = r#"{
        "transitions": [],
        "terminal_states": ["Fulfilled", "Failed"]
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");
    assert!(def.is_terminal("Fulfilled"));
    assert!(def.is_terminal("Failed"));
    assert!(!def.is_terminal("Pending"));
}

#[test]
fn hp4_parse_without_terminal_states_field() {
    let input = r#"{"transitions": []}"#;

    let def = parse_fsm(input).expect("parse should succeed");
    assert!(!def.is_terminal("Anything"));
}

#[test]
fn hp5_multiple_transitions_workflow() {
    let input = r#"{
        "transitions": [
            {"from": "Pending", "event": "Authorize", "to": "Authorized", "effects": []},
            {"from": "Authorized", "event": "Charge", "to": "Charged", "effects": []},
            {"from": "Charged", "event": "Fulfill", "to": "Fulfilled", "effects": []}
        ],
        "terminal_states": ["Fulfilled"]
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");

    // Three transitions present
    let (to, _) = def.transition("Pending", "Authorize").unwrap();
    assert_eq!(to, "Authorized");

    let (to, _) = def.transition("Authorized", "Charge").unwrap();
    assert_eq!(to, "Charged");

    let (to, _) = def.transition("Charged", "Fulfill").unwrap();
    assert_eq!(to, "Fulfilled");

    // Terminal state
    assert!(def.is_terminal("Fulfilled"));
    assert!(!def.is_terminal("Pending"));
}

// -- Error Paths ----------------------------------------------------------

#[test]
fn ep1_invalid_json() {
    let result = parse_fsm("not json{{{");
    assert!(matches!(result, Err(ParseFsmError::InvalidJson(_))));
}

#[test]
fn ep2_missing_transitions_field() {
    let result = parse_fsm("{}");
    assert!(matches!(result, Err(ParseFsmError::MissingField(_))));
}

#[test]
fn ep3_transition_missing_from() {
    let input = r#"{"transitions": [{"event": "Go", "to": "Done", "effects": []}]}"#;
    let result = parse_fsm(input);
    assert!(matches!(result, Err(ParseFsmError::MissingField(_))));
}

#[test]
fn ep4_effect_missing_effect_type() {
    let input = r#"{"transitions": [{
        "from": "A", "event": "B", "to": "C",
        "effects": [{"payload": ""}]
    }]}"#;
    let result = parse_fsm(input);
    assert!(matches!(result, Err(ParseFsmError::InvalidEffect(_))));
}

// -- Edge Cases -----------------------------------------------------------

#[test]
fn edge_transitions_not_array() {
    let input = r#"{"transitions": "oops"}"#;
    let result = parse_fsm(input);
    assert!(matches!(result, Err(ParseFsmError::MissingField(_))));
}

#[test]
fn edge_null_terminal_states_treated_as_empty() {
    let input = r#"{"transitions": [], "terminal_states": null}"#;
    let def = parse_fsm(input).expect("parse should succeed");
    assert!(!def.is_terminal("Anything"));
}

#[test]
fn edge_empty_transitions_valid() {
    let input = r#"{"transitions": []}"#;
    let def = parse_fsm(input).expect("parse should succeed");
    assert!(def.transition("A", "B").is_none());
}

#[test]
fn edge_json_array_not_object() {
    let result = parse_fsm("[]");
    assert!(matches!(result, Err(ParseFsmError::InvalidJson(_))));
}

#[test]
fn edge_json_number_not_object() {
    let result = parse_fsm("42");
    assert!(matches!(result, Err(ParseFsmError::InvalidJson(_))));
}

#[test]
fn edge_extra_fields_ignored() {
    let input = r#"{
        "transitions": [],
        "unknown_field": "ignored",
        "another": 42
    }"#;
    let def = parse_fsm(input).expect("parse should succeed with extra fields");
    assert!(def.transition("A", "B").is_none());
}

#[test]
fn edge_effect_with_payload_string() {
    let input = r#"{
        "transitions": [{
            "from": "A", "event": "B", "to": "C",
            "effects": [{"effect_type": "Log", "payload": "hello world"}]
        }]
    }"#;
    let def = parse_fsm(input).expect("parse should succeed");
    let (_, effects) = def.transition("A", "B").unwrap();
    assert_eq!(effects[0].payload.as_ref(), b"hello world");
}

// -- initial_state tests (DEFECT-1 fix) -----------------------------------

#[test]
fn hp6_parse_with_initial_state() {
    let input = r#"{
        "transitions": [],
        "initial_state": "Pending"
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");
    assert_eq!(def.initial_state(), Some("Pending"));
}

#[test]
fn hp7_parse_without_initial_state_field() {
    let input = r#"{"transitions": []}"#;

    let def = parse_fsm(input).expect("parse should succeed");
    assert!(def.initial_state().is_none());
}

#[test]
fn edge_null_initial_state_treated_as_none() {
    let input = r#"{"transitions": [], "initial_state": null}"#;

    let def = parse_fsm(input).expect("parse should succeed");
    assert!(def.initial_state().is_none());
}

// -- E2E Roundtrip with plan_fsm_signal ------------------------------------

#[test]
fn e2e_parse_fsm_roundtrip_with_plan_fsm_signal() {
    use crate::fsm::{plan_fsm_signal, FsmActorState};

    let input = r#"{
        "transitions": [
            {"from": "Pending", "event": "Authorize", "to": "Authorized", "effects": []},
            {"from": "Authorized", "event": "Charge", "to": "Charged", "effects": []}
        ],
        "terminal_states": ["Charged"]
    }"#;

    let def = parse_fsm(input).expect("parse should succeed");

    // Verify transitions
    let (to, _) = def.transition("Pending", "Authorize").unwrap();
    assert_eq!(to, "Authorized");

    // Verify terminal state
    assert!(def.is_terminal("Charged"));
    assert!(!def.is_terminal("Pending"));

    // Roundtrip with plan_fsm_signal
    let state = FsmActorState::new("Pending");
    let plan = plan_fsm_signal(&def, &state, "Authorize").expect("plan should exist");
    assert_eq!(plan.next_state.current_state, "Authorized");
}
