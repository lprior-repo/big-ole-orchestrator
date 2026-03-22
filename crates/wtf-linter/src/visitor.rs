#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

// Placeholder — the syn Visit trait implementations live in the rule modules.
// This module will re-export the combined visitor once rules are implemented.

use syn::{spanned::Spanned, visit::Visit, Expr, ExprCall, ExprMethodCall, Path};

use crate::diagnostic::{Diagnostic, LintCode};

const SUGGESTION: &str = "wrap in ctx.activity(...)";

pub struct DirectAsyncIoVisitor {
    pub diagnostics: Vec<Diagnostic>,
}

impl DirectAsyncIoVisitor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn emit_diagnostic(&mut self, span: Option<(usize, usize)>) {
        let diagnostic = Diagnostic::new(LintCode::L003, "direct async I/O in workflow function")
            .with_suggestion(SUGGESTION);
        let diagnostic = match span {
            Some((start, end)) => Diagnostic {
                span: Some((start, end)),
                ..diagnostic
            },
            None => diagnostic,
        };
        self.diagnostics.push(diagnostic);
    }

    fn is_reqwest_get_call(&self, path: &Path) -> bool {
        path.segments.len() == 2
            && path.segments[0].ident == "reqwest"
            && path.segments[1].ident == "get"
    }

    fn is_reqwest_method_call(&self, expr: &ExprMethodCall) -> bool {
        let receiver = &expr.receiver;
        let method = &expr.method;

        if method == "get"
            || method == "post"
            || method == "put"
            || method == "delete"
            || method == "patch"
        {
            if let Expr::Path(path_expr) = receiver.as_ref() {
                let path = &path_expr.path;
                return path.segments.len() == 2
                    && path.segments[0].ident == "reqwest"
                    && (path.segments[1].ident == "Client"
                        || path.segments[1].ident == "blocking");
            }
        }
        false
    }

    fn is_sqlx_fetch_method(&self, method: &syn::Ident) -> bool {
        method == "fetch_one"
            || method == "fetch_optional"
            || method == "fetch_all"
            || method == "fetch_many"
            || method == "fetch_paginated"
    }

    fn is_sqlx_query_call(&self, path: &Path) -> bool {
        path.segments.len() == 2
            && path.segments[0].ident == "sqlx"
            && path.segments[1].ident == "query"
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(call_expr) => {
                if let Expr::Path(path_expr) = call_expr.func.as_ref() {
                    if self.is_reqwest_get_call(&path_expr.path) {
                        let span = loc_of(expr);
                        self.emit_diagnostic(span);
                    } else if self.is_sqlx_query_call(&path_expr.path) {
                        let span = loc_of(expr);
                        self.emit_diagnostic(span);
                    }
                }
            }
            Expr::MethodCall(method_expr) => {
                if self.is_reqwest_method_call(method_expr) {
                    let span = loc_of(expr);
                    self.emit_diagnostic(span);
                } else if self.is_sqlx_fetch_method(&method_expr.method) {
                    if let Expr::Call(inner_call) = method_expr.receiver.as_ref() {
                        if let Expr::Path(path_expr) = inner_call.func.as_ref() {
                            if self.is_sqlx_query_call(&path_expr.path) {
                                let span = loc_of(expr);
                                self.emit_diagnostic(span);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Default for DirectAsyncIoVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> Visit<'ast> for DirectAsyncIoVisitor {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        self.check_expr(expr);
        syn::visit::visit_expr(self, expr);
    }
}

fn loc_of(expr: &Expr) -> Option<(usize, usize)> {
    Some((expr.span().start().column, expr.span().end().column))
}

pub fn check_direct_async_io(
    source: &str,
) -> Result<Vec<Diagnostic>, crate::diagnostic::LintError> {
    let file = syn::parse_file(source).map_err(|e| {
        crate::diagnostic::LintError::ParseError(format!("failed to parse source: {}", e))
    })?;

    let mut visitor = DirectAsyncIoVisitor::new();
    visitor.visit_file(&file);
    Ok(visitor.into_diagnostics())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::LintError;

    fn check(source: &str) -> Result<Vec<Diagnostic>, LintError> {
        check_direct_async_io(source)
    }

    #[test]
    fn test_emits_no_diagnostic_for_code_without_async_io() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    let data = ctx.activity("fetch", ()).await?;
    Ok(())
}
"#;
        let result = check(source);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_emits_diagnostic_for_reqwest_get_call() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    let resp = reqwest::get("https://example.com").await?;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_sqlx_query_fetch_one() {
        let source = r#"
async fn workflow(ctx: &Ctx, pool: &MyPool) -> Result<(), Error> {
    let row = sqlx::query("SELECT * FROM users")
        .fetch_one(pool)
        .await?;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_multiple_violations() {
        let source = r#"
async fn workflow(ctx: &Ctx, pool: &MyPool) -> Result<(), Error> {
    let resp = reqwest::get("https://example.com").await?;
    let row = sqlx::query("SELECT * FROM users")
        .fetch_one(pool)
        .await?;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_returns_parse_error_for_invalid_rust() {
        let source = "async fn workflow { // missing parentheses";
        let result = check(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_handles_reqwest_post_method() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    let resp = reqwest::Client::new()
        .post("https://example.com")
        .await?;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_visitor_initializes_with_empty_diagnostics() {
        let visitor = DirectAsyncIoVisitor::new();
        assert!(visitor.into_diagnostics().is_empty());
    }

    #[test]
    fn test_diagnostic_contains_correct_lint_code() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    let _ = reqwest::get("https://example.com").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert!(!result.is_empty());
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_diagnostic_contains_suggestion() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    let _ = reqwest::get("https://example.com").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert!(!result.is_empty());
        assert_eq!(
            result[0].suggestion.as_deref(),
            Some("wrap in ctx.activity(...)")
        );
    }
}
