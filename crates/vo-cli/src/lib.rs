pub mod cli;
pub mod commands;
pub mod dispatch_mod;
pub mod lint_targets;
pub mod parse;

pub use cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
pub use commands::check::{
    run_check, validate_binary_header, BinaryFormat, CheckError, ELF_MAGIC, KNOWN_MAGICS,
    MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE, MACHO_MAGIC_64_LE,
};
pub use commands::gc::{
    delete_version_dir, fetch_pinned_hashes, find_unpinned_directories, run_gc, GcConfig, GcError,
    GcSummary,
};
pub use commands::init::{InitConfig, InitError, run_init, VO_DIR_NAME, WORKFLOWS_DIR_NAME, CONFIG_FILE_NAME};
pub use commands::lock::{LockConfig, LockError, run_lock, LOCK_FILE_NAME};
pub use commands::doctor::{DoctorConfig, DoctorError, DoctorReport, run_doctor};
pub use dispatch_mod::dispatch;
pub use parse::parse_strict_numeric;
