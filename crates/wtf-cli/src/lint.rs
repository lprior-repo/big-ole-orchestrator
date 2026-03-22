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
pub enum OutputFormat {
    Human,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Human
    }
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

const RULE_EXPLANATIONS: &[(&str, &str)] = &[
    (
        "WTF-L001",
        "Non-deterministic time call detected. \
        Workflow functions must produce identical results on every replay. \
        Use `ctx.now()` instead of `std::time::SystemTime::now()`, \
        `chrono::Utc::now()`, or `chrono::Local::now()`.",
    ),
    (
        "WTF-L002",
        "Non-deterministic random call detected. \
        Workflow functions must produce identical results on every replay. \
        Use `ctx.random_u64()` instead of `rand::random()`, \
        `rand::thread_rng()`, or `uuid::Uuid::new_v4()`.",
    ),
    (
        "WTF-L003",
        "Direct async I/O detected. \
        Workflow functions must not perform direct async I/O. \
        Wrap I/O operations in `ctx.activity(\"name\", input)` instead.",
    ),
    (
        "WTF-L004",
        "ctx.* call inside closure with non-deterministic dispatch order. \
        Closures passed to `.map()`, `.for_each()`, `.fold()`, or `.filter_map()` \
        may execute in any order. Use `ctx.parallel()` or sequential iteration.",
    ),
    (
        "WTF-L005",
        "tokio::spawn detected. \
        `tokio::spawn` creates untracked async work that is not logged. \
        Use `ctx.activity()` to ensure the result is logged and replayable.",
    ),
    (
        "WTF-L006",
        "std::thread::spawn detected. \
        Thread spawn is not replayable. \
        Use `ctx.activity()` instead.",
    ),
];

pub fn explain_rule(rule_code: &str) -> Option<String> {
    RULE_EXPLANATIONS
        .iter()
        .find(|(code, _)| *code == rule_code)
        .map(|(_, explanation)| explanation.to_string())
}

pub fn run_lint(
    paths: &[PathBuf],
    format: OutputFormat,
    check: bool,
) -> Result<ExitCode, LintCommandError> {
    let file_diags = collect_diagnostics(paths)?;
    let total_diagnostics: usize = file_diags.iter().map(|(_, d)| d.len()).sum();
    let exit_code = if total_diagnostics == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };
    if !check {
        for (path, diags) in &file_diags {
            emit_diagnostics(diags, format, path);
        }
    }
    Ok(exit_code)
}

fn collect_diagnostics(
    paths: &[PathBuf],
) -> Result<Vec<(PathBuf, Vec<Diagnostic>)>, LintCommandError> {
    let mut all_file_diags = Vec::new();
    let mut had_parse_error = false;
    for path in paths {
        if path.is_dir() {
            for file_diag in collect_from_directory(path)? {
                match file_diag {
                    Ok((p, diags)) => {
                        all_file_diags.push((p, diags));
                    }
                    Err(LintError::ParseError(_)) => had_parse_error = true,
                }
            }
        } else {
            match lint_single_file(path) {
                Ok(diags) => all_file_diags.push((path.clone(), diags)),
                Err(LintError::ParseError(_)) => had_parse_error = true,
            }
        }
    }
    if all_file_diags.is_empty() && had_parse_error {
        return Ok(vec![]);
    }
    Ok(all_file_diags)
}

fn collect_from_directory(
    dir: &Path,
) -> Result<Vec<Result<(PathBuf, Vec<Diagnostic>), LintError>>, LintCommandError> {
    let mut results = Vec::new();
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            results.extend(collect_from_directory(&path)?);
        } else if is_rust_file(&path) {
            results.push(lint_single_file(&path).map(|diags| (path, diags)));
        }
    }
    Ok(results)
}

fn lint_single_file(path: &Path) -> Result<Vec<Diagnostic>, LintError> {
    if !is_rust_file(path) {
        return Ok(vec![]);
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| LintError::ParseError(e.to_string()))?;
    wtf_linter::lint_workflow_code(&content)
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "rs")
        .unwrap_or(false)
}

fn emit_diagnostics(diagnostics: &[Diagnostic], format: OutputFormat, file_path: &Path) {
    match format {
        OutputFormat::Human => emit_human(diagnostics, file_path),
        OutputFormat::Json => emit_json(diagnostics),
    }
}

fn emit_human(diagnostics: &[Diagnostic], file_path: &Path) {
    for d in diagnostics {
        let location = if let Some((start, _)) = d.span {
            let line_col = line_col_from_offset(file_path, start);
            format!("{}:{}:{}", file_path.display(), line_col.0, line_col.1)
        } else {
            format!("{}:1:1", file_path.display())
        };
        eprintln!("{} {}WTF-{:?}: {}", location, d.severity, d.code, d.message);
    }
}

fn line_col_from_offset(path: &Path, byte_offset: usize) -> (usize, usize) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (1, 1);
    };
    let mut line = 1;
    let mut col = 1;
    for (i, byte) in content.bytes().enumerate() {
        if i == byte_offset {
            return (line, col);
        }
        if byte == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn emit_json(diagnostics: &[Diagnostic]) {
    let json_diags: Vec<JsonDiagnostic> = diagnostics.iter().map(JsonDiagnostic::from).collect();
    if let Err(e) = serde_json::to_writer_pretty(std::io::stdout(), &json_diags) {
        eprintln!("failed to serialize diagnostics: {e}");
    }
}
