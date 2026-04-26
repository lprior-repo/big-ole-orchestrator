#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintCode {
    L002,
    L003,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintSeverity {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintSeverity::Info => write!(f, "INFO"),
            LintSeverity::Warning => write!(f, "WARNING"),
            LintSeverity::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum LintError {
    #[error("parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: LintCode,
    pub message: String,
    pub suggestion: Option<String>,
    pub severity: LintSeverity,
    pub span: Option<(usize, usize)>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(code: LintCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
            severity: LintSeverity::Warning,
            span: None,
        }
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    #[must_use]
    pub fn with_severity(mut self, severity: LintSeverity) -> Self {
        self.severity = severity;
        self
    }

    #[must_use]
    pub fn with_span(mut self, start: usize, end: usize) -> Self {
        self.span = Some((start, end));
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_new() {
        let d = Diagnostic::new(LintCode::L002, "test message");
        assert!(matches!(d.code, LintCode::L002));
        assert_eq!(d.message, "test message");
        assert!(d.suggestion.is_none());
    }

    #[test]
    fn test_diagnostic_with_suggestion() {
        let d = Diagnostic::new(LintCode::L002, "test message").with_suggestion("use this instead");
        assert!(d.suggestion.is_some());
        assert_eq!(d.suggestion.as_ref().unwrap(), "use this instead");
    }

    #[test]
    fn test_diagnostic_message() {
        let d = Diagnostic::new(LintCode::L002, "random UUID call detected");
        assert_eq!(d.message(), "random UUID call detected");
    }

    #[test]
    fn test_diagnostic_message_empty() {
        let d = Diagnostic::new(LintCode::L002, "");
        assert_eq!(d.message(), "");
    }

    #[test]
    fn test_diagnostic_message_unicode() {
        let d = Diagnostic::new(LintCode::L002, "error: \u{274c} invalid");
        assert!(d.message().contains('\u{274c}'));
    }

    #[test]
    fn test_diagnostic_clone() {
        let d = Diagnostic::new(LintCode::L002, "msg").with_suggestion("fix");
        let d2 = d.clone();
        assert_eq!(d.message(), d2.message());
    }
}
