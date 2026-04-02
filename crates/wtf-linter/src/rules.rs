#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use syn::{visit::Visit, ExprCall, File, Path};

#[must_use]
pub fn check_random_in_workflow(file: &File) -> Vec<Diagnostic> {
    let mut detector = RandomDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}

fn path_contains(path: &Path, segment: &str) -> bool {
    path.segments.iter().any(|s| s.ident == segment)
}

fn is_uuid_new_v4_call(call: &ExprCall) -> bool {
    let path = match &*call.func {
        syn::Expr::Path(p) => Some(&p.path),
        _ => None,
    };
    path.is_some_and(|p| path_contains(p, "Uuid") && path_contains(p, "new_v4"))
}

fn is_rand_random_call(call: &ExprCall) -> bool {
    let path = match &*call.func {
        syn::Expr::Path(p) => Some(&p.path),
        _ => None,
    };
    path.is_some_and(|p| path_contains(p, "rand") && path_contains(p, "random"))
}

#[derive(Default)]
struct RandomDetector {
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for RandomDetector {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if is_uuid_new_v4_call(node) || is_rand_random_call(node) {
            self.diagnostics.push(
                Diagnostic::new(
                    LintCode::L002,
                    "non-deterministic random call in workflow function",
                )
                .with_suggestion("use `ctx.random_u64()` instead"),
            );
        }
        syn::visit::visit_expr_call(self, node);
    }
}
