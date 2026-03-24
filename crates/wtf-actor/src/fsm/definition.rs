use bytes::Bytes;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use wtf_common::EffectDeclaration;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from parsing `graph_raw` JSON into [`FsmDefinition`].
#[derive(Debug, Error)]
pub enum ParseFsmError {
    #[error("invalid JSON in graph_raw: {0}")]
    InvalidJson(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("invalid effect declaration: {0}")]
    InvalidEffect(String),
}

// ---------------------------------------------------------------------------
// Serde intermediate types (private — wire format only)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct FsmGraph {
    transitions: Vec<FsmTransitionJson>,
    #[serde(default)]
    terminal_states: Option<Vec<String>>,
    #[serde(default)]
    initial_state: Option<String>,
}

#[derive(Deserialize)]
struct FsmTransitionJson {
    from: Option<String>,
    event: Option<String>,
    to: Option<String>,
    #[serde(default)]
    effects: Vec<FsmEffectJson>,
}

#[derive(Deserialize)]
struct FsmEffectJson {
    effect_type: Option<String>,
    #[serde(default)]
    payload: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure conversion helpers (Calculation layer)
// ---------------------------------------------------------------------------

fn validate_transition(
    t: &FsmTransitionJson,
) -> Result<(String, String, String, Vec<EffectDeclaration>), ParseFsmError> {
    let from = t
        .from
        .as_ref()
        .ok_or(ParseFsmError::MissingField("from"))?
        .clone();
    let event = t
        .event
        .as_ref()
        .ok_or(ParseFsmError::MissingField("event"))?
        .clone();
    let to =
        t.to.as_ref()
            .ok_or(ParseFsmError::MissingField("to"))?
            .clone();

    let effects = t
        .effects
        .iter()
        .map(parse_effect)
        .collect::<Result<Vec<_>, _>>()?;

    Ok((from, event, to, effects))
}

fn parse_effect(e: &FsmEffectJson) -> Result<EffectDeclaration, ParseFsmError> {
    let effect_type = e
        .effect_type
        .as_ref()
        .ok_or_else(|| ParseFsmError::InvalidEffect("missing effect_type".into()))?
        .clone();
    let payload = e
        .payload
        .as_deref()
        .map_or_else(Bytes::new, |s| Bytes::from(s.as_bytes().to_vec()));
    Ok(EffectDeclaration {
        effect_type,
        payload,
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse `graph_raw` JSON into an [`FsmDefinition`].
///
/// Pure function — zero I/O, zero logging.
pub fn parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError> {
    let raw: serde_json::Value =
        serde_json::from_str(graph_raw).map_err(|e| ParseFsmError::InvalidJson(e.to_string()))?;

    if !raw.is_object() {
        return Err(ParseFsmError::InvalidJson(
            "graph_raw must be a JSON object".into(),
        ));
    }

    if raw.get("transitions").is_none_or(|v| !v.is_array()) {
        return Err(ParseFsmError::MissingField("transitions"));
    }

    let graph: FsmGraph =
        serde_json::from_value(raw).map_err(|e| ParseFsmError::InvalidJson(e.to_string()))?;

    // Validate all transitions before building (pure error collection)
    let validated: Vec<_> = graph
        .transitions
        .iter()
        .map(validate_transition)
        .collect::<Result<Vec<_>, _>>()?;

    let transitions: HashMap<(String, String), (String, Vec<EffectDeclaration>)> = validated
        .into_iter()
        .map(|(from, event, to, effects)| ((from, event), (to, effects)))
        .collect();

    let terminal_states = graph
        .terminal_states
        .map_or_else(HashSet::new, |v| v.into_iter().collect());

    Ok(FsmDefinition {
        transitions,
        terminal_states,
        initial_state: graph.initial_state,
    })
}

// ---------------------------------------------------------------------------
// FsmDefinition (existing)
// ---------------------------------------------------------------------------

/// FSM workflow definition (bead wtf-tzjw).
#[derive(Debug, Clone, Default)]
pub struct FsmDefinition {
    transitions: HashMap<(String, String), (String, Vec<EffectDeclaration>)>,
    terminal_states: HashSet<String>,
    initial_state: Option<String>,
}

impl FsmDefinition {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare `state` as a terminal state (workflow ends when entered).
    pub fn add_terminal_state(&mut self, state: impl Into<String>) {
        self.terminal_states.insert(state.into());
    }

    /// Returns `true` if `state` is a declared terminal state.
    #[must_use]
    pub fn is_terminal(&self, state: &str) -> bool {
        self.terminal_states.contains(state)
    }

    /// Returns the declared initial state, if any.
    #[must_use]
    pub fn initial_state(&self) -> Option<&str> {
        self.initial_state.as_deref()
    }

    pub fn add_transition(
        &mut self,
        from: impl Into<String>,
        event: impl Into<String>,
        to: impl Into<String>,
        effects: Vec<EffectDeclaration>,
    ) {
        self.transitions
            .insert((from.into(), event.into()), (to.into(), effects));
    }

    #[must_use]
    pub fn transition(
        &self,
        current_state: &str,
        event_name: &str,
    ) -> Option<(&str, &[EffectDeclaration])> {
        self.transitions
            .get(&(current_state.to_owned(), event_name.to_owned()))
            .map(|(to, effects)| (to.as_str(), effects.as_slice()))
    }
}

// ---------------------------------------------------------------------------
// Tests (extracted to definition_tests.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "definition_tests.rs"]
mod tests;
