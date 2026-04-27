use std::path::PathBuf;
use vo_cli::{dispatch, interpret_cli_from, Command};

#[tokio::test]
async fn e2e_init_with_custom_engine_url_creates_correct_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        path,
        "--engine-url",
        "http://prod:9090",
        "--storage-path",
        "/data/storage",
    ])
    .expect("parse");
    dispatch(cli).await.expect("init");

    let config = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(config.contains("http://prod:9090"));
    assert!(config.contains("/data/storage"));
}

#[tokio::test]
async fn e2e_init_then_doctor_with_workflow_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    dispatch(init_cli).await.expect("init");

    std::fs::write(
        dir.path().join(".vo/workflows/app"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write workflow");

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    dispatch(lock_cli).await.expect("lock");

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_doctor_report_after_corrupt_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    dispatch(init_cli).await.expect("init");

    std::fs::write(dir.path().join("vo.lock"), "corrupted-lock-content\n").expect("write");

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[test]
fn command_debug_all_variants() {
    let commands = vec![
        format!(
            "{:?}",
            Command::Purge {
                instance: "i".into()
            }
        ),
        format!(
            "{:?}",
            Command::Check {
                workflow: false,
                path: PathBuf::from("/tmp")
            }
        ),
        format!(
            "{:?}",
            Command::Gc {
                engine_url: "u".into(),
                dry_run: false
            }
        ),
        format!(
            "{:?}",
            Command::Init {
                project_dir: PathBuf::from("."),
                engine_url: "u".into(),
                storage_path: PathBuf::from("s"),
            }
        ),
        format!(
            "{:?}",
            Command::Lock {
                project_dir: PathBuf::from(".")
            }
        ),
        format!(
            "{:?}",
            Command::Doctor {
                project_dir: PathBuf::from(".")
            }
        ),
        format!(
            "{:?}",
            Command::Rebuild {
                project_dir: PathBuf::from("."),
                projection_id: None,
                list_projections: false,
                force: false,
            }
        ),
    ];
    assert_eq!(commands.len(), 7);
    for debug_str in &commands {
        assert!(!debug_str.is_empty());
    }
}
