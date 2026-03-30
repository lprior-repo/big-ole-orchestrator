#[derive(Debug, PartialEq, thiserror::Error)]
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

// ADVERSARIAL TESTS

#[test]
fn dependency_quotes() {
    let toml = r#"
[dependencies]
"fjall" = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    // Should pass, but it fails because it expects exactly "fjall" without quotes
    let res = DependencyChecker::validate(toml);
    assert_ne!(res, Ok(()));
}

#[test]
fn dependency_tables() {
    let toml = r#"
[dependencies.tokio]
version = "1.0"

[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
"#;
    // Tokio is added, it should throw DisallowedDependency { name: "tokio" }
    // But it will throw DisallowedDependency { name: "version" }
    let res = DependencyChecker::validate(toml);
    assert_eq!(
        res,
        Err(StructuralError::DisallowedDependency {
            name: "version".to_string()
        })
    );
}

#[test]
fn disallowed_inline_table() {
    let toml = r#"
[dependencies]
fjall = "1.0"
serde = "1.0"
serde_json = "1.0"
vo-types = "1.0"
axum = { version = "1.0" }
"#;
    // Does it catch axum? Yes, because key is `axum`.
    let res = DependencyChecker::validate(toml);
    assert_eq!(
        res,
        Err(StructuralError::DisallowedDependency {
            name: "axum".to_string()
        })
    );
}

#[test]
fn missing_module_pub_crate() {
    let rs = r#"
pub(crate) mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;
"#;
    // Should be OK conceptually but fails due to regex
    let res = ModuleChecker::validate(rs);
    assert_eq!(
        res,
        Err(StructuralError::MissingModule {
            name: "partitions".to_string()
        })
    );
}

#[test]
fn extra_inline_module_allowed() {
    let rs = r#"
mod partitions;
mod codec;
mod append;
mod query;
mod timer_index;

pub mod runtime {
    // I am a runtime module that violates the contract!
}
"#;
    // Structural checker does not forbid extra modules, only enforces required ones
    let res = ModuleChecker::validate(rs);
    assert_eq!(res, Ok(()));
}
