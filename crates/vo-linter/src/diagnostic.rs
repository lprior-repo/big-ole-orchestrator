#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintCode {
    L002,
    L003,
    L004,
    L005,
    L006,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    code: LintCode,
    message: String,
    suggestion: Option<String>,
    severity: Severity,
    field: Option<String>,
    suggested_bound: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(code: LintCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
            severity: Severity::Warning,
            field: None,
            suggested_bound: None,
        }
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    #[must_use]
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    #[must_use]
    pub fn with_suggested_bound(mut self, bound: impl Into<String>) -> Self {
        self.suggested_bound = Some(bound.into());
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }

    #[must_use]
    pub const fn code(&self) -> &LintCode {
        &self.code
    }

    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    #[must_use]
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    #[must_use]
    pub fn suggested_bound(&self) -> Option<&str> {
        self.suggested_bound.as_deref()
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

    #[test]
    fn test_diagnostic_suggestion_none() {
        let d = Diagnostic::new(LintCode::L002, "msg");
        assert!(d.suggestion().is_none());
    }

    #[test]
    fn test_diagnostic_suggestion_some() {
        let d = Diagnostic::new(LintCode::L002, "msg").with_suggestion("use this instead");
        assert!(d.suggestion.is_some());
        assert_eq!(d.suggestion.unwrap(), "use this instead");
    }

    #[test]
    fn test_diagnostic_default_severity_is_warning() {
        let d = Diagnostic::new(LintCode::L002, "test");
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn test_diagnostic_with_severity() {
        let d = Diagnostic::new(LintCode::L002, "test")
            .with_severity(Severity::Error);
        assert_eq!(d.severity, Severity::Error);
    }

    #[test]
    fn test_diagnostic_field() {
        let d = Diagnostic::new(LintCode::L003, "test")
            .with_field("max_attempts");
        assert_eq!(d.field(), Some("max_attempts"));
    }

    #[test]
    fn test_diagnostic_suggested_bound() {
        let d = Diagnostic::new(LintCode::L003, "test")
            .with_suggested_bound("<= 50");
        assert_eq!(d.suggested_bound(), Some("<= 50"));
    }

    #[test]
    fn test_diagnostic_full_chain() {
        let d = Diagnostic::new(LintCode::L003, "max_attempts exceeds safe bound")
            .with_severity(Severity::Warning)
            .with_field("max_attempts")
            .with_suggested_bound("<= 50")
            .with_suggestion("reduce max_attempts");
        assert_eq!(d.code(), &LintCode::L003);
        assert_eq!(d.message(), "max_attempts exceeds safe bound");
        assert_eq!(d.severity(), Severity::Warning);
        assert_eq!(d.field(), Some("max_attempts"));
        assert_eq!(d.suggested_bound(), Some("<= 50"));
        assert_eq!(d.suggestion(), Some("reduce max_attempts"));
    }
}