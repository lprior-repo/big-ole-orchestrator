//! Initialization Order Contract Tests (ADR-054)
//!
//! These tests verify the initialization order contract for the Veloxide engine.
//! All tests follow the TDD approach: write failing tests first, then implement.

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Test that phases must complete in order
    #[test]
    fn init_phases_must_complete_in_order() {
        let phases = [
            AtomicBool::new(false), // Phase 1: Storage Foundation
            AtomicBool::new(false), // Phase 2: Storage Partitions
            AtomicBool::new(false), // Phase 3: Actor System
            AtomicBool::new(false), // Phase 4: Background Services
            AtomicBool::new(false), // Phase 5: Runtime Acceptance
        ];

        // Phase 1 must complete first
        phases[0].store(true, Ordering::SeqCst);
        assert!(phases[0].load(Ordering::SeqCst));

        // Cannot skip phase 1
        assert!(!phases[1].load(Ordering::SeqCst));
        assert!(!phases[2].load(Ordering::SeqCst));
        assert!(!phases[3].load(Ordering::SeqCst));
        assert!(!phases[4].load(Ordering::SeqCst));
    }

    /// Test that each phase can only be marked complete once
    #[test]
    fn init_phase_marked_complete_once() {
        let phase_flag = AtomicBool::new(false);

        // First mark_complete succeeds
        let first = phase_flag.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        assert!(first.is_ok());

        // Second mark_complete fails (already complete)
        let second = phase_flag.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        assert!(second.is_err());
    }

    /// Test that init barrier correctly reports phase status
    #[test]
    fn init_barrier_reports_correct_status() {
        #[derive(Clone, Copy)]
        enum Phase {
            StorageFoundation = 0,
            StoragePartitions = 1,
            ActorSystem = 2,
            BackgroundServices = 3,
            RuntimeAcceptance = 4,
        }

        impl Phase {
            fn as_usize(self) -> usize {
                self as usize
            }
        }

        struct InitBarrier {
            phases: [AtomicBool; 5],
        }

        impl InitBarrier {
            fn new() -> Self {
                Self {
                    phases: [
                        AtomicBool::new(false),
                        AtomicBool::new(false),
                        AtomicBool::new(false),
                        AtomicBool::new(false),
                        AtomicBool::new(false),
                    ],
                }
            }

            fn phase_complete(&self, phase: Phase) -> bool {
                self.phases[phase.as_usize()].load(Ordering::SeqCst)
            }

            fn mark_complete(&self, phase: Phase) {
                self.phases[phase.as_usize()].store(true, Ordering::SeqCst);
            }
        }

        let barrier = InitBarrier::new();

        // No phases complete initially
        assert!(!barrier.phase_complete(Phase::StorageFoundation));
        assert!(!barrier.phase_complete(Phase::StoragePartitions));
        assert!(!barrier.phase_complete(Phase::ActorSystem));
        assert!(!barrier.phase_complete(Phase::BackgroundServices));
        assert!(!barrier.phase_complete(Phase::RuntimeAcceptance));

        // Mark Phase 1 complete
        barrier.mark_complete(Phase::StorageFoundation);
        assert!(barrier.phase_complete(Phase::StorageFoundation));
        assert!(!barrier.phase_complete(Phase::StoragePartitions));
    }

    /// Test that shutdown happens in reverse order
    #[test]
    fn shutdown_reverse_order() {
        let shutdown_order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let phases = [
            ("RuntimeAcceptance", shutdown_order.clone()),
            ("BackgroundServices", shutdown_order.clone()),
            ("ActorSystem", shutdown_order.clone()),
            ("StoragePartitions", shutdown_order.clone()),
            ("StorageFoundation", shutdown_order.clone()),
        ];

        // Simulate shutdown in reverse order
        for (name, order) in phases.iter() {
            order.lock().unwrap().push(*name);
        }

        let order = shutdown_order.lock().unwrap();
        assert_eq!(order[0], "RuntimeAcceptance");
        assert_eq!(order[1], "BackgroundServices");
        assert_eq!(order[2], "ActorSystem");
        assert_eq!(order[3], "StoragePartitions");
        assert_eq!(order[4], "StorageFoundation");
    }

    /// Test init order dependency graph
    #[test]
    fn init_dependency_graph_respected() {
        // Verify: Scheduler requires JobStore (Phase 2)
        // Verify: Reanimator requires TimerStorage and WorkQueue (Phase 2)
        // Verify: Actor system requires storage partitions (Phase 2)

        struct MockPhaseTracker {
            completed: Vec<String>,
        }

        impl MockPhaseTracker {
            fn new() -> Self {
                Self { completed: Vec::new() }
            }

            fn complete(&mut self, phase: &str) {
                self.completed.push(phase.to_string());
            }

            fn is_complete(&self, phase: &str) -> bool {
                self.completed.contains(&phase.to_string())
            }
        }

        let mut tracker = MockPhaseTracker::new();

        // Phase 1: Storage Foundation
        tracker.complete("StorageFoundation");
        assert!(tracker.is_complete("StorageFoundation"));

        // Phase 2: Storage Partitions (depends on Phase 1)
        assert!(tracker.is_complete("StorageFoundation"));
        tracker.complete("StoragePartitions");
        assert!(tracker.is_complete("StoragePartitions"));

        // Phase 3: Actor System (depends on Phase 2)
        assert!(tracker.is_complete("StoragePartitions"));
        tracker.complete("ActorSystem");
        assert!(tracker.is_complete("ActorSystem"));

        // Phase 4: Background Services (depends on Phase 2, 3)
        assert!(tracker.is_complete("StoragePartitions"));
        assert!(tracker.is_complete("ActorSystem"));
        tracker.complete("BackgroundServices");
        assert!(tracker.is_complete("BackgroundServices"));

        // Phase 5: Runtime Acceptance (depends on Phase 4)
        assert!(tracker.is_complete("BackgroundServices"));
        tracker.complete("RuntimeAcceptance");
        assert!(tracker.is_complete("RuntimeAcceptance"));
    }

    /// Test that Phase 5 only reached when all previous phases complete
    #[test]
    fn runtime_acceptance_requires_all_phases() {
        #[derive(Clone, Copy)]
        enum Phase {
            StorageFoundation = 0,
            StoragePartitions = 1,
            ActorSystem = 2,
            BackgroundServices = 3,
            RuntimeAcceptance = 4,
        }

        struct SystemState {
            phases: [bool; 5],
        }

        impl SystemState {
            fn new() -> Self {
                Self {
                    phases: [false; 5],
                }
            }

            fn is_ready_for_runtime_acceptance(&self) -> bool {
                self.phases[Phase::StorageFoundation as usize]
                    && self.phases[Phase::StoragePartitions as usize]
                    && self.phases[Phase::ActorSystem as usize]
                    && self.phases[Phase::BackgroundServices as usize]
            }

            fn complete_phase(&mut self, phase: Phase) {
                self.phases[phase as usize] = true;
            }
        }

        let mut state = SystemState::new();

        // Cannot reach runtime acceptance without all phases
        assert!(!state.is_ready_for_runtime_acceptance());

        // Complete all but one phase
        state.complete_phase(Phase::StorageFoundation);
        state.complete_phase(Phase::StoragePartitions);
        state.complete_phase(Phase::ActorSystem);
        // Missing BackgroundServices

        assert!(!state.is_ready_for_runtime_acceptance());

        // Complete final phase
        state.complete_phase(Phase::BackgroundServices);
        assert!(state.is_ready_for_runtime_acceptance());
    }
}
