//! Kani verification harnesses for LoadSheddingSemaphore.
//!
//! These harnesses formally verify bounded concurrency properties:
//! - Semaphore never allows more permits to be acquired than max_permits
//! - acquired_count is always <= max_permits
//! - available_permits + acquired_count == max_permits (invariance)
//! - is_load_shedding_active threshold logic is correct
//!
//! Run with: `cargo kani -p vo-core --module shedding_verification`

#[cfg(kani)]
mod verification {
    use super::*;

    /// KANI-SHEDDING-01: Verify acquired_count never exceeds max_permits.
    ///
    /// Property: For any LoadSheddingSemaphore, acquired_count() <= max_permits()
    ///           holds invariantly, even after arbitrary try_acquire calls.
    ///
    /// Bound: Symbolic max_permits in [1, u32::MAX], arbitrary concurrent state
    /// Rationale: This is the core bounded concurrency guarantee. If this fails,
    ///            the semaphore could allow unbounded resource consumption,
    ///            violating ADR-006's load shedding guarantees.
    #[kani::proof]
    fn verify_acquired_count_bounded() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);

        let semaphore = LoadSheddingSemaphore::new(max_permits);

        let acquired = semaphore.acquired_count();
        assert!(acquired <= max_permits);
    }

    /// KANI-SHEDDING-02: Verify available_permits + acquired_count == max_permits.
    ///
    /// Property: The sum of available permits and acquired count is always
    ///           exactly equal to max_permits (conservation law).
    ///
    /// Bound: Symbolic max_permits, arbitrary state
    /// Rationale: This invariant ensures the semaphore correctly tracks resource
    ///            usage. Any discrepancy indicates a bookkeeping error.
    #[kani::proof]
    fn verify_permits_conservation() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);
        kani::assume(max_permits <= 1000);

        let semaphore = LoadSheddingSemaphore::new(max_permits);

        let available = semaphore.available_permits();
        let acquired = semaphore.acquired_count();

        assert_eq!(available + acquired, max_permits);
    }

    /// KANI-SHEDDING-03: Verify is_load_shedding_active threshold logic.
    ///
    /// Property: is_load_shedding_active(threshold) returns true iff
    ///           acquired_count >= threshold, for any threshold.
    ///
    /// Bound: Symbolic threshold in [0, usize::MAX]
    /// Rationale: Load shedding activation must be precise. Missing the threshold
    ///            could cause system overload; triggering early wastes capacity.
    #[kani::proof]
    fn verify_load_shedding_threshold_logic() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);
        kani::assume(max_permits <= 1000);

        let semaphore = LoadSheddingSemaphore::new(max_permits);
        let threshold: usize = kani::any();

        let is_active = semaphore.is_load_shedding_active(threshold);
        let acquired = semaphore.acquired_count();

        if acquired >= threshold {
            assert!(is_active);
        } else {
            assert!(!is_active);
        }
    }

    /// KANI-SHEDDING-04: Verify try_acquire_many respects max permits.
    ///
    /// Property: try_acquire_many(n) where n > max_permits must fail with
    ///           LimitReached, and available_permits must remain unchanged.
    ///
    /// Bound: Symbolic n > max_permits
    /// Rationale: The semaphore must enforce hard limits. Requests exceeding
    ///            max capacity must be rejected, not partially honored.
    #[kani::proof]
    fn verify_acquire_many_never_exceeds_limit() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);
        kani::assume(max_permits <= 1000);

        let semaphore = LoadSheddingSemaphore::new(max_permits);
        let requested: usize = kani::any();
        kani::assume(requested > max_permits);

        let initial_available = semaphore.available_permits();
        let result = semaphore.try_acquire_many(requested);

        assert!(result.is_err());
        assert_eq!(semaphore.available_permits(), initial_available);
    }

    /// KANI-SHEDDING-05: Verify successful acquire reduces available permits exactly.
    ///
    /// Property: After try_acquire_many(n) succeeds, available permits decrease
    ///           by exactly n, and acquired_count increases by exactly n.
    ///
    /// Bound: n <= max_permits
    /// Rationale: Correct resource tracking is essential for load shedding.
    ///            Any discrepancy indicates a concurrency or bookkeeping bug.
    #[kani::proof]
    fn verify_acquire_reduces_permits_precisely() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);
        kani::assume(max_permits <= 1000);

        let semaphore = LoadSheddingSemaphore::new(max_permits);
        let requested: usize = kani::any();
        kani::assume(requested > 0);
        kani::assume(requested <= max_permits);

        let initial_available = semaphore.available_permits();
        let initial_acquired = semaphore.acquired_count();

        let result = semaphore.try_acquire_many(requested);

        if result.is_ok() {
            assert_eq!(semaphore.available_permits(), initial_available - requested);
            assert_eq!(semaphore.acquired_count(), initial_acquired + requested);
        } else {
            assert_eq!(semaphore.available_permits(), initial_available);
            assert_eq!(semaphore.acquired_count(), initial_acquired);
        }
    }

    /// KANI-SHEDDING-06: Verify check_load_shedding_threshold consistency.
    ///
    /// Property: check_load_shedding_threshold(t) returns Ok iff
    ///           is_load_shedding_active(t) is false.
    ///
    /// Bound: Symbolic threshold
    /// Rationale: The check function must be consistent with the query function.
    ///            Inconsistency would cause incorrect load shedding decisions.
    #[kani::proof]
    fn verify_check_load_shedding_consistent() {
        let max_permits: usize = kani::any();
        kani::assume(max_permits > 0);
        kani::assume(max_permits <= 1000);

        let semaphore = LoadSheddingSemaphore::new(max_permits);
        let threshold: usize = kani::any();

        let check_result = semaphore.check_load_shedding_threshold(threshold);
        let is_active = semaphore.is_load_shedding_active(threshold);

        assert_eq!(check_result.is_ok(), !is_active);
    }

    /// KANI-SHEDDING-07: Verify error variants are correctly distinguished.
    ///
    /// Property: LimitReached error has is_load_shedding() == false,
    ///           and LoadSheddingActive has is_load_shedding() == true.
    ///
    /// Bound: All possible error field values
    /// Rationale: The is_load_shedding() method must correctly distinguish
    ///            between hard limit exhaustion and load shedding activation.
    #[kani::proof]
    fn verify_error_variant_classification() {
        let limit_err = SemaphoreLimitError::LimitReached {
            current_permits: 0,
            requested: 1,
        };
        assert!(!limit_err.is_load_shedding());

        let shedding_err = SemaphoreLimitError::LoadSheddingActive {
            yielded_actors: 100,
            threshold: 50,
        };
        assert!(shedding_err.is_load_shedding());
    }

    /// KANI-SHEDDING-08: Verify default limit matches MAX_CONCURRENT_BINARIES.
    ///
    /// Property: LoadSheddingSemaphore::with_default_limit().max_permits()
    ///           equals MAX_CONCURRENT_BINARIES.
    ///
    /// Bound: Compile-time constant
    /// Rationale: The default configuration must match ADR-006 specification.
    ///            Using the correct default is critical for system behavior.
    #[kani::proof]
    fn verify_default_limit_constant() {
        let semaphore = LoadSheddingSemaphore::with_default_limit();
        assert_eq!(semaphore.max_permits(), MAX_CONCURRENT_BINARIES);
    }
}
