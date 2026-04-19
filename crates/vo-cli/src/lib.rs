//! Command-line interface for the Veloxide workflow engine.
//!
//! This crate provides the CLI tool `vel` for interacting with the Veloxide engine.
//!
//! # Commands
//!
//! - `vel init` - Initialize a new workflow workspace
//! - `vel check` - Validate workflow definitions and binary formats
//! - `vel doctor` - Run system health checks
//! - `vel gc` - Garbage collect unused workflow versions
//! - `vel lock` - Manage distributed locks
//!
//! # Architecture
//!
//! The CLI uses a middleware-based dispatcher for command handling with
//! support for middleware chaining and error mapping.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_imports,
    unused,
    clippy::redundant_locals,
    clippy::collapsible_if
)]

pub mod cli;
pub mod commands;
pub mod dispatch_mod;
pub mod dispatch_v2;
pub mod handler;
pub mod lint_targets;
pub mod middleware;
pub mod parse;
pub mod registry;
pub mod utils;

pub use cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
pub use commands::check::{
    run_check, validate_binary_header, BinaryFormat, CheckError, ELF_MAGIC, KNOWN_MAGICS,
    MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE, MACHO_MAGIC_64_LE,
};
pub use commands::doctor::{run_doctor, DoctorConfig, DoctorError};
pub use commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
pub use commands::gc::{
    delete_version_dir, fetch_pinned_hashes, find_unpinned_directories, run_gc, GcConfig, GcError,
    GcSummary,
};
pub use commands::history::{
    get_history, load_history, redo_command, save_history, undo_command, HistoryConfig,
    HistoryError, HistoryOutput, RedoResult, UndoResult,
};
pub use commands::init::{
    run_init, InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
pub use commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
pub use commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
pub use commands::status::{run_status, StatusConfig, StatusError, WorkflowStatusResponse};
pub use dispatch_mod::dispatch;
pub use dispatch_v2::{
    create_dispatcher_v2, dispatch_v2, CommandDispatcherV2, DefaultDispatchContext,
    DispatchContext, LoggingMiddlewareV2, MetricsMiddlewareV2, MiddlewareResult, MiddlewareV2,
};
pub use handler::CommandHandler;
pub use middleware::{create_dispatcher, CommandContext, CommandDispatcher, Middleware};
pub use parse::parse_strict_numeric;
pub use registry::HandlerRegistry;
