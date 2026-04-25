use std::path::PathBuf;
use vo_cli::commands::doctor_checks::{CheckCategory, Severity};
use vo_cli::commands::doctor::DoctorConfig;
use vo_cli::commands::init::{InitConfig, InitError};

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    std::fs::create_dir_all(vo_dir.join("storage")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

#[test]
fn init_creates_valid_toml_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://engine:4000".into(),
        storage_path: PathBuf::from("/data/vo"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");

    let content = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    let table: toml::Table = content.parse().expect("valid TOML");
    assert!(table.contains_key("engine"));
    assert!(table.contains_key("storage"));
    assert_eq!(
        table["engine"]["url"].as_str().unwrap(),
        "http://engine:4000"
    );
    assert_eq!(table["storage"]["path"].as_str().unwrap(), "/data/vo");
}

#[test]
fn init_config_content_matches_expected_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");

    let content = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(content.starts_with("[engine]"));
    assert!(content.contains("url = \"http://localhost:3000\""));
    assert!(content.contains("[storage]"));
    assert!(content.contains("path = \".vo/storage\""));
}

#[test]
fn init_idempotent_same_config_returns_same_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let r1 = vo_cli::commands::init::run_init(&config).expect("first");
    let r2 = vo_cli::commands::init::run_init(&config).expect("second");
    assert_eq!(r1, r2);
}

#[test]
fn init_rejects_different_config_after_init() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config1 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let config2 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://different:9999".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config1).expect("first");
    let result = vo_cli::commands::init::run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn doctor_validates_config_toml_parseable() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_project(dir.path());
    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    let config_checks: Vec<_> = report
        .categories
        .iter()
        .find(|c| c.category == CheckCategory::ConfigValidation)
        .map(|c| c.checks.clone())
        .unwrap_or_default();

    assert!(config_checks
        .iter()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Info));
}

#[test]
fn doctor_detects_empty_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(dir.path().join("config.toml"), "").expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.errors().any(|c| c.check == "config-empty"));
}

#[test]
fn doctor_detects_invalid_toml() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(dir.path().join("config.toml"), "}}}{invalid{{").expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report
        .errors()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Error));
}

#[test]
fn doctor_detects_missing_engine_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[storage]\npath = \".vo/storage\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-engine"));
}

#[test]
fn doctor_detects_missing_storage_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-storage"));
}

#[test]
fn doctor_detects_empty_engine_url() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-engine-url"));
}

#[test]
fn doctor_detects_empty_storage_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \"\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-storage-path"));
}
