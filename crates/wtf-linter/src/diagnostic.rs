#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use thiserror::Error;

/// Lint rule codes (ADR-020).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintCode {
    /// Non-deterministic time call in workflow function.
    L001,
    /// Non-deterministic random call in workflow function.
    L002,
    /// Direct async I/O in workflow function.
    L003,
    /// ctx.* call inside closure with non-deterministic dispatch order.
    L004,
    /// `tokio::spawn` inside workflow function.
    L005,
    /// `std::thread::spawn` inside workflow function.
    L006,
}

impl LintCode {
    #[must_use]
    pub fn severity(self) -> Severity {
        match self {
            Self::L004 => Severity::Warning,
            _ => Severity::Error,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L001 => "WTF-L001",
            Self::L002 => "WTF-L002",
            Self::L003 => "WTF-L003",
            Self::L004 => "WTF-L004",
            Self::L005 => "WTF-L005",
            Self::L006 => "WTF-L006",
        }
    }
}

impl std::fmt::Display for LintCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

/// A lint diagnostic emitted by a lint rule.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: LintCode,
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Optional suggestion for how to fix the violation.
    pub suggestion: Option<String>,
    /// Byte span in the source file (start, end).
    pub span: Option<(usize, usize)>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(code: LintCode, message: impl Into<String>) -> Self {
        Self {
            severity: code.severity(),
            code,
            message: message.into(),
            suggestion: None,
            span: None,
        }
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        if let Some(ref s) = self.suggestion {
            write!(f, "\n  = note: {s}")?;
        }
        Ok(())
    }
}

/// Error returned when linting fails at the parse stage (not a lint violation).
#[derive(Debug, Error)]
pub enum LintError {
    #[error("failed to parse source: {0}")]
    ParseError(String),
}
