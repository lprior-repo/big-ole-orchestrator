use std::path::PathBuf;
use vo_cli::{interpret_cli_from, map_error_to_exit_code};

#[tokio::test]
async fn e2e_init_creates_vo_dir_and_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());

    assert!(dir.path().join(".vo").is_dir());
    assert!(dir.path().join(".vo/workflows").is_dir());
    assert!(dir.path().join("config.toml").exists());

    let config = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(config.contains("[engine]"));
    assert!(config.contains("[storage]"));
}

#[tokio::test]
async fn e2e_init_then_lock_empty_workflows_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(init_cli).await.expect("init");

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(lock_cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_init_lock_doctor_full_pipeline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(init_cli).await.expect("init");

    std::fs::write(
        dir.path().join(".vo/workflows/test-wf"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write workflow binary");

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(lock_cli).await.expect("lock");

    assert!(dir.path().join("vo.lock").exists());

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_valid_elf_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_path = dir.path().join("test.bin");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00]).expect("write");

    let cli =
        interpret_cli_from(vec!["vo", "check", bin_path.to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_nonexistent_file_returns_error() {
    let cli =
        interpret_cli_from(vec!["vo", "check", "/tmp/no-such-file-vo-test-xyz"]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
    let code = map_error_to_exit_code(result.as_ref().expect_err("err"));
    assert_eq!(code, 1);
}

#[tokio::test]
async fn e2e_gc_dry_run_succeeds() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_init_rejects_nonexistent_dir() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/tmp/nonexistent-e2e-test-xyz-123",
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_lock_without_init_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = interpret_cli_from(vec![
        "vo",
        "lock",
        "--project-dir",
        dir.path().to_str().expect("path"),
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_doctor_without_init_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = interpret_cli_from(vec![
        "vo",
        "doctor",
        "--project-dir",
        dir.path().to_str().expect("path"),
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_check_directory_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli =
        interpret_cli_from(vec!["vo", "check", dir.path().to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_check_symlink_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let real = dir.path().join("real.bin");
    std::fs::write(&real, [0x7F, 0x45, 0x4C, 0x46]).expect("write");
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let cli = interpret_cli_from(vec!["vo", "check", link.to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}
