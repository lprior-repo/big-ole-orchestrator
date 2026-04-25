#![allow(clippy::redundant_pattern_matching)]
use std::path::{Path, PathBuf};

use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::commands::rebuild::RebuildError;
use vo_cli::{
    CliError, Command, CommandDispatcherV2, DefaultDispatchContext, DispatchContext,
    HandlerRegistry, LoggingMiddlewareV2, MetricsMiddlewareV2, MiddlewareResult, MiddlewareV2,
    interpret_cli_from, map_error_to_exit_code,
};

struct AbortMiddleware;

impl MiddlewareV2 for AbortMiddleware {
    fn name(&self) -> &'static str {
        "abort"
    }

    fn before(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = MiddlewareResult> + Send + '_>> {
        Box::pin(async {
            MiddlewareResult::Abort(CliError::Dispatch("aborted by middleware".into()))
        })
    }

    fn after(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }

    fn on_error(
        &self,
        _ctx: &dyn DispatchContext,
        _error: &CliError,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

#[tokio::test]
async fn dispatch_v2_abort_middleware_returns_error() {
    let dispatcher = CommandDispatcherV2::new().with_middleware(AbortMiddleware);
    let cli = vo_cli::Cli {
        command: Command::Check {
            workflow: false,
            path: PathBuf::from("/tmp"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
    match result {
        Err(CliError::Dispatch(msg)) => assert!(msg.contains("aborted by middleware")),
        _ => panic!("expected Dispatch error"),
    }
}

#[test]
fn registry_lookups_all_commands() {
    let registry = HandlerRegistry::default();

    let cmds = vec![
        (
            Command::Purge {
                instance: "x".into(),
            },
            "purge",
        ),
        (
            Command::Check {
                workflow: false,
                path: PathBuf::from("/tmp"),
            },
            "check",
        ),
        (
            Command::Gc {
                engine_url: "http://x".into(),
                dry_run: false,
            },
            "gc",
        ),
        (
            Command::Init {
                project_dir: PathBuf::from("."),
                engine_url: "http://x".into(),
                storage_path: PathBuf::from(".vo/storage"),
            },
            "init",
        ),
        (
            Command::Lock {
                project_dir: PathBuf::from("."),
            },
            "lock",
        ),
        (
            Command::Doctor {
                project_dir: PathBuf::from("."),
            },
            "doctor",
        ),
        (
            Command::Rebuild {
                project_dir: PathBuf::from("."),
                projection_id: None,
                list_projections: false,
                force: false,
            },
            "rebuild",
        ),
    ];

    for (cmd, expected_name) in cmds {
        let cli = vo_cli::Cli { command: cmd };
        let handler = registry.get(&cli).unwrap_or_else(|| {
            panic!("handler not found for {expected_name}");
        });
        assert_eq!(handler.name(), expected_name);
    }
}

#[test]
fn registry_names_sorted() {
    let registry = HandlerRegistry::default();
    let mut names = registry.names();
    names.sort();
    assert_eq!(
        names,
        vec!["check", "doctor", "gc", "init", "lock", "purge", "rebuild", "status"]
    );
}

#[test]
fn exit_code_for_all_clap_help_variants() {
    let kinds = [
        clap::error::ErrorKind::DisplayHelp,
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        clap::error::ErrorKind::DisplayVersion,
    ];
    for kind in kinds {
        let mut cmd = clap::Command::new("vo");
        let err = cmd.error(kind, "test");
        assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
    }
}

#[test]
fn exit_code_for_unknown_argument() {
    let result = interpret_cli_from(vec!["vo", "--unknown-flag"]);
    assert!(result.is_err());
    let err = CliError::Clap(result.unwrap_err());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn exit_code_for_invalid_numeric() {
    let err = CliError::InvalidNumeric("test".into());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn exit_code_for_each_command_error_type() {
    let errors: Vec<CliError> = vec![
        CliError::Dispatch("test".into()),
        CliError::Check(CheckError::FileNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Gc(GcError::VersionsDirNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Init(InitError::DirNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Lock(LockError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
        CliError::Doctor(DoctorError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
        CliError::Rebuild(RebuildError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
    ];
    for err in errors {
        assert_eq!(map_error_to_exit_code(&err), 1);
    }
}

#[tokio::test]
async fn logging_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = LoggingMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}

#[tokio::test]
async fn metrics_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = MetricsMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}
