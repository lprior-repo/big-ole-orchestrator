// Integration tests for FsmDefinition terminal state detection (bead wtf-xcb4).
use wtf_actor::fsm::{plan_fsm_signal, FsmActorState, FsmDefinition};

#[test]
fn is_terminal_returns_true_for_declared_terminal_state() {
    let mut def = FsmDefinition::new();
    def.add_terminal_state("Done");
    assert!(def.is_terminal("Done"));
}

#[test]
fn is_terminal_returns_false_for_non_terminal_state() {
    let mut def = FsmDefinition::new();
    def.add_terminal_state("Done");
    assert!(!def.is_terminal("Pending"));
}

#[test]
fn is_terminal_returns_false_when_no_terminals_declared() {
    let def = FsmDefinition::new();
    assert!(!def.is_terminal("AnyState"));
}

#[test]
fn transitioning_into_terminal_state_is_detectable() {
    let mut def = FsmDefinition::new();
    def.add_transition("Authorized", "Fulfill", "Fulfilled", vec![]);
    def.add_terminal_state("Fulfilled");
    let state = FsmActorState::new("Authorized");
    let plan = plan_fsm_signal(&def, &state, "Fulfill").expect("plan");
    assert!(def.is_terminal(&plan.next_state.current_state));
}
