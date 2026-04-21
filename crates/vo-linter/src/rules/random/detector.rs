#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use std::collections::HashMap;
use syn::{visit::Visit, ExprCall, File, ItemUse, Path, UseTree};

#[must_use]
pub fn check_random_in_workflow(file: &File) -> Vec<Diagnostic> {
    let mut detector = RandomDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}

fn path_contains(path: &Path, segment: &str, use_renames: &HashMap<String, String>) -> bool {
    path.segments.iter().any(|s| {
        let ident_str = s.ident.to_string();
        let resolved = use_renames
            .get(&ident_str)
            .map(|r| r.as_str())
            .unwrap_or(&ident_str);
        resolved == segment
    })
}

fn is_uuid_new_v4_call(call: &ExprCall, use_renames: &HashMap<String, String>) -> bool {
    if !call.args.is_empty() {
        return false;
    }
    let path = match &*call.func {
        syn::Expr::Path(p) => Some(&p.path),
        _ => None,
    };
    path.is_some_and(|p| {
        path_contains(p, "Uuid", use_renames) && path_contains(p, "new_v4", use_renames)
    })
}

fn is_rand_random_call(call: &ExprCall, use_renames: &HashMap<String, String>) -> bool {
    let path = match &*call.func {
        syn::Expr::Path(p) => Some(&p.path),
        _ => None,
    };
    path.is_some_and(|p| {
        path_contains(p, "rand", use_renames) && path_contains(p, "random", use_renames)
    })
}

#[derive(Default)]
struct RandomDetector {
    diagnostics: Vec<Diagnostic>,
    use_renames: HashMap<String, String>,
}

fn collect_use_rename(tree: &UseTree, renames: &mut HashMap<String, String>) {
    match tree {
        UseTree::Path(path) => {
            collect_use_rename(&path.tree, renames);
        }
        UseTree::Rename(rename_tree) => {
            renames.insert(
                rename_tree.rename.to_string(),
                rename_tree.ident.to_string(),
            );
        }
        _ => {}
    }
}

impl<'ast> Visit<'ast> for RandomDetector {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        collect_use_rename(&node.tree, &mut self.use_renames);
        syn::visit::visit_item_use(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if is_uuid_new_v4_call(node, &self.use_renames)
            || is_rand_random_call(node, &self.use_renames)
        {
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
