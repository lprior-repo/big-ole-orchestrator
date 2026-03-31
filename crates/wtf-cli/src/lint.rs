#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use clap::ValueEnum;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use wtf_linter::diagnostic::LintError;
use wtf_linter::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[derive(Default)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}


#[derive(Debug, Clone, Serialize)]
struct JsonDiagnostic {
    code: String,
    severity: String,
    message: String,
    suggestion: Option<String>,
    span: Option<Span>,
    file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Span {
    start: usize,
    end: usize,
}

impl From<&Diagnostic> for JsonDiagnostic {
    fn from(d: &Diagnostic) -> Self {
        Self {
            code: d.code.to_string(),
            severity: d.severity.to_string(),
            message: d.message.clone(),
            suggestion: d.suggestion.clone(),
            span: d.span.map(|(start, end)| Span { start, end }),
            file: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LintCommandError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("lint error: {0}")]
    Lint(#[from] LintError),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("no files found")]
    NoFilesFound,
}

pub fn run_lint(paths: &[PathBuf], format: OutputFormat) -> Result<ExitCode, LintCommandError> {
    let all_diagnostics = collect_diagnostics(paths)?;
    let exit_code = if all_diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };
    emit_diagnostics(&all_diagnostics, format);
    Ok(exit_code)
}

fn collect_diagnostics(paths: &[PathBuf]) -> Result<Vec<Diagnostic>, LintCommandError> {
    let mut all_diagnostics = Vec::new();
    let mut had_parse_error = false;
    for path in paths {
        let results = if path.is_dir() {
            collect_from_directory(path)?
        } else {
            vec![lint_single_file(path)]
        };
        for result in results {
            match result {
                Ok(diagnostics) => all_diagnostics.extend(diagnostics),
                Err(LintError::ParseError(_)) => had_parse_error = true,
            }
        }
    }
    if all_diagnostics.is_empty() && had_parse_error {
        return Ok(vec![]);
    }
    Ok(all_diagnostics)
}

fn collect_from_directory(
    dir: &Path,
) -> Result<Vec<Result<Vec<Diagnostic>, LintError>>, LintCommandError> {
    let mut results = Vec::new();
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            results.extend(collect_from_directory(&path)?);
        } else if is_rust_file(&path) {
            results.push(lint_single_file(&path));
        }
    }
    Ok(results)
}

fn lint_single_file(path: &Path) -> Result<Vec<Diagnostic>, LintError> {
    if !is_rust_file(path) {
        return Ok(vec![]);
    }
    let _content =
        std::fs::read_to_string(path).map_err(|e| LintError::ParseError(e.to_string()))?;
    Ok(vec![])
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "rs")
        .unwrap_or(false)
}

fn emit_diagnostics(diagnostics: &[Diagnostic], format: OutputFormat) {
    match format {
        OutputFormat::Human => emit_human(diagnostics),
        OutputFormat::Json => emit_json(diagnostics),
    }
}

fn emit_human(diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        eprintln!("{d}");
    }
}

fn emit_json(diagnostics: &[Diagnostic]) {
    let json_diags: Vec<JsonDiagnostic> = diagnostics.iter().map(JsonDiagnostic::from).collect();
    if let Err(e) = serde_json::to_writer_pretty(std::io::stdout(), &json_diags) {
        eprintln!("failed to serialize diagnostics: {e}");
    }
}
