//! Edge-case tests for workspace_swap module.
//!
//! Covers boundary conditions, adversarial inputs, and uncommon paths
//! that the standard tests don't exercise.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::TempDir;
use vo_core::workspace_swap::{
    atomic_swap, recover_swap, AtomicSwap, RecoveryOutcome, SwapError, SwapPhase, SwapStatus,
};

fn create_workspace_with_files(dir: &TempDir, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let workspace = dir.path().join(name);
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");
    for (filename, content) in files {
        if let Some(parent) = PathBuf::from(filename).parent() {
            if parent != std::path::Path::new("") {
                fs::create_dir_all(workspace.join(parent))
                    .expect("parent dir creation should succeed");
            }
        }
        fs::write(workspace.join(filename), content).expect("file creation should succeed");
    }
    workspace
}

fn shadow_path(workspace: &PathBuf) -> PathBuf {
    let mut p = workspace.clone();
    let name = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    p.set_file_name(format!("{}.shadow", name));
    p
}

fn journal_path(workspace: &PathBuf) -> PathBuf {
    let mut p = workspace.clone();
    let name = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    p.set_file_name(format!("{}.swap-journal", name));
    p
}

fn backup_path(workspace: &PathBuf) -> PathBuf {
    let mut p = workspace.clone();
    let name = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    p.set_file_name(format!("{}.backup", name));
    p
}

#[test]
fn empty_workspace_stage_and_commit_succeeds() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");

    let swap = AtomicSwap::new(&workspace);
    swap.stage()
        .expect("stage on empty workspace should succeed");
    swap.commit()
        .expect("commit on empty workspace should succeed");

    assert!(workspace.exists());
    assert!(!shadow_path(&workspace).exists());
    assert!(!journal_path(&workspace).exists());
}

#[test]
fn workspace_with_empty_subdirectories_preserves_structure() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(workspace.join("empty_dir/sub"))
        .expect("empty dirs creation should succeed");
    fs::create_dir_all(workspace.join("non_empty")).expect("non-empty dir creation should succeed");
    fs::write(workspace.join("non_empty/file.txt"), "data").expect("file write should succeed");

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");
    swap.commit().expect("commit should succeed");

    assert!(workspace.join("empty_dir/sub").is_dir());
    assert!(workspace.join("non_empty/file.txt").exists());
}

#[test]
fn unicode_filenames_preserved_after_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(
        &temp_dir,
        "ws",
        &[
            ("日本語.txt", "japanese content"),
            ("emoji_🎉.txt", "emoji content"),
            ("café.txt", "french content"),
            ("spaces in name.txt", "spaces content"),
        ],
    );

    atomic_swap(&workspace).expect("atomic_swap with unicode names should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("日本語.txt")).expect("read should succeed"),
        "japanese content"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("emoji_🎉.txt")).expect("read should succeed"),
        "emoji content"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("café.txt")).expect("read should succeed"),
        "french content"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("spaces in name.txt")).expect("read should succeed"),
        "spaces content"
    );
}

#[test]
fn deep_nesting_preserved_after_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    let mut deep_path = workspace.clone();
    for i in 0..20 {
        deep_path = deep_path.join(format!("level_{}", i));
    }
    fs::create_dir_all(&deep_path).expect("deep dirs creation should succeed");
    fs::write(deep_path.join("bottom.txt"), "deepest").expect("file write should succeed");

    atomic_swap(&workspace).expect("atomic_swap with deep nesting should succeed");

    let mut verify_path = workspace.clone();
    for i in 0..20 {
        verify_path = verify_path.join(format!("level_{}", i));
    }
    assert_eq!(
        fs::read_to_string(verify_path.join("bottom.txt")).expect("read should succeed"),
        "deepest"
    );
}

#[test]
fn journal_whitespace_trimmed_correctly() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "  staging  ").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(
        status,
        SwapStatus::Incomplete(SwapPhase::Staging),
        "whitespace should be trimmed"
    );
}

#[test]
fn journal_case_sensitive_rejects_uppercase() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    for bad_content in &["Staging", "STAGED", "Swapping", "COMPLETE"] {
        let journal = journal_path(&workspace);
        fs::write(&journal, *bad_content).expect("journal write should succeed");

        let swap = AtomicSwap::new(&workspace);
        let result = swap.check_status();
        assert!(
            matches!(result, Err(SwapError::InvalidJournal(_))),
            "check_status should reject case-variant journal: {}",
            bad_content
        );
    }
}

#[test]
fn check_status_rejects_invalid_journal_content() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "corrupt_data_12345").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let result = swap.check_status();
    assert!(
        matches!(result, Err(SwapError::InvalidJournal(_))),
        "check_status should return InvalidJournal for corrupt content"
    );
}

#[test]
fn check_status_handles_empty_journal_file() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let result = swap.check_status();
    assert!(
        matches!(result, Err(SwapError::InvalidJournal(_))),
        "check_status should return InvalidJournal for empty journal"
    );
}

#[test]
fn recover_after_complete_returns_already_complete() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "complete").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let outcome = swap.recover().expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::AlreadyComplete);
    assert!(
        !journal.exists(),
        "journal should be cleaned up after AlreadyComplete recovery"
    );
}

#[test]
fn recover_after_complete_via_full_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "original")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");
    swap.commit().expect("commit should succeed");

    let outcome = recover_swap(&workspace).expect("recover_swap should succeed");
    assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
}

#[test]
fn multiple_consecutive_swaps_succeed() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("counter.txt", "0")]);

    for i in 1..=5 {
        fs::write(workspace.join("counter.txt"), format!("{}", i)).expect("write should succeed");
        atomic_swap(&workspace).unwrap_or_else(|e| panic!("swap {} should succeed: {}", i, e));
    }

    assert_eq!(
        fs::read_to_string(workspace.join("counter.txt")).expect("read should succeed"),
        "5"
    );
}

#[test]
fn stage_then_recover_then_stage_again_succeeds() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "original")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("first stage should succeed");

    swap.recover().expect("recover should succeed");

    let swap2 = AtomicSwap::new(&workspace);
    swap2.stage().expect("stage after recovery should succeed");
    swap2
        .commit()
        .expect("commit after recovery should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("data.txt")).expect("read should succeed"),
        "original"
    );
}

#[test]
fn recover_swapping_when_both_backup_and_workspace_exist() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("original.txt", "original")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let backup = backup_path(&workspace);
    let shadow = shadow_path(&workspace);
    let journal = journal_path(&workspace);

    fs::rename(&workspace, &backup).expect("rename to backup should succeed");
    fs::write(&journal, "swapping").expect("journal write should succeed");

    let swap2 = AtomicSwap::new(&workspace);
    swap2.recover().expect("recover should succeed");

    assert!(workspace.exists(), "workspace should be restored");
    assert!(!shadow.exists(), "shadow should be cleaned up");
    assert!(!backup.exists(), "backup should be cleaned up");
    assert!(!journal.exists(), "journal should be cleaned up");

    assert_eq!(
        fs::read_to_string(workspace.join("original.txt")).expect("read should succeed"),
        "original"
    );
}

#[test]
fn swap_phase_initial_not_reported_by_check_status() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(
        status,
        SwapStatus::NoPriorSwap,
        "no journal means NoPriorSwap, not Initial"
    );
}

#[test]
fn check_status_reports_complete_from_journal() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "complete").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::Complete);
}

#[test]
fn commit_after_stage_without_interference_succeeds() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::new(&workspace);
    let phase = swap.stage().expect("stage should succeed");
    assert_eq!(phase, SwapPhase::Staged);

    swap.commit().expect("commit should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("data.txt")).expect("read should succeed"),
        "content"
    );
    assert!(!shadow_path(&workspace).exists());
    assert!(!journal_path(&workspace).exists());
    assert!(!backup_path(&workspace).exists());
}

#[test]
fn atomic_swap_rejects_when_journal_has_invalid_content() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "gibberish_xyz").expect("journal write should succeed");

    let result = atomic_swap(&workspace);
    assert!(
        matches!(result, Err(SwapError::InvalidJournal(_))),
        "atomic_swap should fail with InvalidJournal for corrupt journal"
    );
}

#[test]
fn recover_from_initial_phase_returns_nothing() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::NoPriorSwap);

    let outcome = swap.recover().expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
}

#[test]
fn swap_preserves_file_permissions() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace =
        create_workspace_with_files(&temp_dir, "ws", &[("script.sh", "#!/bin/sh\necho hi")]);

    let original_mode = fs::metadata(workspace.join("script.sh"))
        .expect("metadata should succeed")
        .permissions()
        .mode();

    atomic_swap(&workspace).expect("atomic_swap should succeed");

    let new_mode = fs::metadata(workspace.join("script.sh"))
        .expect("metadata should succeed")
        .permissions()
        .mode();
    assert_eq!(
        original_mode, new_mode,
        "file permissions should be preserved"
    );
}

#[test]
fn with_shadow_suffix_still_uses_default_journal() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::with_shadow_suffix(&workspace, ".my-shadow");
    swap.stage().expect("stage should succeed");

    assert!(
        journal_path(&workspace).exists(),
        "default journal path should be used"
    );
    assert!(
        temp_dir.path().join("ws.my-shadow").exists(),
        "custom shadow should exist"
    );
    assert!(
        !temp_dir.path().join("ws.shadow").exists(),
        "default shadow should not exist"
    );

    swap.commit().expect("commit should succeed");
}

#[test]
fn check_status_on_nonexistent_workspace_with_journal() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("dir creation should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "staged").expect("journal write should succeed");

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::Incomplete(SwapPhase::Staged));
}

#[test]
fn large_number_of_files_stress_test() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");

    for i in 0..200 {
        let dir_name = format!("dir_{}", i % 10);
        fs::create_dir_all(workspace.join(&dir_name)).expect("dir creation should succeed");
        fs::write(
            workspace.join(format!("{}/file_{}.txt", dir_name, i)),
            format!("content_{}", i),
        )
        .expect("file write should succeed");
    }

    atomic_swap(&workspace).expect("atomic_swap with many files should succeed");

    for i in 0..200 {
        let path = workspace.join(format!("dir_{}/file_{}.txt", i % 10, i));
        assert_eq!(
            fs::read_to_string(&path).unwrap_or_else(|e| panic!(
                "read {} failed: {}",
                path.display(),
                e
            )),
            format!("content_{}", i)
        );
    }
}

#[test]
fn zero_length_file_preserved_after_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(
        &temp_dir,
        "ws",
        &[("empty.txt", ""), ("nonempty.txt", "data")],
    );

    atomic_swap(&workspace).expect("atomic_swap should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("empty.txt")).expect("read should succeed"),
        ""
    );
    assert_eq!(
        fs::read_to_string(workspace.join("nonempty.txt")).expect("read should succeed"),
        "data"
    );
}

#[test]
fn atomic_swap_rejects_staging_phase_recovery_needed() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "staging").expect("journal write should succeed");

    let result = atomic_swap(&workspace);
    assert!(
        matches!(result, Err(SwapError::RecoveryNeeded(SwapPhase::Staging))),
        "atomic_swap should return RecoveryNeeded(Staging)"
    );
}

#[test]
fn atomic_swap_rejects_swapping_phase_recovery_needed() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let journal = journal_path(&workspace);
    fs::write(&journal, "swapping").expect("journal write should succeed");

    let result = atomic_swap(&workspace);
    assert!(
        matches!(result, Err(SwapError::RecoveryNeeded(SwapPhase::Swapping))),
        "atomic_swap should return RecoveryNeeded(Swapping)"
    );
}

#[test]
fn recover_swap_convenience_function_returns_nothing_for_clean_state() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let outcome = recover_swap(&workspace).expect("recover_swap should succeed");
    assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
}

#[test]
fn workspace_accessor_returns_canonical_path() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    let swap = AtomicSwap::new(&workspace);
    assert_eq!(swap.workspace(), workspace);
}

#[test]
fn shadow_dir_returns_expected_path() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    let swap = AtomicSwap::new(&workspace);
    assert_eq!(swap.shadow_dir(), shadow_path(&workspace));
}

#[test]
fn concurrent_shadow_suffix_isolation() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "shared")]);

    let swap_a = AtomicSwap::with_shadow_suffix(&workspace, ".shadow-a");
    let swap_b = AtomicSwap::with_shadow_suffix(&workspace, ".shadow-b");

    swap_a.stage().expect("stage a should succeed");
    swap_b
        .stage()
        .expect("stage b should succeed with different suffix");

    assert!(temp_dir.path().join("ws.shadow-a").exists());
    assert!(temp_dir.path().join("ws.shadow-b").exists());

    swap_a.commit().expect("commit a should succeed");
    swap_b.commit().expect("commit b should succeed");
}
