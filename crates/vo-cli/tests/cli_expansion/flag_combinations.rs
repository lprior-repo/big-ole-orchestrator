use std::path::PathBuf;
use vo_cli::{interpret_cli_from, Command};

#[test]
fn gc_flags_both_set() {
    let cli = interpret_cli_from(vec![
        "vo",
        "gc",
        "--engine-url",
        "http://engine:8080",
        "--dry-run",
    ])
    .expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://engine:8080");
            assert!(dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_only_dry_run() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_only_engine_url() {
    let cli =
        interpret_cli_from(vec!["vo", "gc", "--engine-url", "http://custom:9090"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://custom:9090");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_none_defaults() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn init_flags_all_custom() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/custom/dir",
        "--engine-url",
        "http://custom:9999",
        "--storage-path",
        "/custom/storage",
    ])
    .expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/custom/dir"));
            assert_eq!(engine_url, "http://custom:9999");
            assert_eq!(storage_path, PathBuf::from("/custom/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_project_dir_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", "/my/proj"]).expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/proj"));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_engine_url_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--engine-url", "http://remote:8080"])
        .expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://remote:8080");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_storage_path_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--storage-path", "/data/vo"]).expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from("/data/vo"));
        }
        _ => panic!("expected Init"),
    }
}
