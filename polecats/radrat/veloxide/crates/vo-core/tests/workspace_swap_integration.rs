//! Integration tests for workspace_swap module.
//!
//! These tests exercise the workspace swap functionality with real filesystem
//! operations to verify the atomic swap behavior, crash recovery, and journal
//! handling work correctly across component interactions.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use vo_core::workspace_swap::{
    atomic_swap, recover_swap, AtomicSwap, RecoveryOutcome, SwapPhase, SwapStatus,
};

fn create_workspace_with_files(dir: &TempDir, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let workspace = dir.path().join(name);
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");
    for (filename, content) in files {
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
fn atomic_swap_commits_without_prior_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(
        &temp_dir,
        "ws",
        &[("data.txt", "hello world"), ("config.json", "{}")],
    );

    let result = atomic_swap(&workspace);
    assert!(
        result.is_ok(),
        "atomic_swap should succeed with no prior state"
    );

    assert_eq!(
        fs::read_to_string(workspace.join("data.txt")).expect("read should succeed"),
        "hello world"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("config.json")).expect("read should succeed"),
        "{}"
    );
}

#[test]
fn atomic_swap_rejects_incomplete_prior_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("original.txt", "data")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let result = atomic_swap(&workspace);
    assert!(
        result.is_err(),
        "atomic_swap should fail when prior swap is incomplete"
    );
}

#[test]
fn recover_from_staging_phase_rolls_back() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace =
        create_workspace_with_files(&temp_dir, "ws", &[("original.txt", "original data")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "staging").expect("journal write should succeed");

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::RolledBack);

    assert_eq!(
        fs::read_to_string(workspace.join("original.txt")).expect("read should succeed"),
        "original data"
    );
    assert!(
        !shadow_path(&workspace).exists(),
        "shadow should be cleaned up after rollback"
    );
    assert!(
        !journal.exists(),
        "journal should be cleaned up after rollback"
    );
}

#[test]
fn recover_from_staged_phase_rolls_back() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace =
        create_workspace_with_files(&temp_dir, "ws", &[("original.txt", "original data")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "staged").expect("journal write should succeed");

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::RolledBack);

    assert_eq!(
        fs::read_to_string(workspace.join("original.txt")).expect("read should succeed"),
        "original data"
    );
}

#[test]
fn recover_from_swapping_phase_rolls_back() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace =
        create_workspace_with_files(&temp_dir, "ws", &[("original.txt", "original data")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let backup = backup_path(&workspace);
    let shadow = shadow_path(&workspace);
    let journal = journal_path(&workspace);

    fs::rename(&workspace, &backup).expect("backup rename should succeed");

    fs::write(&journal, "swapping").expect("journal write should succeed");

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::RolledBack);

    assert!(workspace.exists(), "workspace should be restored");
    assert_eq!(
        fs::read_to_string(workspace.join("original.txt")).expect("read should succeed"),
        "original data"
    );
    assert!(!shadow.exists(), "shadow should be cleaned up");
    assert!(!backup.exists(), "backup should be cleaned up");
}

#[test]
fn recover_handles_workspace_missing_during_swapping() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace =
        create_workspace_with_files(&temp_dir, "ws", &[("important.txt", "critical data")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let backup = backup_path(&workspace);
    let shadow = shadow_path(&workspace);
    let journal = journal_path(&workspace);

    fs::rename(&workspace, &backup).expect("backup rename should succeed");
    fs::write(&journal, "swapping").expect("journal write should succeed");

    assert!(
        !workspace.exists(),
        "workspace should not exist after rename to backup"
    );

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::RolledBack);

    assert!(
        workspace.exists(),
        "workspace should be restored from backup"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("important.txt")).expect("read should succeed"),
        "critical data"
    );
    assert!(!shadow.exists(), "shadow should be cleaned up");
    assert!(!backup.exists(), "backup should be cleaned up");
}

#[test]
fn recover_returns_nothing_when_no_journal() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
}

#[test]
fn recover_after_successful_commit_returns_nothing() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");
    swap.commit().expect("commit should succeed");

    let outcome = recover_swap(&workspace).expect("recover should succeed");
    assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
}

#[test]
fn check_status_reports_no_prior_swap_initially() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::NoPriorSwap);
}

#[test]
fn check_status_reports_incomplete_during_staging() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "staging").expect("journal write should succeed");

    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::Incomplete(SwapPhase::Staging));
}

#[test]
fn check_status_reports_incomplete_during_staged() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::Incomplete(SwapPhase::Staged));
}

#[test]
fn check_status_reports_incomplete_during_swapping() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");

    let backup = backup_path(&workspace);
    fs::rename(&workspace, &backup).expect("backup rename should succeed");

    let journal = journal_path(&workspace);
    fs::write(&journal, "swapping").expect("journal write should succeed");

    let status = swap.check_status().expect("check_status should succeed");
    assert_eq!(status, SwapStatus::Incomplete(SwapPhase::Swapping));
}

#[test]
fn stage_fails_if_workspace_does_not_exist() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let missing = temp_dir.path().join("nonexistent");

    let swap = AtomicSwap::new(&missing);
    let result = swap.stage();
    assert!(
        result.is_err(),
        "stage should fail when workspace doesn't exist"
    );
}

#[test]
fn stage_fails_if_workspace_is_a_file() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let file_path = temp_dir.path().join("not_a_dir.txt");
    fs::write(&file_path, "data").expect("file creation should succeed");

    let swap = AtomicSwap::new(&file_path);
    let result = swap.stage();
    assert!(
        result.is_err(),
        "stage should fail when workspace is a file"
    );
}

#[test]
fn stage_fails_if_shadow_already_exists() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("first stage should succeed");

    let swap2 = AtomicSwap::new(&workspace);
    let result = swap2.stage();
    assert!(
        result.is_err(),
        "second stage should fail when shadow exists"
    );
}

#[test]
fn commit_is_idempotent_when_no_prior_swap() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    let result = swap.commit();
    assert!(result.is_ok(), "commit should succeed with no prior swap");
}

#[test]
fn commit_is_idempotent_when_already_complete() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::new(&workspace);
    swap.stage().expect("stage should succeed");
    swap.commit().expect("first commit should succeed");

    let result = swap.commit();
    assert!(result.is_ok(), "second commit should be idempotent");
}

#[test]
fn atomic_swap_preserves_nested_directories() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(workspace.join("a/b/c")).expect("nested dirs creation should succeed");
    fs::write(workspace.join("a/b/c/deep.txt"), "deep content")
        .expect("file creation should succeed");
    fs::write(workspace.join("a/top.txt"), "top level").expect("file creation should succeed");

    atomic_swap(&workspace).expect("atomic_swap should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("a/b/c/deep.txt")).expect("read should succeed"),
        "deep content"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("a/top.txt")).expect("read should succeed"),
        "top level"
    );
}

#[test]
fn atomic_swap_handles_many_files() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");

    for i in 0..100 {
        fs::write(
            workspace.join(format!("file_{}.txt", i)),
            format!("content {}", i),
        )
        .expect("file creation should succeed");
    }

    atomic_swap(&workspace).expect("atomic_swap should succeed");

    for i in 0..100 {
        let content = fs::read_to_string(workspace.join(format!("file_{}.txt", i)))
            .expect("read should succeed");
        assert_eq!(content, format!("content {}", i));
    }
}

#[test]
fn atomic_swap_with_custom_shadow_suffix() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[("data.txt", "content")]);

    let swap = AtomicSwap::with_shadow_suffix(&workspace, ".custom-shadow");
    swap.stage()
        .expect("stage with custom suffix should succeed");

    assert!(
        temp_dir.path().join("ws.custom-shadow").exists(),
        "custom shadow suffix should be used"
    );
    assert!(
        !temp_dir.path().join("ws.shadow").exists(),
        "default shadow suffix should not exist"
    );
}

#[test]
fn workspace_swap_with_large_file() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = temp_dir.path().join("ws");
    fs::create_dir_all(&workspace).expect("workspace creation should succeed");

    let large_content = "x".repeat(1024 * 1024);
    fs::write(workspace.join("large.txt"), &large_content)
        .expect("large file creation should succeed");

    atomic_swap(&workspace).expect("atomic_swap with large file should succeed");

    let read_content =
        fs::read_to_string(workspace.join("large.txt")).expect("read should succeed");
    assert_eq!(read_content.len(), 1024 * 1024);
}

#[test]
fn swap_workspace_method_returns_original_path() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    assert_eq!(swap.workspace(), workspace);
}

#[test]
fn shadow_dir_method_returns_correct_path() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let workspace = create_workspace_with_files(&temp_dir, "ws", &[]);

    let swap = AtomicSwap::new(&workspace);
    let expected_shadow = shadow_path(&workspace);
    assert_eq!(swap.shadow_dir(), expected_shadow);
}

#[test]
fn swap_phase_variants_exist() {
    let _ = SwapPhase::Initial;
    let _ = SwapPhase::Staging;
    let _ = SwapPhase::Staged;
    let _ = SwapPhase::Swapping;
    let _ = SwapPhase::Complete;
}

#[test]
fn swap_status_variants_exist() {
    let _ = SwapStatus::NoPriorSwap;
    let _ = SwapStatus::Complete;
    let _ = SwapStatus::Incomplete(SwapPhase::Staging);
}

#[test]
fn recovery_outcome_variants_exist() {
    let _ = RecoveryOutcome::NothingToRecover;
    let _ = RecoveryOutcome::AlreadyComplete;
    let _ = RecoveryOutcome::RolledBack;
}
