use super::actions::{
    validate_projection_batch, validate_projection_payload, CompatibleProjectionIterator,
    ProjectionRecord,
};
use super::calc::{
    check_projection_compat, is_projection_compatible, projection_compat_window, window_is_valid,
    window_max_supported, window_min_supported,
};
use super::types::{ProjectionCompat, ProjectionCompatibilityWindow, ProjectionError};
use proptest::prelude::*;

// -------------------------------------------------------------------------
// projection_compat_window tests
// -------------------------------------------------------------------------

#[test]
fn projection_compat_window_returns_ok_when_min_ge_1_and_max_ge_min() {
    let result = projection_compat_window(1, 3);
    assert!(result.is_ok());
    let window = result.unwrap();
    assert!(window_is_valid(&window));
    assert_eq!(window_min_supported(&window), 1);
    assert_eq!(window_max_supported(&window), 3);
}

#[test]
fn projection_compat_window_returns_ok_for_min_equals_max() {
    let result = projection_compat_window(5, 5);
    assert!(result.is_ok());
}

#[test]
fn projection_compat_window_returns_ok_at_minimum_boundary() {
    let result = projection_compat_window(1, 1);
    assert!(result.is_ok());
}

#[test]
fn projection_compat_window_returns_window_misconfigured_when_min_is_zero() {
    let result = projection_compat_window(0, 3);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

#[test]
fn projection_compat_window_returns_window_misconfigured_when_max_lt_min() {
    let result = projection_compat_window(5, 3);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

#[test]
fn projection_compat_window_returns_window_misconfigured_when_min_gt_max() {
    let result = projection_compat_window(7, 5);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

// -------------------------------------------------------------------------
// check_projection_compat tests
// -------------------------------------------------------------------------

#[test]
fn check_projection_compat_returns_fresh_when_version_equals_max() {
    let window = projection_compat_window(2, 5).unwrap();
    let result = check_projection_compat(5, &window);
    assert_eq!(result, Ok(ProjectionCompat::Fresh));
}

#[test]
fn check_projection_compat_returns_needs_upcast_when_version_within_window() {
    let window = projection_compat_window(2, 5).unwrap();
    let result = check_projection_compat(3, &window);
    assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 3, to: 5 }));
}

#[test]
fn check_projection_compat_returns_needs_upcast_when_version_equals_min() {
    let window = projection_compat_window(2, 5).unwrap();
    let result = check_projection_compat(2, &window);
    assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 2, to: 5 }));
}

#[test]
fn check_projection_compat_returns_stale_too_old_when_version_below_min() {
    let window = projection_compat_window(3, 7).unwrap();
    let result = check_projection_compat(1, &window);
    assert_eq!(
        result,
        Ok(ProjectionCompat::StaleTooOld {
            projection: 1,
            window_min: 3
        })
    );
}

#[test]
fn check_projection_compat_returns_stale_zero_version_when_version_is_zero() {
    let window = projection_compat_window(1, 5).unwrap();
    let result = check_projection_compat(0, &window);
    assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
}

#[test]
fn check_projection_compat_returns_window_misconfigured_when_window_invalid() {
    let invalid_window = ProjectionCompatibilityWindow {
        min_supported: 0,
        max_supported: 5,
    };
    let result = check_projection_compat(3, &invalid_window);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

#[test]
fn check_projection_compat_returns_stale_too_old_when_version_exceeds_max() {
    let window = projection_compat_window(2, 5).unwrap();
    let result = check_projection_compat(10, &window);
    assert_eq!(
        result,
        Ok(ProjectionCompat::StaleTooOld {
            projection: 10,
            window_min: 2
        })
    );
}

#[test]
fn check_projection_compat_is_reflexive_at_max_for_any_valid_window() {
    let window = projection_compat_window(1, 3).unwrap();
    let result = check_projection_compat(window_max_supported(&window), &window);
    assert_eq!(result, Ok(ProjectionCompat::Fresh));
}

#[test]
fn check_projection_compat_returns_fresh_at_u8_max_boundary() {
    let window = projection_compat_window(100, u8::MAX).unwrap();
    let result = check_projection_compat(u8::MAX, &window);
    assert_eq!(result, Ok(ProjectionCompat::Fresh));
}

#[test]
fn check_projection_compat_returns_stale_too_old_when_version_exceeds_max_u8() {
    let window = projection_compat_window(1, 5).unwrap();
    let result = check_projection_compat(u8::MAX, &window);
    assert_eq!(
        result,
        Ok(ProjectionCompat::StaleTooOld {
            projection: u8::MAX,
            window_min: 1
        })
    );
}

// -------------------------------------------------------------------------
// is_projection_compatible tests
// -------------------------------------------------------------------------

#[test]
fn is_projection_compatible_returns_true_for_fresh() {
    let window = projection_compat_window(2, 5).unwrap();
    assert!(is_projection_compatible(5, &window));
}

#[test]
fn is_projection_compatible_returns_true_for_needs_upcast() {
    let window = projection_compat_window(2, 5).unwrap();
    assert!(is_projection_compatible(3, &window));
}

#[test]
fn is_projection_compatible_returns_false_for_stale_too_old() {
    let window = projection_compat_window(3, 7).unwrap();
    assert!(!is_projection_compatible(1, &window));
}

#[test]
fn is_projection_compatible_returns_false_for_stale_zero_version() {
    let window = projection_compat_window(1, 5).unwrap();
    assert!(!is_projection_compatible(0, &window));
}

#[test]
fn is_projection_compatible_returns_false_for_version_exceeding_max() {
    let window = projection_compat_window(2, 5).unwrap();
    assert!(!is_projection_compatible(10, &window));
}

#[test]
fn is_projection_compatible_returns_false_for_invalid_window() {
    let invalid_window = ProjectionCompatibilityWindow {
        min_supported: 0,
        max_supported: 5,
    };
    assert!(!is_projection_compatible(3, &invalid_window));
}

// -------------------------------------------------------------------------
// ProjectionCompat::is_compatible tests
// -------------------------------------------------------------------------

#[test]
fn projection_compat_is_compatible_returns_true_for_fresh() {
    assert!(ProjectionCompat::Fresh.is_compatible());
}

#[test]
fn projection_compat_is_compatible_returns_true_for_needs_upcast() {
    assert!(ProjectionCompat::NeedsUpcast { from: 3, to: 7 }.is_compatible());
}

#[test]
fn projection_compat_is_compatible_returns_false_for_stale_too_old() {
    assert!(!ProjectionCompat::StaleTooOld {
        projection: 1,
        window_min: 3
    }
    .is_compatible());
}

#[test]
fn projection_compat_is_compatible_returns_false_for_stale_zero_version() {
    assert!(!ProjectionCompat::StaleZeroVersion.is_compatible());
}

// -------------------------------------------------------------------------
// validate_projection_payload tests
// -------------------------------------------------------------------------

#[test]
fn validate_projection_payload_returns_fresh_when_version_matches_max() {
    let window = projection_compat_window(2, 5).unwrap();
    let payload = br#"{"version": 5, "data": "foo"}"#;
    let result = validate_projection_payload(payload, &window);
    assert_eq!(result, Ok(ProjectionCompat::Fresh));
}

#[test]
fn validate_projection_payload_returns_needs_upcast_when_version_within_window() {
    let window = projection_compat_window(2, 5).unwrap();
    let payload = br#"{"version": 3, "data": "bar"}"#;
    let result = validate_projection_payload(payload, &window);
    assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 3, to: 5 }));
}

#[test]
fn validate_projection_payload_returns_stale_too_old_when_version_below_window() {
    let window = projection_compat_window(3, 7).unwrap();
    let payload = br#"{"version": 1, "data": "old"}"#;
    let result = validate_projection_payload(payload, &window);
    assert_eq!(
        result,
        Ok(ProjectionCompat::StaleTooOld {
            projection: 1,
            window_min: 3
        })
    );
}

#[test]
fn validate_projection_payload_returns_stale_zero_version_when_version_is_zero() {
    let window = projection_compat_window(1, 5).unwrap();
    let payload = br#"{"version": 0, "data": "invalid"}"#;
    let result = validate_projection_payload(payload, &window);
    assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
}

#[test]
fn validate_projection_payload_returns_missing_schema_version_when_no_version_field() {
    let window = projection_compat_window(1, 5).unwrap();
    let payload = br#"{"data": "no_version"}"#;
    let result = validate_projection_payload(payload, &window);
    assert!(matches!(result, Err(ProjectionError::MissingSchemaVersion)));
}

#[test]
fn validate_projection_payload_returns_invalid_schema_version_type_when_version_is_string() {
    let window = projection_compat_window(1, 5).unwrap();
    let payload = br#"{"version": "5"}"#;
    let result = validate_projection_payload(payload, &window);
    assert!(matches!(
        result,
        Err(ProjectionError::InvalidSchemaVersionType)
    ));
}

#[test]
fn validate_projection_payload_returns_invalid_schema_version_type_when_version_is_null() {
    let window = projection_compat_window(1, 5).unwrap();
    let payload = br#"{"version": null}"#;
    let result = validate_projection_payload(payload, &window);
    assert!(matches!(
        result,
        Err(ProjectionError::InvalidSchemaVersionType)
    ));
}

#[test]
fn validate_projection_payload_returns_schema_version_exceeds_max_when_version_too_new() {
    let window = projection_compat_window(1, 5).unwrap();
    let payload = br#"{"version": 100}"#;
    let result = validate_projection_payload(payload, &window);
    assert!(matches!(
        result,
        Err(ProjectionError::SchemaVersionExceedsMax(100, 5))
    ));
}

// -------------------------------------------------------------------------
// validate_projection_batch tests
// -------------------------------------------------------------------------

#[test]
fn validate_projection_batch_returns_ok_for_empty_iterator() {
    let window = projection_compat_window(1, 5).unwrap();
    let payloads: Vec<&[u8]> = vec![];
    let result = validate_projection_batch(payloads, &window);
    assert!(result.is_ok());
}

#[test]
fn validate_projection_batch_returns_ok_when_all_payloads_compatible() {
    let window = projection_compat_window(2, 5).unwrap();
    let payloads = vec![
        br#"{"version": 5}"#,
        br#"{"version": 3}"#,
        br#"{"version": 2}"#,
    ];
    let result = validate_projection_batch(payloads, &window);
    assert!(result.is_ok());
}

#[test]
fn validate_projection_batch_returns_stale_projection_at_first_stale() {
    let window = projection_compat_window(2, 5).unwrap();
    let payloads = vec![
        br#"{"version": 5}"#,
        br#"{"version": 1}"#,
        br#"{"version": 3}"#,
    ];
    let result = validate_projection_batch(payloads, &window);
    assert!(matches!(
        result,
        Err(ProjectionError::StaleProjection(1, 2, 5))
    ));
}

#[test]
fn validate_projection_batch_returns_missing_schema_version_from_first_invalid_payload() {
    let window = projection_compat_window(1, 5).unwrap();
    let p1 = br#"{"version": 5}"#;
    let p2 = br#"{"data": "no_version"}"#;
    let p3 = br#"{"version": 3}"#;
    let payloads = vec![p1.as_slice(), p2.as_slice(), p3.as_slice()];
    let result = validate_projection_batch(payloads, &window);
    assert!(matches!(result, Err(ProjectionError::MissingSchemaVersion)));
}

#[test]
fn validate_projection_batch_short_circuits_on_first_error() {
    let window = projection_compat_window(2, 5).unwrap();
    let payloads = vec![
        br#"{"version": 5}"#,
        br#"{"version": 1}"#,
        br#"{"version": 0}"#,
    ];
    let result = validate_projection_batch(payloads, &window);
    // Should return first stale (version 1), not StaleZeroVersion
    assert!(matches!(
        result,
        Err(ProjectionError::StaleProjection(1, 2, 5))
    ));
}

#[test]
fn validate_projection_batch_returns_window_misconfigured_for_invalid_window() {
    let invalid_window = ProjectionCompatibilityWindow {
        min_supported: 0,
        max_supported: 5,
    };
    let payloads = vec![br#"{"version": 3}"#];
    let result = validate_projection_batch(payloads, &invalid_window);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

// -------------------------------------------------------------------------
// CompatibleProjectionIterator tests
// -------------------------------------------------------------------------

#[test]
fn compatible_projection_iterator_constructs_with_valid_window() {
    let window = projection_compat_window(2, 5).unwrap();
    let inner = std::iter::empty::<Result<ProjectionRecord, &'static str>>();
    let result = CompatibleProjectionIterator::new(inner, window);
    assert!(result.is_ok());
}

#[test]
fn compatible_projection_iterator_returns_window_misconfigured_for_invalid_window() {
    let invalid_window = ProjectionCompatibilityWindow {
        min_supported: 0,
        max_supported: 5,
    };
    let inner = std::iter::empty::<Result<ProjectionRecord, &'static str>>();
    let result = CompatibleProjectionIterator::new(inner, invalid_window);
    assert!(matches!(
        result,
        Err(ProjectionError::WindowMisconfigured { .. })
    ));
}

#[test]
fn compatible_projection_iterator_wraps_any_iterator_type() {
    let window = projection_compat_window(1, 5).unwrap();
    let inner: Vec<Result<ProjectionRecord, &'static str>> =
        vec![Ok(ProjectionRecord::new(5, vec![]))];
    let result = CompatibleProjectionIterator::new(inner.into_iter(), window);
    assert!(result.is_ok());
}

#[test]
fn compatible_projection_iterator_is_send_and_sync() {
    let window = projection_compat_window(1, 5).unwrap();
    let inner: Vec<Result<ProjectionRecord, &'static str>> = vec![];
    let iterator = CompatibleProjectionIterator::new(inner.into_iter(), window).unwrap();
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&iterator);
}

// -------------------------------------------------------------------------
// Invariant tests
// -------------------------------------------------------------------------

#[test]
fn invariant_zero_is_always_stale() {
    let windows = [
        projection_compat_window(1, 1).unwrap(),
        projection_compat_window(1, 5).unwrap(),
        projection_compat_window(100, u8::MAX).unwrap(),
    ];
    for window in windows {
        let result = check_projection_compat(0, &window);
        assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
    }
}

#[test]
fn invariant_fresh_is_always_compatible() {
    let windows = [
        projection_compat_window(1, 1).unwrap(),
        projection_compat_window(2, 5).unwrap(),
        projection_compat_window(100, u8::MAX).unwrap(),
    ];
    for window in windows {
        let result = check_projection_compat(window_max_supported(&window), &window);
        assert!(result.is_ok());
        assert!(result.unwrap().is_compatible());
    }
}

// -------------------------------------------------------------------------
// proptest invariants
// -------------------------------------------------------------------------

proptest! {
    #[test]
    fn proptest_check_projection_compat_partition_exhaustive(
        window_min in 1u8..=u8::MAX,
        window_max in 1u8..=u8::MAX,
        version in 0u8..=u8::MAX
    ) {
        prop_assume!(window_max >= window_min);
        let window = projection_compat_window(window_min, window_max).unwrap();
        let result = check_projection_compat(version, &window);
        prop_assert!(result.is_ok());

        let compat = result.unwrap();

        // Exactly one variant should match
        let is_fresh = matches!(compat, ProjectionCompat::Fresh);
        let is_needs_upcast = matches!(compat, ProjectionCompat::NeedsUpcast { .. });
        let is_stale_too_old = matches!(compat, ProjectionCompat::StaleTooOld { .. });
        let is_stale_zero = matches!(compat, ProjectionCompat::StaleZeroVersion);

        let variant_count = u8::from(is_fresh) + u8::from(is_needs_upcast)
            + u8::from(is_stale_too_old) + u8::from(is_stale_zero);

        prop_assert_eq!(variant_count, 1);
    }

    #[test]
    fn proptest_window_constructor_validates_preconditions(
        window_min in 0u8..=u8::MAX,
        window_max in 0u8..=u8::MAX
    ) {
        let result = projection_compat_window(window_min, window_max);
        if window_min >= 1 && window_max >= window_min {
            prop_assert!(result.is_ok());
            let window = result.unwrap();
            prop_assert!(window_is_valid(&window));
        } else {
            prop_assert!(result.is_err());
        }
    }

    #[test]
    fn proptest_is_projection_compatible_matches_check_projection_compat(
        window_min in 1u8..=u8::MAX,
        window_max in 1u8..=u8::MAX,
        version in 0u8..=u8::MAX
    ) {
        prop_assume!(window_max >= window_min);
        let window = projection_compat_window(window_min, window_max).unwrap();
        let compat_result = check_projection_compat(version, &window);
        let is_compat = is_projection_compatible(version, &window);

        prop_assert_eq!(compat_result.is_ok_and(super::types::ProjectionCompat::is_compatible), is_compat);
    }

    #[test]
    fn proptest_needs_upcast_from_to_consistency(
        window_min in 1u8..=254u8,
        window_max in 1u8..=u8::MAX,
        version in 0u8..=u8::MAX
    ) {
        prop_assume!(window_max >= window_min);
        let window = projection_compat_window(window_min, window_max).unwrap();
        let result = check_projection_compat(version, &window);

        if let Ok(ProjectionCompat::NeedsUpcast { from, to }) = result {
            prop_assert!(from < to);
            prop_assert_eq!(to, window_max);
        }
    }
}
