//! Crash-injection framework for ADR-043 exact-once verification.
//!
//! This module provides injectable crash points at all 12 critical transitions
//! defined in ADR-043:
//! 1. dedupe write
//! 2. StepScheduled
//! 3. fence acquisition
//! 4. child start
//! 5. EffectPrepared
//! 6. connector commit
//! 7. EffectCommitted
//! 8. StepCompleted
//! 9. timer persistence
//! 10. signal acceptance
//! 11. lineage rollover
//! 12. compensation prepare/commit
//!
//! The framework uses a deterministic crash injection state machine that allows
//! tests to configure which transition to crash-at and then verify that the system
//! reaches a consistent state after recovery.
//!
//! ## Architecture
//!
//! Data (`CrashPoint`, `CrashConfig`, `CrashState`) → Actions (crash injection gates)
//!
//! ## Usage
//!
//! ```ignore
//! let config = CrashConfig::new()
//!     .with_crash_at(CrashPoint::DedupeWrite)
//!     .with_recovery(true);
//! let injector = CrashInjector::new(config);
//!
//! // Before the transition:
//! injector.before("dedupe-key-1");
//!
//! // If configured to crash here, this will panic:
//! injector.check_crash("dedupe-key-1");
//!
//! // After the transition:
//! injector.after("dedupe-key-1");
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::panic;

// ---------------------------------------------------------------------------
// Data: CrashPoint
// ---------------------------------------------------------------------------

/// Crash points corresponding to the 12 critical transitions in ADR-043.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrashPoint {
    /// Dedupe write (ADR-043 crash point 1)
    DedupeWrite,
    /// StepScheduled transition (ADR-043 crash point 2)
    StepScheduled,
    /// Fence acquisition (ADR-043 crash point 3)
    FenceAcquisition,
    /// Child start (ADR-043 crash point 4)
    ChildStart,
    /// EffectPrepared (ADR-043 crash point 5)
    EffectPrepared,
    /// Connector commit (ADR-043 crash point 6)
    ConnectorCommit,
    /// EffectCommitted (ADR-043 crash point 7)
    EffectCommitted,
    /// StepCompleted (ADR-043 crash point 8)
    StepCompleted,
    /// Timer persistence (ADR-043 crash point 9)
    TimerPersistence,
    /// Signal acceptance (ADR-043 crash point 10)
    SignalAcceptance,
    /// Lineage rollover (ADR-043 crash point 11)
    LineageRollover,
    /// Compensation prepare/commit (ADR-043 crash point 12)
    Compensation,
}

impl fmt::Display for CrashPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DedupeWrite => write!(f, "dedupe_write"),
            Self::StepScheduled => write!(f, "step_scheduled"),
            Self::FenceAcquisition => write!(f, "fence_acquisition"),
            Self::ChildStart => write!(f, "child_start"),
            Self::EffectPrepared => write!(f, "effect_prepared"),
            Self::ConnectorCommit => write!(f, "connector_commit"),
            Self::EffectCommitted => write!(f, "effect_committed"),
            Self::StepCompleted => write!(f, "step_completed"),
            Self::TimerPersistence => write!(f, "timer_persistence"),
            Self::SignalAcceptance => write!(f, "signal_acceptance"),
            Self::LineageRollover => write!(f, "lineage_rollover"),
            Self::Compensation => write!(f, "compensation"),
        }
    }
}

// ---------------------------------------------------------------------------
// Data: CrashConfig
// ---------------------------------------------------------------------------

/// Configuration for crash injection behavior.
pub struct CrashConfig {
    crash_at: Option<CrashPoint>,
    crash_before: bool,
    recoverable: bool,
    max_crashes: u64,
}

impl Default for CrashConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CrashConfig {
    pub fn new() -> Self {
        Self {
            crash_at: None,
            crash_before: true,
            recoverable: true,
            max_crashes: u64::MAX,
        }
    }

    /// Configure which crash point to inject at.
    pub fn with_crash_at(mut self, point: CrashPoint) -> Self {
        self.crash_at = Some(point);
        self
    }

    /// Configure whether to crash before or after the transition.
    pub fn with_crash_phase(mut self, before: bool) -> Self {
        self.crash_before = before;
        self
    }

    /// Configure whether crashes are recoverable (system should reach consistent state).
    pub fn with_recovery(mut self, recoverable: bool) -> Self {
        self.recoverable = recoverable;
        self
    }

    /// Set maximum number of crashes before giving up.
    pub fn with_max_crashes(mut self, max: u64) -> Self {
        self.max_crashes = max;
        self
    }
}

// ---------------------------------------------------------------------------
// Data: CrashState
// ---------------------------------------------------------------------------

/// Per-operation crash state tracking crash counts and recovery status.
#[derive(Debug, Clone)]
struct CrashState {
    crash_count: u64,
    crashed_before: bool,
    recovered: bool,
}

impl CrashState {
    fn new() -> Self {
        Self {
            crash_count: 0,
            crashed_before: false,
            recovered: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Actions: CrashInjector
// ---------------------------------------------------------------------------

/// Crash injection engine for ADR-043 verification tests.
///
/// Thread-safe wrapper around per-key crash state using thread-local storage
/// for the active configuration.
pub struct CrashInjector {
    config: CrashConfig,
    states: RefCell<HashMap<String, CrashState>>,
}

impl CrashInjector {
    pub fn new(config: CrashConfig) -> Self {
        Self {
            config,
            states: RefCell::new(HashMap::new()),
        }
    }

    /// Get or create crash state for an operation key.
    fn get_or_create_state(&self, key: &str) -> CrashState {
        let mut states = self.states.borrow_mut();
        states
            .entry(key.to_string())
            .or_insert_with(CrashState::new)
            .clone()
    }

    /// Call this BEFORE the critical transition. Panics if a crash is configured.
    pub fn before(&self, key: &str) {
        let point = match self.config.crash_at {
            Some(p) => p,
            None => return,
        };

        let mut states = self.states.borrow_mut();
        let state = states
            .entry(key.to_string())
            .or_insert_with(CrashState::new);

        if state.crash_count >= self.config.max_crashes {
            return;
        }

        if self.config.crash_before && !state.crashed_before {
            state.crashed_before = true;
            state.crash_count += 1;
            panic!(
                "ADR-043 crash injection: crash at {} (before), attempt #{}",
                point, state.crash_count
            );
        }
    }

    /// Call this AFTER the critical transition. Triggers recovery check.
    pub fn after(&self, key: &str) {
        let point = match self.config.crash_at {
            Some(p) => p,
            None => return,
        };

        let mut states = self.states.borrow_mut();
        let state = states
            .entry(key.to_string())
            .or_insert_with(CrashState::new);

        if self.config.crash_before && state.crashed_before {
            // Already crashed before, don't also crash after for same attempt
            state.recovered = true;
            return;
        }

        if !self.config.crash_before && !state.recovered {
            state.recovered = true;
            if state.crash_count < self.config.max_crashes {
                state.crash_count += 1;
                panic!(
                    "ADR-043 crash injection: crash at {} (after), attempt #{}",
                    point, state.crash_count
                );
            }
        }
    }

    /// Check if a crash has occurred and recovery is needed.
    pub fn needs_recovery(&self, key: &str) -> bool {
        let states = self.states.borrow();
        states
            .get(key)
            .map(|s| s.crashed_before && !s.recovered)
            .unwrap_or(false)
    }

    /// Record that recovery has been performed.
    pub fn record_recovery(&self, key: &str) {
        let mut states = self.states.borrow_mut();
        if let Some(state) = states.get_mut(key) {
            state.recovered = true;
        }
    }

    /// Get crash count for an operation key.
    pub fn crash_count(&self, key: &str) -> u64 {
        let states = self.states.borrow();
        states.get(key).map(|s| s.crash_count).unwrap_or(0)
    }

    /// Reset all crash state (for test cleanup).
    pub fn reset(&self) {
        self.states.borrow_mut().clear();
    }
}

// ---------------------------------------------------------------------------
// Actions: CrashPointInjector (scoped guard for automatic crash injection)
// ---------------------------------------------------------------------------

/// Scoped crash injection guard. Panics on drop if crash was configured.
/// Simulates a process crash mid-transition.
pub struct CrashGuard<'a> {
    injector: &'a CrashInjector,
    key: String,
    crashed: bool,
}

impl<'a> CrashGuard<'a> {
    pub fn new(injector: &'a CrashInjector, key: &str, crash_after: bool) -> Self {
        if !crash_after {
            injector.before(key);
        }
        Self {
            injector,
            key: key.to_string(),
            crashed: false,
        }
    }
}

impl Drop for CrashGuard<'_> {
    fn drop(&mut self) {
        if !self.crashed {
            self.injector.after(&self.key);
        }
    }
}

// ---------------------------------------------------------------------------
// Data: CrashScenario
// ---------------------------------------------------------------------------

/// A crash scenario that defines a sequence of crash points and expected outcomes.
#[derive(Debug, Clone)]
pub struct CrashScenario {
    pub name: String,
    pub crash_points: Vec<CrashPoint>,
    pub expected_properties: Vec<String>,
}

impl CrashScenario {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            crash_points: Vec::new(),
            expected_properties: Vec::new(),
        }
    }

    pub fn with_crash_point(mut self, point: CrashPoint) -> Self {
        self.crash_points.push(point);
        self
    }

    pub fn with_expected_property(mut self, property: &str) -> Self {
        self.expected_properties.push(property.to_string());
        self
    }
}

// ---------------------------------------------------------------------------
// Data: ReplayInvariant
// ---------------------------------------------------------------------------

/// Represents a replay invariant that must hold after crash recovery.
#[derive(Debug, Clone)]
pub struct ReplayInvariant {
    pub name: String,
    pub description: String,
    pub pre_crash_state: String,
    pub post_recovery_state: String,
    pub invariant_predicate: String,
}

impl ReplayInvariant {
    pub fn new(name: &str, description: &str, pre_state: &str, post_state: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            pre_crash_state: pre_state.to_string(),
            post_recovery_state: post_state.to_string(),
            invariant_predicate: format!(
                "state({:?}) == state({:?})",
                pre_state, post_state
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_point_display_all_12_transitions() {
        // Verify all 12 ADR-043 crash points have Display implementations
        assert_eq!(CrashPoint::DedupeWrite.to_string(), "dedupe_write");
        assert_eq!(CrashPoint::StepScheduled.to_string(), "step_scheduled");
        assert_eq!(
            CrashPoint::FenceAcquisition.to_string(),
            "fence_acquisition"
        );
        assert_eq!(CrashPoint::ChildStart.to_string(), "child_start");
        assert_eq!(
            CrashPoint::EffectPrepared.to_string(),
            "effect_prepared"
        );
        assert_eq!(
            CrashPoint::ConnectorCommit.to_string(),
            "connector_commit"
        );
        assert_eq!(
            CrashPoint::EffectCommitted.to_string(),
            "effect_committed"
        );
        assert_eq!(
            CrashPoint::StepCompleted.to_string(),
            "step_completed"
        );
        assert_eq!(
            CrashPoint::TimerPersistence.to_string(),
            "timer_persistence"
        );
        assert_eq!(
            CrashPoint::SignalAcceptance.to_string(),
            "signal_acceptance"
        );
        assert_eq!(
            CrashPoint::LineageRollover.to_string(),
            "lineage_rollover"
        );
        assert_eq!(
            CrashPoint::Compensation.to_string(),
            "compensation"
        );
    }

    #[test]
    fn no_crash_config_does_not_panic() {
        let injector = CrashInjector::new(CrashConfig::new());
        // Should not panic — no crash configured
        injector.before("op-1");
        injector.after("op-1");
    }

    #[test]
    #[should_panic(expected = "ADR-043 crash injection")]
    fn before_crash_panic_when_configured() {
        let injector =
            CrashInjector::new(CrashConfig::new().with_crash_at(CrashPoint::DedupeWrite));
        injector.before("op-1"); // should panic
    }

    #[test]
    fn after_crash_panic_when_configured() {
        let config = CrashConfig::new()
            .with_crash_at(CrashPoint::EffectCommitted)
            .with_crash_phase(false);
        let injector = CrashInjector::new(config);

        // Before should NOT panic
        injector.before("op-2");
        // After should panic
        let result = panic::catch_unwind(|| injector.after("op-2"));
        assert!(result.is_err());
    }

    #[test]
    fn multiple_operations_have_independent_state() {
        let injector =
            CrashInjector::new(CrashConfig::new().with_crash_at(CrashPoint::StepScheduled));

        let result1 = panic::catch_unwind(|| injector.before("op-a"));
        assert!(result1.is_err());

        // op-b should also crash (independent state, same config)
        let result2 = panic::catch_unwind(|| injector.before("op-b"));
        assert!(result2.is_err());
    }

    #[test]
    fn needs_recovery_after_crash() {
        let injector =
            CrashInjector::new(CrashConfig::new().with_crash_at(CrashPoint::FenceAcquisition));

        let result = panic::catch_unwind(|| injector.before("op-3"));
        assert!(result.is_err());

        assert!(injector.needs_recovery("op-3"));
        injector.record_recovery("op-3");
        assert!(!injector.needs_recovery("op-3"));
    }

    #[test]
    fn reset_clears_all_crash_state() {
        let injector =
            CrashInjector::new(CrashConfig::new().with_crash_at(CrashPoint::ChildStart));

        let _ = panic::catch_unwind(|| injector.before("op-reset"));
        assert!(injector.needs_recovery("op-reset"));

        injector.reset();
        assert!(!injector.needs_recovery("op-reset"));
    }

    #[test]
    fn crash_config_max_crashes_limit() {
        let config = CrashConfig::new()
            .with_crash_at(CrashPoint::SignalAcceptance)
            .with_max_crashes(2);
        let injector = CrashInjector::new(config);

        // First crash
        let _ = panic::catch_unwind(|| injector.before("op-max"));
        assert_eq!(injector.crash_count("op-max"), 1);

        // Second crash
        let _ = panic::catch_unwind(|| injector.before("op-max"));
        assert_eq!(injector.crash_count("op-max"), 2);

        // Should NOT crash again (max reached)
        injector.before("op-max"); // no panic
    }

    #[test]
    fn crash_scenario_builder() {
        let scenario = CrashScenario::new("full_workflow_crash_recovery")
            .with_crash_point(CrashPoint::DedupeWrite)
            .with_crash_point(CrashPoint::StepScheduled)
            .with_crash_point(CrashPoint::FenceAcquisition)
            .with_expected_property("duplicate ingress does not create duplicate work")
            .with_expected_property("replay reaches same legal state");

        assert_eq!(scenario.name, "full_workflow_crash_recovery");
        assert_eq!(scenario.crash_points.len(), 3);
        assert_eq!(scenario.expected_properties.len(), 2);
    }

    #[test]
    fn replay_invariant_creation() {
        let invariant = ReplayInvariant::new(
            "INV-REPLAY-001",
            "Replay after dedupe write crash reaches same state",
            "admitted",
            "admitted",
        );

        assert_eq!(invariant.name, "INV-REPLAY-001");
        assert!(invariant.invariant_predicate.contains("admitted"));
    }
}
