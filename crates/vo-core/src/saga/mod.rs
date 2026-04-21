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
//!
//! ## Nested Saga Support
//!
//! Sagas can contain nested sagas as steps. When a parent saga fails, nested
//! sagas are compensated in reverse dependency order.
//!
//! ```text
//! ParentSaga
//!   ├── Step 1: Effect E1 → Compensation C1
//!   ├── Step 2: NestedSaga A
//!   │           ├── Effect A1 → Compensation CA1
//!   │           └── Effect A2 → Compensation CA2
//!   └── Step 3: Effect E3 → Compensation C3
//!
//! Compensation order on failure: C3 → CA2 → CA1 → C1
//! ```

pub mod nested;
pub mod state;

pub use nested::{HierarchicalSaga, NestedSaga};
pub use state::{CompensationState, CompensationTransitionError, CompensationTransitionEvent};
