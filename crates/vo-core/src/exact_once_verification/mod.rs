//! Exact-once verification harness (ADR-043).
//!
//! This module provides a framework for crash-oriented exact-once testing,
//! defining injectable crash points, deterministic state comparison, and
//! verification properties.
//!
//! ## Crash-Point Matrix
//!
//! Every critical transition defines injectable crash points before and after:
//!
//! 1. [`CrashPoint::DedupeWrite`]        - dedupe write
//! 2. [`CrashPoint::StepScheduled`]       - StepScheduled transition
//! 3. [`CrashPoint::FenceAcquisition`]   - fence acquisition
//! 4. [`CrashPoint::ChildStart`]          - child start
//! 5. [`CrashPoint::EffectPrepared`]      - EffectPrepared
//! 6. [`CrashPoint::ConnectorCommit`]     - connector commit
//! 7. [`CrashPoint::EffectCommitted`]    - EffectCommitted
//! 8. [`CrashPoint::StepCompleted`]       - StepCompleted
//! 9. [`CrashPoint::TimerPersistence`]   - timer persistence
//! 10. [`CrashPoint::SignalAcceptance`]  - signal acceptance
//! 11. [`CrashPoint::LineageRollover`]    - lineage rollover
//! 12. [`CrashPoint::Compensation`]       - compensation prepare/commit
//!
//! ## Required Properties
//!
//! The test harness must prove at least:
//!
//! 1. Duplicate ingress does not create duplicate logical work
//! 2. Stale fence completions cannot win
//! 3. Replay after any injected crash reaches the same legal state
//! 4. Connector ambiguity always routes through reconciliation
//! 5. Projection rebuild reproduces the same operator state
//! 6. Lineage rollover preserves correct signal routing
//! 7. Compensation never runs for an effect that was never durably committed
//!
//! ## Test Layers
//!
//! 1. Pure state-machine tests
//! 2. Property tests for replay invariants
//! 3. Connector contract tests
//! 4. Storage crash-injection integration tests
//! 5. Black-box product-owner scenarios

pub mod crash_points;
pub mod harness;
