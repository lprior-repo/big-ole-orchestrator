use std::path::PathBuf;
use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::{map_error_to_exit_code, parse_strict_numeric, CliError};

#[test]
fn parse_strict_numeric_accepts_1() {
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
}

#[test]
fn parse_strict_numeric_accepts_large_number() {
    assert_eq!(parse_strict_numeric("1000000000").unwrap(), 1_000_000_000);
}

#[test]
fn parse_strict_numeric_rejects_hex() {
    assert!(parse_strict_numeric("0x10").is_err());
}

#[test]
fn parse_strict_numeric_rejects_scientific() {
    assert!(parse_strict_numeric("1e10").is_err());
}

#[test]
fn parse_strict_numeric_rejects_whitespace() {
    assert!(parse_strict_numeric(" 42").is_err());
    assert!(parse_strict_numeric("42 ").is_err());
    assert!(parse_strict_numeric("4 2").is_err());
}

#[test]
fn exit_code_clap_help_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::DisplayHelp, "help");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_version_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::DisplayVersion, "v");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_missing_arg_help_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        "h",
    );
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_unknown_arg_is_2() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::UnknownArgument, "x");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 2);
}

#[test]
fn exit_code_invalid_numeric_is_2() {
    assert_eq!(
        map_error_to_exit_code(&CliError::InvalidNumeric("x".into())),
        2
    );
}

#[test]
fn exit_code_check_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Check(CheckError::FileNotFound {
            path: PathBuf::from("/tmp")
        })),
        1
    );
}

#[test]
fn exit_code_gc_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Gc(GcError::VersionsDirNotFound {
            path: PathBuf::from("/v")
        })),
        1
    );
}

#[test]
fn exit_code_init_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Init(InitError::DirNotFound {
            path: PathBuf::from("/d")
        })),
        1
    );
}

#[test]
fn exit_code_lock_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Lock(LockError::NotInitialized {
            path: PathBuf::from("/p")
        })),
        1
    );
}

#[test]
fn exit_code_doctor_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Doctor(DoctorError::NotInitialized {
            path: PathBuf::from("/p")
        })),
        1
    );
}

#[test]
fn exit_code_dispatch_error_is_1() {
    assert_eq!(map_error_to_exit_code(&CliError::Dispatch("err".into())), 1);
}
