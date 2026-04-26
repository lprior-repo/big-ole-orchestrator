//! Kani verification harnesses for effect types.

use super::types::*;
use super::transitions::*;

/// K-01: Verify apply_effect_transition exhaustiveness.
/// All 3×2 = 6 combinations must be covered without panic.
#[kani::proof]
fn verify_effect_transition_exhaustiveness() {
    let state: u8 = kani::any();
    let event: u8 = kani::any();
    kani::assume(state < 3);
    kani::assume(event < 2);

    let current = match state {
        0 => EffectIntent::Prepared,
        1 => EffectIntent::Committed,
        _ => EffectIntent::RolledBack,
    };
    let evt = match event {
        0 => EffectTransitionEvent::Commit,
        _ => EffectTransitionEvent::Rollback,
    };

    // Must not panic — all combinations handled
    let _ = apply_effect_transition(current, evt);
}

/// K-02: Verify EffectRecord::new rejects empty intent_id.
#[kani::proof]
fn verify_effect_record_rejects_empty_intent_id() {
    let intent_id = String::new();
    let result = EffectRecord::new(
        intent_id,
        EffectKind::HttpCall,
        serde_json::Value::Null,
        EffectIntent::Prepared,
        None,
    );
    assert!(result.is_none());
}
