mod admission_multi_indicator_boundary {
    use crate::admission::types::{
        AdmissionError, AdmissionThresholds, PressureIndicator, WritePressureState,
    };

    #[test]
    fn stall_and_queue_depth_triggers_multiple_indicators() {
        let state = WritePressureState {
            writer_queue_depth: 200,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: true,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
        };
        let result =
            crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(matches!(
            result,
            Err(AdmissionError::MultiplePressureIndicators { .. })
        ));
        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            assert!(indicators.contains(&PressureIndicator::WriterQueueDepth));
            assert!(indicators.contains(&PressureIndicator::CompactionStall));
            assert_eq!(indicators.len(), 2);
        }
    }

    #[test]
    fn all_three_queues_exceeded_triggers_multiple() {
        let state = WritePressureState {
            writer_queue_depth: 200,
            batch_commit_latency_ms: 2000,
            blob_queue_depth: 100,
            compaction_stall_active: false,
            storage_stall_active: true,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 100,
            batch_commit_latency_ms_threshold: 1000,
            blob_queue_depth_threshold: 50,
        };
        let result =
            crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(matches!(
            result,
            Err(AdmissionError::MultiplePressureIndicators { .. })
        ));
        if let Err(AdmissionError::MultiplePressureIndicators { indicators }) = result {
            assert_eq!(indicators.len(), 4);
        }
    }

    #[test]
    fn zero_state_zero_thresholds_no_stalls_passes() {
        let state = WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        };
        let thresholds = AdmissionThresholds {
            writer_queue_depth_threshold: 0,
            batch_commit_latency_ms_threshold: 0,
            blob_queue_depth_threshold: 0,
        };
        let result =
            crate::admission::check::check_admission_with_thresholds(&state, &thresholds);
        assert!(result.is_ok());
    }
}