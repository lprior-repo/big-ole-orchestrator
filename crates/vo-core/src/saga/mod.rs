//! Saga compensation lifecycle state machine (ADR-034).
//!
//! This module implements the compensation lifecycle state machine for managed effect nodes.
//! Each compensation goes through states: Pending → Executing → Completed/Failed
//!
//! ## State Diagram
//!
//! ```text
//!      ┌─────────┐
//!      │ Pending │
//!      └────┬────┘
//!           │ start()
//!           ▼
//!      ┌───────────┐
//! ────▶│ Executing │◀─────────
//!      └───┬───────┘          │
//!          │ complete()       │ fail()
//          ▼                   │
//     ┌───────────┐            │
//     │ Completed │ (terminal)  │
//     └───────────┘            │
//                              ┌────────┐
//                              │ Failed │ (terminal)
//                              └────────┘
//! ```
//!
//! ## Invariants
//!
//! - A compensation cannot move to Completed without passing through Executing
//! - IF a compensation is already Completed, THE SYSTEM SHALL NOT re-execute it

pub mod state;

pub use state::{CompensationState, CompensationTransitionError, CompensationTransitionEvent};
