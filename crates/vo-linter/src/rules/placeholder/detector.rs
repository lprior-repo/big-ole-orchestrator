#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use syn::visit::Visit;
use syn::{Expr, ExprCall, File, Item, Stmt};

#[must_use]
pub fn check_placeholder_tests(file: &File) -> Vec<Diagnostic> {
    let mut detector = PlaceholderDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}

const PLACEHOLDER_MACROS: &[&str] = &["todo", "unimplemented", "unreachable"];

#[derive(Default)]
struct PlaceholderDetector {
    diagnostics: Vec<Diagnostic>,
}

impl PlaceholderDetector {
    fn check_macro_in_stmt(&mut self, mac: &syn::Macro) {
        let ident = mac.path.segments.last().map(|s| s.ident.to_string());
        if let Some(name) = ident {
            if PLACEHOLDER_MACROS.contains(&name.as_str()) {
                self.diagnostics.push(Diagnostic::new(
                    LintCode::L004,
                    format!(
                        "placeholder macro `{}` found in test; replace with real assertion",
                        name
                    ),
                ));
            }
        }
    }

    fn check_assert_eq_macro(&mut self, mac: &syn::Macro) {
        let ident = mac.path.segments.last().map(|s| s.ident.to_string());
        let Some(name) = ident else { return };
        if name != "assert_eq" && name != "assert_ne" {
            return;
        }

        struct TwoExprs(Expr, Expr);
        impl syn::parse::Parse for TwoExprs {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                let left: Expr = input.parse()?;
                input.parse::<syn::Token![,]>()?;
                let right: Expr = input.parse()?;
                Ok(TwoExprs(left, right))
            }
        }

        let Ok(args) = syn::parse2::<TwoExprs>(mac.tokens.clone()) else {
            return;
        };

        let TwoExprs(left, right) = args;
        if Self::expr_is_constant(&left) && Self::expr_is_same_constant(&left, &right) {
            self.diagnostics.push(Diagnostic::new(
                LintCode::L004,
                format!(
                    "{} with identical arguments is a tautological placeholder; test real behavior",
                    name
                ),
            ));
        }
    }

    fn check_assert_true(&mut self, call: &ExprCall) {
        if let Expr::Path(p) = &*call.func {
            if p.path.is_ident("assert") {
                if let Some(first_arg) = call.args.first() {
                    if let Expr::Lit(lit) = first_arg {
                        if let syn::Lit::Bool(b) = &lit.lit {
                            if b.value {
                                self.diagnostics.push(Diagnostic::new(
                                    LintCode::L004,
                                    "assert!(true) is a no-op placeholder; provide a real condition",
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_constant_only_assert(&mut self, call: &ExprCall) {
        let func_name = if let Expr::Path(p) = &*call.func {
            let segs = &p.path.segments;
            if segs.len() == 1 {
                let ident = &segs[0].ident;
                if ident == "assert_eq" || ident == "assert_ne" {
                    Some(ident.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(name) = func_name {
            let args = &call.args;
            if args.len() >= 2 {
                let left = &args[0];
                let right = &args[1];
                if Self::expr_is_constant(&left) && Self::expr_is_same_constant(&left, &right) {
                    self.diagnostics.push(Diagnostic::new(
                        LintCode::L004,
                        format!(
                            "{} with identical arguments is a tautological placeholder; test real behavior",
                            name
                        ),
                    ));
                }
            }
        }
    }

    fn expr_is_constant(expr: &Expr) -> bool {
        match expr {
            Expr::Lit(_) => true,
            Expr::Path(p) if p.qself.is_none() => {
                let segs = &p.path.segments;
                if segs.is_empty() {
                    return false;
                }
                let last = &segs[segs.len() - 1].ident;
                let s = last.to_string();
                s.chars().next().map_or(false, |c| c.is_uppercase())
            }
            _ => false,
        }
    }

    fn expr_is_same_constant(left: &Expr, right: &Expr) -> bool {
        format!("{:?}", left) == format!("{:?}", right)
    }

    fn check_ignored_test(&mut self, attrs: &[syn::Attribute]) {
        for attr in attrs {
            if attr.path().is_ident("ignore") {
                self.diagnostics.push(Diagnostic::new(
                    LintCode::L004,
                    "test is marked #[ignore]; remove or implement the test",
                ));
            }
        }
    }
}

impl<'ast> Visit<'ast> for PlaceholderDetector {
    fn visit_item(&mut self, node: &'ast Item) {
        if let Item::Fn(f) = node {
            let is_test = f.attrs.iter().any(|a| a.path().is_ident("test"));
            let is_async_test = f.attrs.iter().any(|a| a.path().is_ident("tokio::test"));
            if is_test || is_async_test {
                self.check_ignored_test(&f.attrs);
                for stmt in &f.block.stmts {
                    self.visit_stmt(stmt);
                }
                return;
            }
        }
        syn::visit::visit_item(self, node);
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        match node {
            Stmt::Expr(e, _semi) => {
                if let Expr::Macro(m) = e {
                    self.check_macro_in_stmt(&m.mac);
                    self.check_assert_eq_macro(&m.mac);
                    self.check_assert_macro(&m.mac);
                } else if let Expr::Call(c) = e {
                    self.check_assert_true(c);
                    self.check_constant_only_assert(c);
                }
            }
            _ => {}
        }
        syn::visit::visit_stmt(self, node);
    }

    fn check_assert_macro(&mut self, mac: &syn::Macro) {
        let ident = mac.path.segments.last().map(|s| s.ident.to_string());
        let Some(name) = ident else { return };
        if name != "assert" {
            return;
        }

        struct SingleArg(syn::Expr);
        impl syn::parse::Parse for SingleArg {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                let expr: syn::Expr = input.parse()?;
                Ok(SingleArg(expr))
            }
        }

        let Ok(args) = syn::parse2::<SingleArg>(mac.tokens.clone()) else {
            return;
        };

        if let syn::Expr::Lit(lit) = &args.0 {
            if let syn::Lit::Bool(b) = &lit.lit {
                if b.value {
                    self.diagnostics.push(Diagnostic::new(
                        LintCode::L004,
                        "assert!(true) is a no-op placeholder; provide a real condition",
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod bdd_placeholder_tests {
    use super::*;
    use quote::quote;
    use syn::parse_str;

    fn lint(src: &str) -> Vec<Diagnostic> {
        let file: File = parse_str(src).expect("parse failed");
        check_placeholder_tests(&file)
    }

    mod detects_placeholder_macros {
        use super::*;

        #[test]
        fn todo_macro_is_detected() {
            let src = quote! {
                #[test]
                fn feature_pending() {
                    todo!()
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("`todo`") && m.contains("placeholder")),
                "Expected diagnostic about todo placeholder, got: {:?}",
                messages
            );
        }

        #[test]
        fn unimplemented_macro_is_detected() {
            let src = quote! {
                #[test]
                fn not_yet_implemented() {
                    unimplemented!()
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("`unimplemented`") && m.contains("placeholder")),
                "Expected diagnostic about unimplemented placeholder, got: {:?}",
                messages
            );
        }

        #[test]
        fn unreachable_macro_is_detected() {
            let src = quote! {
                #[test]
                fn dead_code_path() {
                    unreachable!()
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("`unreachable`") && m.contains("placeholder")),
                "Expected diagnostic about unreachable placeholder, got: {:?}",
                messages
            );
        }
    }

    mod detects_noop_assertions {
        use super::*;

        #[test]
        fn assert_true_is_detected_as_noop() {
            let src = quote! {
                #[test]
                fn test_something() {
                    assert!(true);
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("assert!(true)") && m.contains("no-op placeholder")),
                "Expected diagnostic about assert!(true) no-op, got: {:?}",
                messages
            );
        }
    }

    mod detects_ignored_tests {
        use super::*;

        #[test]
        fn ignored_test_attribute_is_detected() {
            let src = quote! {
                #[test]
                #[ignore]
                fn skipped_test() {
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("#[ignore]")),
                "Expected diagnostic about #[ignore] test, got: {:?}",
                messages
            );
        }
    }

    mod detects_constant_only_assertions {
        use super::*;

        #[test]
        fn assert_eq_with_same_constants_is_detected() {
            let src = quote! {
                const VALUE: i32 = 42;
                #[test]
                fn test_constant() {
                    assert_eq!(VALUE, VALUE);
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("assert_eq") && m.contains("identical arguments") && m.contains("tautological placeholder")),
                "Expected diagnostic about tautological assert_eq, got: {:?}",
                messages
            );
        }

        #[test]
        fn assert_ne_with_same_constants_is_detected() {
            let src = quote! {
                const VALUE: i32 = 42;
                #[test]
                fn test_constant() {
                    assert_ne!(VALUE, VALUE);
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                messages.iter().any(|m| m.contains("assert_ne") && m.contains("identical arguments") && m.contains("tautological placeholder")),
                "Expected diagnostic about tautological assert_ne, got: {:?}",
                messages
            );
        }

        #[test]
        fn literal_constants_assertion_is_detected() {
            let src = quote! {
                #[test]
                fn test_literals() {
                    assert_eq!(1, 1);
                }
            };
            let diags = lint(&src.to_string());
            let messages: Vec<_> = diags.iter().map(|d| d.message()).collect();
            assert!(
                !messages.is_empty(),
                "Expected diagnostic about literal constant assertion, got empty"
            );
        }
    }

    mod production_path_exercise {
        use super::*;

        #[test]
        fn real_assertion_passes_without_diagnostic() {
            let src = quote! {
                #[test]
                fn testAddition() {
                    assert_eq!(2 + 2, 4);
                }
            };
            let diags = lint(&src.to_string());
            let placeholder_diags: Vec<_> = diags
                .iter()
                .filter(|d| d.message().contains("placeholder") || d.message().contains("noop"))
                .collect();
            assert!(
                placeholder_diags.is_empty(),
                "Real assertions should not trigger placeholder diagnostics, got: {:?}",
                placeholder_diags
            );
        }

        #[test]
        fn real_comparison_passes_without_diagnostic() {
            let src = quote! {
                #[test]
                fn testOrdering() {
                    assert!(5 > 3);
                }
            };
            let diags = lint(&src.to_string());
            let placeholder_diags: Vec<_> = diags
                .iter()
                .filter(|d| d.message().contains("placeholder") || d.message().contains("noop"))
                .collect();
            assert!(
                placeholder_diags.is_empty(),
                "Real comparisons should not trigger placeholder diagnostics, got: {:?}",
                placeholder_diags
            );
        }
    }

    mod exact_once_evidence {
        use super::*;

        #[test]
        fn single_placeholder_produces_single_diagnostic() {
            let src = quote! {
                #[test]
                fn test_something() {
                    todo!()
                }
            };
            let diags = lint(&src.to_string());
            assert_eq!(
                diags.len(),
                1,
                "Expected exactly one diagnostic for single placeholder, got {}",
                diags.len()
            );
        }

        #[test]
        fn multiple_placeholders_produce_multiple_diagnostics() {
            let src = quote! {
                #[test]
                fn test_something() {
                    todo!();
                    unimplemented!();
                }
            };
            let diags = lint(&src.to_string());
            assert_eq!(
                diags.len(),
                2,
                "Expected two diagnostics for two placeholders, got {}",
                diags.len()
            );
        }
    }
}