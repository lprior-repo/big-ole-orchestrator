#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode, LintError};
use syn::visit::Visit;
use syn::{spanned::Spanned, Expr, ExprAwait, ExprCall, Path};

const SUGGESTION: &str =
    "direct async I/O in workflow function — wrap in ctx.activity(\"name\", input) instead";

pub fn lint_workflow_code(source: &str) -> Result<Vec<Diagnostic>, LintError> {
    let syntax_tree = syn::parse_file(source).map_err(|e| LintError::ParseError(e.to_string()))?;
    let mut visitor = L003Visitor::new();
    visitor.visit_file(&syntax_tree);
    Ok(visitor.diagnostics)
}

struct L003Visitor {
    diagnostics: Vec<Diagnostic>,
}

impl L003Visitor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    fn emit_diagnostic(&mut self, span: Option<(usize, usize)>) {
        let mut diagnostic =
            Diagnostic::new(LintCode::L003, "direct async I/O in workflow function")
                .with_suggestion(SUGGESTION);
        if let Some((start, end)) = span {
            diagnostic.span = Some((start, end));
        }
        self.diagnostics.push(diagnostic);
    }

    fn is_direct_io_path(path: &Path) -> bool {
        let path_str = path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        path_str.contains("reqwest")
            || path_str.contains("sqlx")
            || path_str.contains("tokio::fs")
            || path_str.contains("tokio::net")
            || path_str.contains("hyper")
            || path_str.starts_with("std::fs::")
    }

    fn check_expr_for_direct_io(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(call_expr) => {
                if let Expr::Path(path_expr) = call_expr.func.as_ref() {
                    if Self::is_direct_io_path(&path_expr.path) {
                        self.emit_diagnostic(Some(loc_of(expr)));
                    }
                }
                if Self::is_sqlx_query_builder_call(call_expr) {
                    self.emit_diagnostic(Some(loc_of(expr)));
                }
            }
            Expr::MethodCall(method_expr) => {
                let method_name = method_expr.method.to_string();
                if matches!(
                    method_name.as_str(),
                    "get"
                        | "post"
                        | "put"
                        | "delete"
                        | "fetch"
                        | "fetch_one"
                        | "fetch_optional"
                        | "fetch_all"
                ) && Self::is_method_chain_from_direct_io(&method_expr.receiver)
                {
                    self.emit_diagnostic(Some(loc_of(expr)));
                }
            }
            _ => {}
        }
    }

    fn is_sqlx_query_builder_call(call: &ExprCall) -> bool {
        if let Expr::Path(path_expr) = call.func.as_ref() {
            let path_str = path_expr
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            return path_str.starts_with("sqlx::query")
                || path_str.starts_with("sqlx::query_as")
                || path_str.starts_with("sqlx::query_scalar");
        }
        false
    }

    fn is_method_chain_from_direct_io(receiver: &Expr) -> bool {
        match receiver {
            Expr::Call(call_expr) => {
                if let Expr::Path(path_expr) = call_expr.func.as_ref() {
                    let path_str = path_expr
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    return path_str.starts_with("sqlx::query")
                        || path_str.starts_with("sqlx::query_as")
                        || path_str.starts_with("sqlx::query_scalar");
                }
            }
            Expr::MethodCall(method_expr) => {
                return Self::is_method_chain_from_direct_io(&method_expr.receiver);
            }
            _ => {}
        }
        false
    }

    fn check_await(&mut self, await_expr: &ExprAwait) {
        self.check_expr_for_direct_io(&await_expr.base);
    }
}

impl Default for L003Visitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> syn::visit::Visit<'ast> for L003Visitor {
    fn visit_expr_await(&mut self, await_expr: &'ast ExprAwait) {
        self.check_await(await_expr);
        syn::visit::visit_expr_await(self, await_expr);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Call(call_expr) = expr {
            if let Expr::Path(path_expr) = call_expr.func.as_ref() {
                let path_str = path_expr
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                if path_str.starts_with("std::fs::") {
                    self.emit_diagnostic(Some(loc_of(expr)));
                }
            }
        }
        syn::visit::visit_expr(self, expr);
    }
}

fn loc_of(expr: &Expr) -> (usize, usize) {
    (expr.span().start().column, expr.span().end().column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Result<Vec<Diagnostic>, LintError> {
        lint_workflow_code(source)
    }

    #[test]
    fn test_emits_no_diagnostic_for_ctx_activity() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    ctx.activity("fetch", bytes).await;
    Ok(())
}
"#;
        let result = check(source);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_emits_diagnostic_for_reqwest_get_await() {
        let source = r#"
async fn workflow(ctx: &Ctx) -> Result<(), Error> {
    reqwest::get("https://example.com").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_sqlx_query_await() {
        let source = r#"
async fn workflow(pool: &Pool) -> Result<(), Error> {
    sqlx::query("SELECT * FROM t").fetch_one(pool).await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_tokio_fs_read_await() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    tokio::fs::read("file.txt").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_std_fs_read() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    let data = std::fs::read("file.txt")?;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, LintCode::L003);
    }

    #[test]
    fn test_emits_diagnostic_for_hyper_request() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    hyper::Client::request(request).await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert!(!result.is_empty());
        assert!(result.iter().all(|d| d.code == LintCode::L003));
    }

    #[test]
    fn test_emits_diagnostic_for_tokio_net_tcpstream() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    tokio::net::TcpStream::connect("127.0.0.1:8080").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert!(!result.is_empty());
        assert!(result.iter().all(|d| d.code == LintCode::L003));
    }

    #[test]
    fn test_emits_multiple_diagnostics_for_multiple_violations() {
        let source = r#"
async fn workflow(pool: &Pool) -> Result<(), Error> {
    let _ = reqwest::get("https://a.com").await;
    let _ = sqlx::query("SELECT 1").fetch_one(pool).await;
    let _ = tokio::fs::read("f.txt").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|d| d.code == LintCode::L003));
    }

    #[test]
    fn test_diagnostic_code_is_wtf_l003() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    reqwest::get("https://example.com").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert_eq!(result[0].code.as_str(), "WTF-L003");
    }

    #[test]
    fn test_diagnostic_suggestion_contains_ctx_activity() {
        let source = r#"
async fn workflow() -> Result<(), Error> {
    reqwest::get("https://example.com").await;
    Ok(())
}
"#;
        let result = check(source).expect("should parse");
        assert!(result[0]
            .suggestion
            .as_ref()
            .is_some_and(|s| s.contains("ctx.activity")));
    }

    #[test]
    fn test_returns_parse_error_for_invalid_rust() {
        let source = "async fn workflow { // missing parentheses";
        let result = check(source);
        assert!(result.is_err());
    }
}
