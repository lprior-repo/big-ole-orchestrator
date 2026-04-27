mod workspace_swap_boundary {
    use crate::workspace_swap::{AtomicSwap, SwapError, SwapPhase, SwapStatus};
    use std::fs;

    #[test]
    fn swap_status_no_prior_swap_when_no_journal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn swap_status_incomplete_after_stage() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );
    }

    #[test]
    fn swap_status_complete_after_full_swap() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "data").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn swap_error_display_all_variants() {
        let errors = vec![
            SwapError::NotADirectory("/tmp/x".into()),
            SwapError::WorkspaceNotFound("/tmp/y".into()),
            SwapError::ShadowExists("/tmp/z".into()),
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }

    #[test]
    fn swap_status_equality() {
        assert_eq!(SwapStatus::NoPriorSwap, SwapStatus::NoPriorSwap);
        assert_eq!(SwapStatus::Complete, SwapStatus::Complete);
        assert_eq!(
            SwapStatus::Incomplete(SwapPhase::Staging),
            SwapStatus::Incomplete(SwapPhase::Staging)
        );
        assert_ne!(SwapStatus::NoPriorSwap, SwapStatus::Complete);
    }

    #[test]
    fn swap_stage_fails_for_missing_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent");
        let swap = AtomicSwap::new(&missing);
        let result = swap.stage();
        assert!(matches!(result, Err(SwapError::WorkspaceNotFound(_))));
    }
}