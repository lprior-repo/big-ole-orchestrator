use std::path::PathBuf;

use vo_cli::{
    interpret_cli_from, map_error_to_exit_code, CheckError, CliError, DoctorError, GcError,
    InitError, LockError, RebuildError,
};

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
