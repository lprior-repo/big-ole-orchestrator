//! Crash-injection macros for exact-once verification (ADR-043).
//!
//! These macros provide ergonomic syntax for injecting crashes at defined
//! crash points within the state machine transition logic.
//!
//! ## Usage
//!
//! ```ignore
//! use exact_once_verification::crash_injection;
//!
//! // Wrap a state transition with crash injection
//! crash_injection!(harness, CrashPoint::StepScheduled, CrashPosition::Before, {
//!     state_machine.transition_to(StepScheduled)
//! });
//!
//! // Assert post-crash invariants
//! crash_injection_assert!(harness, CrashPoint::EffectCommitted, {
//!     assert!(!effects.contains(duplicate_effect));
//! });
//! ```

use crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition};

/// Executes the inner block only if no crash should be injected at this point.
///
/// # Arguments
/// * `$harness` - The VerificationHarness instance
/// * `$point` - The CrashPoint to check
/// * `$position` - The CrashPosition (Before/After)
/// * `$block` - The code block to execute if no crash
///
/// # Example
/// ```ignore
/// crash_injection!(harness, CrashPoint::DedupeWrite, CrashPosition::Before, {
///     dedupe.insert(key, value);
/// });
/// ```
#[macro_export]
macro_rules! crash_injection {
    ($harness:expr, $point:expr, $position:expr, $block:block) => {{
        if !$harness.should_crash_at($point, $position) {
            $block
        } else {
            // Crash injection point reached - panic with structured message
            // This simulates a crash at the exact point
            panic!(concat!(
                "CRASH_INJECTED: ",
                stringify!($point),
                "/",
                stringify!($position)
            ),);
        }
    }};
}

/// Variant that returns a Result instead of panicking.
///
/// Use this when the inner block returns a Result and you want to propagate
/// the crash as an error rather than panicking.
#[macro_export]
macro_rules! crash_injection_result {
    ($harness:expr, $point:expr, $position:expr, $block:block) => {{
        if !$harness.should_crash_at($point, $position) {
            $block
        } else {
            Err($crate::exact_once_verification::macros::CrashError::injected($point, $position))
        }
    }};
}

/// Assert that invariants hold at a given crash point.
///
/// This macro is used in tests to verify that specific invariants are
/// maintained regardless of where a crash occurs in the transition.
#[macro_export]
macro_rules! crash_invariant_assert {
    ($harness:expr, $point:expr, $invariant:expr) => {{
        let point_name = $point.name();
        assert!(
            $invariant,
            "Invariant violated at crash point: {}",
            point_name
        );
    }};
}

/// Generates a test case for each crash point × position combination.
///
/// # Arguments
/// * `$name_prefix` - Prefix for the generated test name
/// * `$harness_expr` - Expression that creates the VerificationHarness
/// * `$test_block` - The test block to execute
///
/// # Example
/// ```ignore
/// crash_point_matrix_tests! {
///     "dedupe_write" => harness,
///     test_dedupe_crash_before {
///         // test logic
///     }
/// }
/// ```
#[macro_export]
macro_rules! crash_point_matrix_tests {
    ($name_prefix:expr => $harness:ident, $($test_name:ident => $block:block)*) => {
        mod crash_point_matrix {
            use super::*;
            use $crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition};

            $(
                #[test]
                fn #test_name() {
                    let $harness = VerificationHarness::new();
                    $block
                }
            )*
        }
    };
}

/// Runs the inner block with crash injection enabled at every crash point sequentially.
///
/// This is useful for stress testing where you want to verify that the
/// system can recover from crashes at any point.
#[macro_export]
macro_rules! crash_injection_stress {
    ($harness:expr, $point:expr, $block:block) => {{
        for position in CrashPosition::all() {
            let stress_harness = VerificationHarness::with_crash_scenario($point, *position);
            if stress_harness.should_crash($point) {
                $block
            }
        }
    }};
}

/// Waits for a condition with crash injection at each wait point.
///
/// This is useful for testing race conditions and ensuring that
/// the system handles crashes during waiting periods correctly.
#[macro_export]
macro_rules! crash_injection_wait {
    ($harness:expr, $point:expr, $condition:expr, $timeout:expr) => {{
        let start = std::time::Instant::now();
        while !$condition {
            if start.elapsed() > $timeout {
                return Err(
                    $crate::exact_once_verification::macros::CrashError::with_message(
                        $point,
                        CrashPosition::Before,
                        format!("timeout elapsed: {:?}", start.elapsed()),
                    ),
                );
            }
            // Check for crash injection at TimerPersistence points
            crash_injection!($harness, $point, CrashPosition::Before, {});
        }
        Ok(())
    }};
}

/// Indicates a crash was intentionally injected during testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashError {
    pub point: CrashPoint,
    pub position: CrashPosition,
    pub message: Option<String>,
}

impl CrashError {
    pub fn injected(point: CrashPoint, position: CrashPosition) -> Self {
        Self {
            point,
            position,
            message: None,
        }
    }

    pub fn with_message(point: CrashPoint, position: CrashPosition, msg: String) -> Self {
        Self {
            point,
            position,
            message: Some(msg),
        }
    }
}

impl std::fmt::Display for CrashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CrashError: {} @ {}", self.point, self.position)?;
        if let Some(msg) = &self.message {
            write!(f, " - {}", msg)?;
        }
        Ok(())
    }
}

impl std::error::Error for CrashError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_once_verification::harness::VerificationHarness;

    #[test]
    fn crash_error_display() {
        let err = CrashError::injected(CrashPoint::DedupeWrite, CrashPosition::Before);
        assert_eq!(format!("{}", err), "CrashError: DedupeWrite @ Before");

        let err_with_msg = CrashError::with_message(
            CrashPoint::StepCompleted,
            CrashPosition::After,
            "test".into(),
        );
        assert_eq!(
            format!("{}", err_with_msg),
            "CrashError: StepCompleted @ After - test"
        );
    }

    #[test]
    fn crash_injection_no_crash_when_disabled() {
        let harness = VerificationHarness::new();
        let mut counter = 0;

        crash_injection!(harness, CrashPoint::DedupeWrite, CrashPosition::Before, {
            counter += 1;
        });

        assert_eq!(counter, 1);
    }

    #[test]
    #[should_panic(expected = "CRASH_INJECTED: CrashPoint::DedupeWrite/CrashPosition::Before")]
    fn crash_injection_panics_when_enabled() {
        let harness = VerificationHarness::with_crash_scenario(
            CrashPoint::DedupeWrite,
            CrashPosition::Before,
        );

        crash_injection!(harness, CrashPoint::DedupeWrite, CrashPosition::Before, {
            panic!("This should not be reached");
        });
    }

    #[test]
    fn crash_injection_result_ok() {
        let harness = VerificationHarness::new();
        let result: Result<i32, CrashError> = crash_injection_result!(
            harness,
            CrashPoint::FenceAcquisition,
            CrashPosition::Before,
            { Ok(42) }
        );

        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn crash_injection_result_err() {
        let harness = VerificationHarness::with_crash_scenario(
            CrashPoint::FenceAcquisition,
            CrashPosition::Before,
        );

        let result: Result<i32, CrashError> = crash_injection_result!(
            harness,
            CrashPoint::FenceAcquisition,
            CrashPosition::Before,
            { Ok(42) }
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.point, CrashPoint::FenceAcquisition);
        assert_eq!(err.position, CrashPosition::Before);
    }

    #[test]
    fn crash_invariant_assert_passes() {
        let harness = VerificationHarness::new();
        crash_invariant_assert!(harness, CrashPoint::StepScheduled, true);
    }

    #[test]
    #[should_panic(expected = "Invariant violated at crash point: StepScheduled")]
    fn crash_invariant_assert_fails() {
        let harness = VerificationHarness::new();
        crash_invariant_assert!(harness, CrashPoint::StepScheduled, false);
    }
}
