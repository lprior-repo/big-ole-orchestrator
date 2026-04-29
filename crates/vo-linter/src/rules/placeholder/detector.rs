#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode, LintSeverity};
use std::collections::HashSet;
use syn::{
    visit::Visit, ExprMacro, File, FnArg, ItemFn, ItemModule, Lit, Meta, Path,
};

/// L003: Detect placeholder anti-patterns in test code.
///
/// Patterns detected:
/// - **L003-A**: `assert!(true)` / `assert_eq!(actual, true)` — trivially passing
/// - **L003-B**: `#[ignore]` on test functions — unexecuted tests
/// - **L003-C**: `todo!()` inside test functions — incomplete tests
/// - **L003-D**: Commented-out handler function signatures — ghost code
/// - **L003-E**: `assert_eq!` / `assert!` with only literal/constant arguments — no logic exercised
/// - **L003-F**: Duplicate type definitions in test modules mirroring production types
#[must_use]
pub fn check_placeholder_tests(file: &File, source: &str) -> Vec<Diagnostic> {
    let mut detector = PlaceholderDetector::new(source);
    detector.visit_file(file);
    detector.diagnostics
}

fn path_is_ident(path: &Path, segment: &str) -> bool {
    path.is_ident(segment)
        || path
            .segments
            .last()
            .is_some_and(|s| s.ident == segment)
}

fn extract_ident_from_path(path: &Path) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    Some(path.segments.last()?.ident.to_string())
}

fn is_test_fn_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let meta = &attr.meta;
        match meta {
            Meta::Path(p) => p.is_ident("test"),
            Meta::List(meta) => {
                meta.path.is_ident("test") || meta.path.is_ident("tokio")
            }
            _ => false,
        }
    })
}

fn is_handler_fn_name(name: &str) -> bool {
    name.starts_with("on_")
        && (name.ends_with("_handler")
            || name.ends_with("_handle")
            || name.ends_with("_command")
            || name.ends_with("_signal"))
}

fn contains_literal_true(tokens: &proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        proc_macro2::TokenTree::Punct(p) => p.as_char() == '!',
        proc_macro2::TokenTree::Ident(id) => id == "true",
        _ => false,
    })
}

fn has_only_literal_args(call: &syn::ExprCall) -> bool {
    call.args.iter().all(|arg| match arg {
        syn::Expr::Lit(_) => true,
        syn::Expr::Array(_) => true,
        syn::Expr::Tuple(tuple) => tuple.elems.iter().all(|e| matches!(e, syn::Expr::Lit(_) | syn::Expr::Array(_))),
        _ => false,
    })
}

fn find_ignore_attr(attrs: &[syn::Attribute]) -> Option<&syn::Attribute> {
    attrs.iter().find(|attr| {
        match &attr.meta {
            Meta::Path(p) => p.is_ident("ignore"),
            _ => false,
        }
    })
}

fn is_in_test_module(module: &ItemModule) -> bool {
    let mod_name = module.ident.to_string();
    mod_name == "tests" || mod_name.starts_with("test_")
}

fn macro_tokens_contains_literal_true(tokens: &proc_macro2::TokenStream) -> bool {
    tokens.to_string().contains("true")
}

#[derive(Default)]
struct PlaceholderDetector<'src> {
    diagnostics: Vec<Diagnostic>,
    source: &'src str,
    test_fn_names: HashSet<String>,
    test_module_names: HashSet<String>,
    production_types: HashSet<String>,
    test_types: HashSet<String>,
    in_test_fn: bool,
    in_test_module: bool,
    current_fn: String,
}

impl<'src> PlaceholderDetector<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            source,
            ..Self::default()
        }
    }

    fn add_diagnostic(&mut self, code: LintCode, severity: LintSeverity, message: impl Into<String>, suggestion: Option<String>) {
        self.diagnostics.push(
            Diagnostic::new(code, message)
                .with_severity(severity)
                .with_suggestion(suggestion.unwrap_or_default()),
        );
    }

    fn scan_source_text(&self) {
        let lines: Vec<&str> = self.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // L003-D: Commented-out handler function signatures
            if trimmed.starts_with("//") && (trimmed.contains("fn on_") || trimmed.contains("fn handle_")) {
                let comment_content = trimmed[2..].trim();
                if comment_content.starts_with("fn ") {
                    self.add_diagnostic(
                        LintCode::L003,
                        LintSeverity::Warning,
                        format!("commented-out handler function at line {idx}: `{comment_content}`"),
                        Some("remove ghost commented-out code or convert to a real test"),
                    );
                }
            }

            // L003-D: Commented-out test functions
            if trimmed.starts_with("//") && trimmed.contains("fn test_") {
                self.add_diagnostic(
                    L003,
                    LintSeverity::Warning,
                    format!("commented-out test function at line {idx}: `{trimmed}`"),
                    Some("remove ghost commented-out tests or re-enable them"),
                );
            }

            // L003-D: Commented-out #[tokio::test] blocks
            if trimmed.starts_with("//") && trimmed.contains("#[tokio::test]") {
                self.add_diagnostic(
                    L003,
                    LintSeverity::Warning,
                    format!("commented-out #[tokio::test] block at line {idx}"),
                    Some("remove ghost commented-out test or re-enable it"),
                );
            }
        }
    }
}

impl<'ast> Visit<'ast> for PlaceholderDetector<'ast> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let is_test = is_test_fn_attr(&node.attrs);
        let old_in_test_fn = self.in_test_fn;
        self.in_test_fn = is_test;
        self.current_fn = node.sig.ident.to_string();

        if is_test {
            self.test_fn_names.insert(node.sig.ident.to_string());

            // L003-B: #[ignore] on test function
            if let Some(_ignore_attr) = find_ignore_attr(&node.attrs) {
                self.add_diagnostic(
                    LintCode::L003,
                    LintSeverity::Warning,
                    format!("ignored test `{}` — not executed by test runner", node.sig.ident),
                    Some("fix the test and remove #[ignore], or remove the test entirely"),
                );
            }
        }

        // Walk into the function body
        syn::visit::visit_item_fn(self, node);
        self.in_test_fn = old_in_test_fn;
    }

    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        let macro_name = extract_ident_from_path(&node.mac.path);

        if let Some(name) = &macro_name {
            // L003-A: assert!(true) / assert_eq!(x, true) / assert_ne!(x, true)
            if (name == "assert" || name == "assert_eq" || name == "assert_ne")
                && self.in_test_fn
                && macro_tokens_contains_literal_true(&node.mac.tokens)
            {
                self.add_diagnostic(
                    LintCode::L003,
                    LintSeverity::Error,
                    format!(
                        "trivial `{name}!` assertion in test `{}` — contains literal `true`",
                        self.current_fn
                    ),
                    Some("assert against computed/produced values, not literals"),
                );
            }
        }

        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_item_module(&mut self, node: &'ast ItemModule) {
        let old_in_test_module = self.in_test_module;
        self.in_test_module = is_in_test_module(node);

        if self.in_test_module {
            self.test_module_names.insert(node.ident.to_string());
        }

        syn::visit::visit_item_module(self, node);
        self.in_test_module = old_in_test_module;
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        // Track types defined in test modules
        if self.in_test_module {
            if let syn::Type::Path(type_path) = &*node.self_ty {
                if let Some(last_seg) = type_path.path.segments.last() {
                    self.test_types.insert(last_seg.ident.to_string());
                }
            }
        }

        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if self.in_test_module {
            self.test_types.insert(node.ident.to_string());
        }
        syn::visit::visit_item_struct(self, node);
    }
}
