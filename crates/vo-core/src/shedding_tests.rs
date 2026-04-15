//! Tests for load shedding semaphore.
//!
//! Implements ADR-006 load shedding validation.

use crate::shedding::{
    LoadSheddingSemaphore, SemaphoreLimitError, MAX_CONCURRENT_BINARIES, MAX_YIELDED_ACTORS,
};

#[tokio::test]
async fn test_task_admitted_when_semaphore_has_permits() {
    let semaphore = LoadSheddingSemaphore::with_default_limit();
    let initial_permits = semaphore.available_permits();
    assert_eq!(initial_permits, MAX_CONCURRENT_BINARIES);

    let result = semaphore.try_acquire();
    assert!(result.is_ok());
    assert_eq!(semaphore.available_permits(), initial_permits - 1);
}

#[tokio::test]
async fn test_task_admitted_when_semaphore_has_permits_duplicate_for_schema() {
    let semaphore = LoadSheddingSemaphore::new(10);
    assert_eq!(semaphore.available_permits(), 10);

    let result1 = semaphore.try_acquire();
    assert!(result1.is_ok());

    let result2 = semaphore.try_acquire();
    assert!(result2.is_ok());

    assert_eq!(semaphore.available_permits(), 8);
}

#[tokio::test]
async fn test_task_rejected_when_semaphore_is_exhausted() {
    let semaphore = LoadSheddingSemaphore::new(2);

    let _permit1 = semaphore.try_acquire().expect("should acquire");
    let _permit2 = semaphore.try_acquire().expect("should acquire");

    let result = semaphore.try_acquire();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SemaphoreLimitError::LimitReached { .. }));
    assert!(!err.is_load_shedding());
}

#[tokio::test]
async fn test_task_rejected_when_semaphore_is_exhausted_duplicate_for_schema() {
    let semaphore = LoadSheddingSemaphore::new(1);

    let _permit = semaphore.try_acquire().expect("should acquire");

    let result = semaphore.try_acquire();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SemaphoreLimitError::LimitReached { current_permits: 0, requested: 1 }));
}

#[tokio::test]
async fn test_permit_release_restores_permits() {
    let semaphore = LoadSheddingSemaphore::new(5);
    assert_eq!(semaphore.available_permits(), 5);

    {
        let _permit = semaphore.try_acquire().expect("should acquire");
        assert_eq!(semaphore.available_permits(), 4);
    }

    assert_eq!(semaphore.available_permits(), 5);
}

#[tokio::test]
async fn test_acquired_count_tracks_permits() {
    let semaphore = LoadSheddingSemaphore::new(100);
    assert_eq!(semaphore.acquired_count(), 0);

    let _p1 = semaphore.try_acquire().expect("should acquire");
    assert_eq!(semaphore.acquired_count(), 1);

    let _p2 = semaphore.try_acquire().expect("should acquire");
    assert_eq!(semaphore.acquired_count(), 2);

    drop(_p1);
    assert_eq!(semaphore.acquired_count(), 1);

    drop(_p2);
    assert_eq!(semaphore.acquired_count(), 0);
}

#[tokio::test]
async fn test_load_shedding_threshold_not_exceeded() {
    let semaphore = LoadSheddingSemaphore::new(MAX_CONCURRENT_BINARIES);

    for _ in 0..100 {
        let _permit = semaphore.try_acquire().expect("should acquire");
    }

    let result = semaphore.check_load_shedding_threshold(MAX_YIELDED_ACTORS);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_load_shedding_threshold_exceeded() {
    let semaphore = LoadSheddingSemaphore::new(10);

    for _ in 0..10 {
        let _permit = semaphore.try_acquire().expect("should acquire");
    }

    let result = semaphore.check_load_shedding_threshold(5);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_load_shedding());
}

#[tokio::test]
async fn test_max_permits_constant() {
    let semaphore = LoadSheddingSemaphore::with_default_limit();
    assert_eq!(semaphore.max_permits(), MAX_CONCURRENT_BINARIES);
}

#[tokio::test]
async fn test_semaphore_limit_error_display() {
    let err = SemaphoreLimitError::LimitReached {
        current_permits: 0,
        requested: 1,
    };
    let display = err.to_string();
    assert!(display.contains("semaphore limit reached"));
    assert!(display.contains("0"));
    assert!(display.contains("1"));
}

#[tokio::test]
async fn test_load_shedding_error_display() {
    let err = SemaphoreLimitError::LoadSheddingActive {
        yielded_actors: 5000,
        threshold: MAX_YIELDED_ACTORS,
    };
    let display = err.to_string();
    assert!(display.contains("load shedding active"));
    assert!(display.contains("5000"));
}

#[tokio::test]
async fn test_try_acquire_many_success() {
    let semaphore = LoadSheddingSemaphore::new(10);
    let result = semaphore.try_acquire_many(5);
    assert!(result.is_ok());
    assert_eq!(semaphore.available_permits(), 5);
}

#[tokio::test]
async fn test_try_acquire_many_exceeds_available() {
    let semaphore = LoadSheddingSemaphore::new(3);
    let result = semaphore.try_acquire_many(5);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SemaphoreLimitError::LimitReached {
            current_permits: 3,
            requested: 5
        }
    ));
}

#[tokio::test]
async fn test_is_load_shedding_active() {
    let semaphore = LoadSheddingSemaphore::new(5);

    assert!(!semaphore.is_load_shedding_active(3));

    for _ in 0..3 {
        let _p = semaphore.try_acquire().expect("should acquire");
    }
    assert!(semaphore.is_load_shedding_active(3));
    assert!(!semaphore.is_load_shedding_active(4));
}

#[tokio::test]
async fn test_load_shedding_check_with_default_threshold() {
    let semaphore = LoadSheddingSemaphore::new(100);

    for _ in 0..50 {
        let _p = semaphore.try_acquire().expect("should acquire");
    }

    let result = semaphore.check_load_shedding();
    assert!(result.is_ok());
}
