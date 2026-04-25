use std::path::PathBuf;

use vo_cli::{Command, HandlerRegistry};

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
