#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_core::admission::{
    check_admission_with_thresholds, AdmissionThresholds, WritePressureState,
};

/// Fuzz target for AdmissionThresholds JSON deserialization.
///
/// This target tests that the admission check functions handle arbitrary JSON input
/// without panicking. It fuzzes the threshold deserialization and subsequent
/// admission check call.
///
/// Risk class:
/// - Panic due to malformed JSON
/// - Panic due to missing fields
/// - Panic due to type mismatches (string instead of u64)
/// - OOM due to deeply nested structures
/// - Logic error if negative values are silently allowed
#[derive(serde::Deserialize)]
struct ThresholdJson {
    writer_queue_depth_threshold: Option<u64>,
    batch_commit_latency_ms_threshold: Option<u64>,
    blob_queue_depth_threshold: Option<u64>,
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(json) = serde_json::from_str::<ThresholdJson>(s) {
            if let (Some(w), Some(b), Some(bl)) = (
                json.writer_queue_depth_threshold,
                json.batch_commit_latency_ms_threshold,
                json.blob_queue_depth_threshold,
            ) {
                let thresholds = AdmissionThresholds {
                    writer_queue_depth_threshold: w,
                    batch_commit_latency_ms_threshold: b,
                    blob_queue_depth_threshold: bl,
                };
                // Use default state - all zeros
                let state = WritePressureState::default();
                let _ = check_admission_with_thresholds(&state, &thresholds);
            }
        }
    }
});
