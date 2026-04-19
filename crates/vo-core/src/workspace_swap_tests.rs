#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn atomic_swap_creates_shadow_then_commits() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "hello").unwrap();

        let swap = AtomicSwap::new(&workspace);

        let phase = swap.stage().unwrap();
        assert_eq!(phase, SwapPhase::Staged);

        let shadow = swap.shadow_path();
        assert!(shadow.exists());
        assert!(shadow.join("file.txt").exists());
        assert_eq!(
            fs::read_to_string(shadow.join("file.txt")).unwrap(),
            "hello"
        );

        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );

        swap.commit().unwrap();

        assert!(!shadow.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn atomic_swap_preserves_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        let nested = workspace.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "deep content").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(
            fs::read_to_string(workspace.join("a/b/c/deep.txt")).unwrap(),
            "deep content"
        );
    }

    #[test]
    fn stage_fails_if_shadow_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let swap2 = AtomicSwap::new(&workspace);
        assert!(matches!(swap2.stage(), Err(SwapError::ShadowExists(_))));
    }

    #[test]
    fn commit_is_idempotent_when_no_prior_swap() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert!(swap.commit().is_ok());
    }

    #[test]
    fn stage_fails_if_workspace_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent");

        let swap = AtomicSwap::new(&missing);
        assert!(matches!(swap.stage(), Err(SwapError::WorkspaceNotFound(_))));
    }

    #[test]
    fn stage_fails_if_path_is_file_not_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "data").unwrap();

        let swap = AtomicSwap::new(&file);
        assert!(matches!(swap.stage(), Err(SwapError::NotADirectory(_))));
    }

    #[test]
    fn commit_idempotent_when_already_complete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "data").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert!(swap.commit().is_ok());
    }

    #[test]
    fn check_status_reports_no_prior_swap_initially() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn check_status_reports_incomplete_after_stage() {
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
    fn check_status_reports_no_prior_after_commit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn recover_rolls_back_from_staging_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "staging").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!swap.shadow_path().exists());
        assert!(!journal.exists());
    }

    #[test]
    fn recover_rolls_back_from_swapping_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn recover_restores_backup_when_workspace_missing_during_swapping() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let backup = swap.backup_path();
        let shadow = swap.shadow_path();
        let journal = swap.journal_path();

        fs::rename(&workspace, &backup).unwrap();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert!(workspace.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!shadow.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn recover_returns_nothing_when_no_journal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.recover().unwrap(), RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn atomic_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("test.txt"), "content").unwrap();

        assert!(atomic_swap(&workspace).is_ok());

        assert_eq!(
            fs::read_to_string(workspace.join("test.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn atomic_swap_returns_recovery_needed_on_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let result = atomic_swap(&workspace);
        assert!(matches!(result, Err(SwapError::RecoveryNeeded(_))));
    }

    #[test]
    fn recover_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let outcome = recover_swap(&workspace).unwrap();
        assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn swap_phase_roundtrip() {
        assert_eq!(
            SwapPhase::from_str_lossy("staging"),
            Some(SwapPhase::Staging)
        );
        assert_eq!(SwapPhase::from_str_lossy("staged"), Some(SwapPhase::Staged));
        assert_eq!(
            SwapPhase::from_str_lossy("swapping"),
            Some(SwapPhase::Swapping)
        );
        assert_eq!(
            SwapPhase::from_str_lossy("complete"),
            Some(SwapPhase::Complete)
        );
        assert_eq!(SwapPhase::from_str_lossy("garbage"), None);
        assert_eq!(SwapPhase::from_str_lossy(""), None);
    }

    #[test]
    fn with_shadow_suffix_uses_custom_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::with_shadow_suffix(&workspace, ".custom-shadow");
        swap.stage().unwrap();

        assert!(dir.path().join("ws.custom-shadow").exists());
        assert!(!dir.path().join("ws.shadow").exists());
    }

    #[test]
    fn workspace_accessor_returns_original_path() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.workspace(), workspace);
    }
}
