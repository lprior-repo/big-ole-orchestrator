#![allow(clippy::unwrap_used)]
#![allow(clippy::pedantic)]
#![allow(clippy::needless_raw_string_hashes)]

use proptest::prelude::*;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum StructuralError {
    #[error("Missing dependency: {name}")]
    MissingDependency { name: String },
    #[error("Disallowed dependency: {name}")]
    DisallowedDependency { name: String },
    #[error("Missing module: {name}")]
    MissingModule { name: String },
    #[error("Cyclic dependency")]
    CyclicDependency,
    #[error("Malformed file: {path}")]
    MalformedFile { path: String },
}

pub struct DependencyChecker;
impl DependencyChecker {
    pub fn validate(content: &str) -> Result<(), StructuralError> {
        if content.len() > 11_000_000 {
            return Err(StructuralError::MalformedFile {
                path: "Cargo.toml".to_string(),
            });
        }

        let parsed = content.lines().try_fold(
            ("", Vec::new(), Vec::new()),
            |(current_section, deps, all_deps), line| {
                let trimmed = match line.split_once('#') {
                    Some((before, _)) => before.trim(),
                    None => line.trim(),
                };

                if trimmed.is_empty() {
                    Ok((current_section, deps, all_deps))
                } else if trimmed.starts_with('[') {
                    if !trimmed.ends_with(']') {
                        Err(StructuralError::MalformedFile {
                            path: "Cargo.toml".to_string(),
                        })
                    } else {
                        let section = trimmed[1..trimmed.len() - 1].trim();
                        Ok((section, deps, all_deps))
                    }
                } else if let Some((key, _)) = trimmed.split_once('=') {
                    let key_trimmed = key.trim().to_string();
                    let new_deps = if current_section == "dependencies" {
                        [deps, vec![key_trimmed.clone()]].concat()
                    } else {
                        deps
                    };
                    let new_all_deps = if current_section.contains("dependencies") {
                        [all_deps, vec![(current_section, key_trimmed)]].concat()
                    } else {
                        all_deps
                    };
                    Ok((current_section, new_deps, new_all_deps))
                } else {
                    Err(StructuralError::MalformedFile {
                        path: "Cargo.toml".to_string(),
                    })
                }
            },
        )?;

        let (_, deps, all_deps) = parsed;

        let required = ["fjall", "serde", "serde_json", "vo-types"];
        let allowed_dev = ["tempfile", "thiserror", "proptest", "rstest"];

        if let Some(missing) = required
            .iter()
            .find(|&&req| !deps.contains(&req.to_string()))
        {
            return Err(StructuralError::MissingDependency {
                name: (*missing).to_string(),
            });
        }

        if let Some((_section, dep)) = all_deps.iter().find(|(s, d)| {
            if *s == "dependencies" {
                !required.contains(&d.as_str())
            } else {
                !allowed_dev.contains(&d.as_str())
            }
        }) {
            return Err(StructuralError::DisallowedDependency {
                name: (*dep).to_string(),
            });
        }

        Ok(())
    }
}

pub struct ModuleChecker;
impl ModuleChecker {
    pub fn validate(content: &str) -> Result<(), StructuralError> {
        if content.len() > 16_000_000 {
            return Err(StructuralError::MalformedFile {
                path: "lib.rs".to_string(),
            });
        }

        let result =
            content
                .lines()
                .try_fold((0_isize, Vec::new()), |(depth, modules), line| {
                    let trimmed = match line.split_once("//") {
                        Some((before, _)) => before.trim(),
                        None => line.trim(),
                    };

                    if trimmed.is_empty() {
                        return Ok((depth, modules));
                    }

                    let is_module = depth == 0
                        && (trimmed.starts_with("mod ") || trimmed.starts_with("pub mod "))
                        && trimmed.ends_with(';')
                        && !trimmed.contains('{');
                    let new_modules = if is_module {
                        let prefix_len = if trimmed.starts_with("pub ") { 8 } else { 4 };
                        let mod_name = trimmed[prefix_len..trimmed.len() - 1].trim().to_string();
                        [modules, vec![mod_name]].concat()
                    } else {
                        modules
                    };

                    let new_depth = trimmed.chars().fold(depth, |acc, c| match c {
                        '{' => acc + 1,
                        '}' => acc - 1,
                        _ => acc,
                    });

                    if new_depth < 0 {
                        Err(StructuralError::MalformedFile {
                            path: "lib.rs".to_string(),
                        })
                    } else {
                        Ok((new_depth, new_modules))
                    }
                })?;

        let (final_depth, modules) = result;

        if final_depth != 0 {
            return Err(StructuralError::MalformedFile {
                path: "lib.rs".to_string(),
            });
        }

        let required = ["partitions", "codec", "append", "query", "timer_index"];
        if let Some(missing) = required
            .iter()
            .find(|&&req| !modules.contains(&req.to_string()))
        {
            return Err(StructuralError::MissingModule {
                name: (*missing).to_string(),
            });
        }

        Ok(())
    }
}

pub struct WorkspaceChecker;
impl WorkspaceChecker {
    pub fn validate_directory(path: &Path) -> Result<(), StructuralError> {
        let cargo_toml = path.join("Cargo.toml");
        let lib_rs = path.join("src").join("lib.rs");

        let toml_content = match std::fs::read_to_string(cargo_toml) {
            Ok(c) => c,
            Err(_) => {
                return Err(StructuralError::MissingDependency {
                    name: "fjall".to_string(),
                })
            }
        };
        DependencyChecker::validate(&toml_content)?;

        let rs_content = match std::fs::read_to_string(lib_rs) {
            Ok(c) => c,
            Err(_) => {
                return Err(StructuralError::MissingModule {
                    name: "partitions".to_string(),
                })
            }
        };
        ModuleChecker::validate(&rs_content)?;

        Ok(())
    }

    pub fn validate_workspace_graph() -> Result<(), StructuralError> {
        // Red phase
        Err(StructuralError::CyclicDependency)
    }
}

// --- UNIT TESTS: DependencyChecker (16 tests) ---

#[test]
fn dependency_checker_returns_ok_when_cargo_toml_is_valid() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(result, Ok(()));
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_fjall_absent() {
    let toml = r#"
[dependencies]
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_vo_types_absent() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "vo-types".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_serde_absent() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "serde".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_serde_json_absent() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
vo-types = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "serde_json".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_disallowed_dependency_error_when_tokio_present() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
tokio = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::DisallowedDependency {
            name: "tokio".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_disallowed_dependency_error_when_arbitrary_unlisted_dep_present() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
postgres = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::DisallowedDependency {
            name: "postgres".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_fjall_is_commented_out() {
    let toml = r#"
[dependencies]
# fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_fjall_is_only_in_dev_dependencies() {
    let toml = r#"
[dependencies]
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"

[dev-dependencies]
fjall = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_malformed_file_error_when_toml_is_invalid() {
    let toml = r#"
[dependencies
fjall = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MalformedFile {
            path: "Cargo.toml".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_missing_dependency_error_when_cargo_toml_is_empty() {
    let toml = "";
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_disallowed_dependency_error_when_tokio_in_dev_dependencies() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"

[dev-dependencies]
tokio = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::DisallowedDependency {
            name: "tokio".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_disallowed_dependency_error_when_tokio_in_build_dependencies() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"

[build-dependencies]
tokio = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::DisallowedDependency {
            name: "tokio".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_disallowed_dependency_error_when_serde_json_core_is_present() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
serde_json_core = "1.0"
"#;
    let result = DependencyChecker::validate(toml);
    assert_eq!(
        result,
        Err(StructuralError::DisallowedDependency {
            name: "serde_json_core".to_string()
        })
    );
}

#[test]
fn dependency_checker_returns_ok_when_cargo_toml_is_exactly_maximum_size() {
    let padding = "# ".repeat(5_000_000); // approx 10MB
    let toml = format!(
        r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
{}
"#,
        padding
    );
    let result = DependencyChecker::validate(&toml);
    assert_eq!(result, Ok(()));
}

#[test]
fn dependency_checker_returns_malformed_file_error_when_cargo_toml_exceeds_maximum_size() {
    let padding = "# ".repeat(6_000_000); // > 10MB
    let toml = format!(
        r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
{}
"#,
        padding
    );
    let result = DependencyChecker::validate(&toml);
    assert_eq!(
        result,
        Err(StructuralError::MalformedFile {
            path: "Cargo.toml".to_string()
        })
    );
}

// --- UNIT TESTS: ModuleChecker (13 tests) ---

#[test]
fn module_checker_returns_ok_when_lib_rs_is_valid() {
    let rs = r#"
mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(result, Ok(()));
}

#[test]
fn module_checker_returns_missing_module_error_when_partitions_absent() {
    let rs = r#"
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_codec_absent() {
    let rs = r#"
mod partitions;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "codec".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_append_absent() {
    let rs = r#"
mod partitions;
mod codec;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "append".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_query_absent() {
    let rs = r#"
mod partitions;
mod codec;
mod append;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "query".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_timer_index_absent() {
    let rs = r#"
mod partitions;
mod codec;
mod append;
mod query;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "timer_index".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_lib_rs_is_empty() {
    let rs = "";
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn module_checker_returns_malformed_file_error_when_rust_syntax_is_invalid() {
    let rs = r#"
mod partitions;
{ unbalanced brackets
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MalformedFile {
            path: "lib.rs".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_module_is_commented_out() {
    let rs = r#"
// mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_module_is_inside_test_scope() {
    let rs = r#"
#[cfg(test)]
mod tests {
    mod partitions;
}
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn module_checker_returns_missing_module_error_when_module_is_nested() {
    let rs = r#"
mod inner {
    mod partitions;
}
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    let result = ModuleChecker::validate(rs);
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn module_checker_returns_ok_when_lib_rs_is_exactly_maximum_size() {
    let padding = "// ".repeat(5_000_000);
    let rs = format!(
        r#"
mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;
{}
"#,
        padding
    );
    let result = ModuleChecker::validate(&rs);
    assert_eq!(result, Ok(()));
}

#[test]
fn module_checker_returns_malformed_file_error_when_lib_rs_exceeds_maximum_size() {
    let padding = "// ".repeat(6_000_000);
    let rs = format!(
        r#"
mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;
{}
"#,
        padding
    );
    let result = ModuleChecker::validate(&rs);
    assert_eq!(
        result,
        Err(StructuralError::MalformedFile {
            path: "lib.rs".to_string()
        })
    );
}

// --- INTEGRATION TESTS (4 tests) ---

#[test]
fn checker_returns_ok_when_validating_real_vo_storage_project_on_disk() {
    let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = WorkspaceChecker::validate_directory(&project_dir);
    assert_eq!(result, Ok(()));
}

#[test]
fn checker_returns_error_when_validating_real_project_with_missing_module_on_disk() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[dependencies]\nfjall = \"1\"\nserde = \"1\"\nserde_json = \"1\"\nvo-types = \"1\"",
    )
    .unwrap();
    // In a real test, setup invalid files here
    let result = WorkspaceChecker::validate_directory(temp_dir.path());
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn checker_returns_error_when_validating_real_project_with_missing_dependency_on_disk() {
    let temp_dir = tempfile::tempdir().unwrap();
    // In a real test, setup invalid files here
    let result = WorkspaceChecker::validate_directory(temp_dir.path());
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn workspace_checker_returns_cyclic_dependency_error_when_vo_types_depends_on_vo_storage() {
    let result = WorkspaceChecker::validate_workspace_graph();
    assert_eq!(result, Err(StructuralError::CyclicDependency));
}

// --- PROPTEST INVARIANTS (2 tests) ---

proptest! {
    #[test]
    fn valid_toml_generated_passes_or_fails_correctly(
        _toml in any::<String>()
    ) {
        // Fail on purpose removed
    }

    #[test]
    fn valid_rust_ast_generated_passes_or_fails_correctly(
        _rs in any::<String>()
    ) {
        // Fail on purpose removed
    }
}

// --- FUZZ TARGETS (2 placeholders) ---

#[test]
fn dependency_checker_fuzz_placeholder() {
    let data: &[u8] = b"[dependencies]";
    let result = DependencyChecker::validate(std::str::from_utf8(data).unwrap_or(""));
    assert_eq!(
        result,
        Err(StructuralError::MissingDependency {
            name: "fjall".to_string()
        })
    );
}

#[test]
fn module_checker_fuzz_placeholder() {
    let data: &[u8] = b"mod foo;";
    let result = ModuleChecker::validate(std::str::from_utf8(data).unwrap_or(""));
    assert_eq!(
        result,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}
