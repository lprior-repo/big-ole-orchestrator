pub mod cli;
pub mod commands;
pub mod dispatch_mod;
pub mod lint_targets;
pub mod middleware;
pub mod parse;

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
pub use commands::init::{
    run_init, InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
pub use commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
pub use commands::verify::{run_verify, VerifyConfig, VerifyError};
pub use dispatch_mod::dispatch;
pub use middleware::{create_dispatcher, CommandContext, CommandDispatcher, Middleware};
pub use parse::parse_strict_numeric;
