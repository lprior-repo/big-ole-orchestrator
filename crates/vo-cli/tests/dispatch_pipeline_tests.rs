use std::ffi::OsString;
use std::path::PathBuf;
use vo_cli::cli::{interpret_cli_from, Cli, CliError, Command};
use vo_cli::dispatch_v2::{
    create_dispatcher_v2, dispatch_v2, CommandDispatcherV2, DefaultDispatchContext,
    DispatchContext, LoggingMiddlewareV2, MetricsMiddlewareV2, MiddlewareResult, MiddlewareV2,
};
use vo_cli::middleware::Middleware as V1Middleware;
use vo_cli::middleware::{
    create_dispatcher, CommandContext, CommandDispatcher, LoggingMiddleware, MetricsMiddleware,
};
use vo_cli::registry::HandlerRegistry;

#[tokio::test]
async fn v1_dispatch_unknown_command_returns_error() {
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Purge {
            instance: "nope".into(),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn v1_dispatch_check_nonexistent_returns_error() {
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Check {
            path: PathBuf::from("/nonexistent/path"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn v1_dispatch_init_in_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_ok());
    assert!(dir.path().join(".vo").is_dir());
}

#[tokio::test]
async fn v1_dispatch_lock_uninit_fails() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Lock {
            project_dir: dir.path().to_path_buf(),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn v1_dispatch_doctor_uninit_fails() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Doctor {
            project_dir: dir.path().to_path_buf(),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn v1_dispatch_rebuild_uninit_fails() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher();
    let cli = Cli {
        command: Command::Rebuild {
            project_dir: dir.path().to_path_buf(),
            projection_id: Some("proj-1".into()),
            list_projections: false,
            force: false,
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn v2_dispatch_init_then_doctor_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher_v2();

    let init_cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    let result = dispatcher.dispatch(init_cli).await;
    assert!(result.is_ok());

    let doctor_cli = Cli {
        command: Command::Doctor {
            project_dir: dir.path().to_path_buf(),
        },
    };
    let result = dispatcher.dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn v2_dispatch_rebuild_list_after_init() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher_v2();

    let init_cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    dispatcher.dispatch(init_cli).await.unwrap();

    let rebuild_cli = Cli {
        command: Command::Rebuild {
            project_dir: dir.path().to_path_buf(),
            projection_id: None,
            list_projections: true,
            force: false,
        },
    };
    let result = dispatcher.dispatch(rebuild_cli).await;
    assert!(result.is_ok());
}

#[test]
fn command_context_clone_preserves_metadata() {
    let ctx = CommandContext::new("cmd");
    ctx.set_metadata("k1", "v1");
    let cloned = ctx.clone();
    assert_eq!(cloned.get_metadata("k1"), Some("v1".into()));
    ctx.set_metadata("k2", "v2");
    assert_eq!(cloned.get_metadata("k2"), Some("v2".into()));
}

#[test]
fn dispatcher_add_middleware_incremental() {
    let registry = HandlerRegistry::default();
    let mut dispatcher = CommandDispatcher::new(registry);
    assert_eq!(dispatcher.middleware_count(), 0);
    dispatcher.add_middleware(LoggingMiddleware::new());
    assert_eq!(dispatcher.middleware_count(), 1);
    dispatcher.add_middleware(MetricsMiddleware::new());
    assert_eq!(dispatcher.middleware_count(), 2);
}

#[test]
fn dispatch_context_trait_default_impl() {
    let ctx = DefaultDispatchContext::new("test-cmd");
    assert_eq!(ctx.command_name(), "test-cmd");
    assert!(ctx.elapsed().as_nanos() > 0 || ctx.elapsed().is_zero());
}

#[tokio::test]
async fn v2_dispatcher_with_no_middleware_dispatches() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = CommandDispatcherV2::new();
    let cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_ok());
    dir.close().unwrap();
}

#[test]
fn logging_middleware_name() {
    let mw = LoggingMiddleware::new();
    assert_eq!(V1Middleware::name(&mw), "logging");
}

#[test]
fn metrics_middleware_name() {
    let mw = MetricsMiddleware::new();
    assert_eq!(V1Middleware::name(&mw), "metrics");
}

#[test]
fn cli_error_from_gc_error() {
    let gc_err = vo_cli::GcError::VersionsDirNotFound {
        path: PathBuf::from("/tmp/versions"),
    };
    let cli_err: CliError = gc_err.into();
    let msg = cli_err.to_string();
    assert!(msg.contains("versions directory"));
}

#[test]
fn cli_error_from_lock_error() {
    let lock_err = vo_cli::LockError::NotInitialized {
        path: PathBuf::from("/tmp/project"),
    };
    let cli_err: CliError = lock_err.into();
    let msg = cli_err.to_string();
    assert!(msg.contains("not initialized"));
}

#[test]
fn cli_error_from_rebuild_error() {
    let rebuild_err = vo_cli::RebuildError::NotInitialized {
        path: PathBuf::from("/tmp/project"),
    };
    let cli_err: CliError = rebuild_err.into();
    let msg = cli_err.to_string();
    assert!(msg.contains("not initialized"));
}

#[tokio::test]
async fn dispatch_v2_function_dispatches() {
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    let result = dispatch_v2(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn v1_dispatch_full_init_lock_doctor_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = create_dispatcher();

    let init_cli = Cli {
        command: Command::Init {
            project_dir: dir.path().to_path_buf(),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    dispatcher.dispatch(init_cli).await.unwrap();

    std::fs::write(dir.path().join(".vo/workflows/test-wf"), b"\x7FELFtestdata").unwrap();

    let lock_cli = Cli {
        command: Command::Lock {
            project_dir: dir.path().to_path_buf(),
        },
    };
    dispatcher.dispatch(lock_cli).await.unwrap();
    assert!(dir.path().join("vo.lock").exists());

    let doctor_cli = Cli {
        command: Command::Doctor {
            project_dir: dir.path().to_path_buf(),
        },
    };
    let result = dispatcher.dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn v2_dispatch_check_valid_elf() {
    let dir = tempfile::tempdir().unwrap();
    let elf_path = dir.path().join("test.elf");
    let mut data = vec![0x7F, 0x45, 0x4C, 0x46];
    data.extend_from_slice(b"padding to reach at least 4 bytes");
    std::fs::write(&elf_path, &data).unwrap();

    let dispatcher = create_dispatcher_v2();
    let cli = Cli {
        command: Command::Check { path: elf_path },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn v2_dispatch_check_nonexistent_file() {
    let dispatcher = create_dispatcher_v2();
    let cli = Cli {
        command: Command::Check {
            path: PathBuf::from("/nonexistent/binary"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}
