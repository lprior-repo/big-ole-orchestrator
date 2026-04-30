#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode, Severity};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{File, Ident, UseTree};

#[derive(Default)]
struct UnusedImportDetector {
    imported_names: HashSet<String>,
    referenced_names: HashSet<String>,
}

fn extract_ident_from_use_tree(tree: &UseTree, prefix: &str) -> Vec<String> {
    match tree {
        UseTree::Path(path) => {
            let ident_str = path.ident.to_string();
            let new_prefix = if prefix.is_empty() {
                ident_str
            } else {
                format!("{}::{}", prefix, ident_str)
            };
            extract_ident_from_use_tree(&path.tree, &new_prefix)
        }
        UseTree::Name(name) => {
            vec![format!("{}::{}", prefix, name.ident)]
        }
        UseTree::Rename(rename) => {
            vec![format!("{}::{}", prefix, rename.rename)]
        }
        UseTree::Glob(_glob) => {
            if prefix.is_empty() {
                vec![]
            } else {
                vec![prefix.to_string()]
            }
        }
        UseTree::Group(group) => {
            let mut ids = Vec::new();
            for item in &group.items {
                ids.extend(extract_ident_from_use_tree(item, prefix));
            }
            ids
        }
    }
}

impl<'ast> Visit<'ast> for UnusedImportDetector {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let items = extract_ident_from_use_tree(&node.tree, "");

        for item in items {
            if !item.is_empty() {
                self.imported_names.insert(item.clone());
            }
        }
    }

    fn visit_ident(&mut self, node: &'ast Ident) {
        self.referenced_names.insert(node.to_string());
        syn::visit::visit_ident(self, node);
    }
}

#[must_use]
pub fn check_unused_imports(file: &File) -> Vec<Diagnostic> {
    let mut detector = UnusedImportDetector::default();
    detector.visit_file(file);

    let mut diagnostics = Vec::new();

    for imported in &detector.imported_names {
        let name = imported.split("::").last().unwrap_or(imported);

        if !detector.referenced_names.contains(name) {
            let segments: Vec<_> = imported.split("::").map(|s| s.to_string()).collect();
            let suggestion = if segments.len() > 1 {
                format!("remove the unused import of `{}`", imported)
            } else {
                format!("remove the unused import `{}`", imported)
            };

            diagnostics.push(
                Diagnostic::new(LintCode::L001, format!("unused import: `{}`", imported))
                    .with_suggestion(&suggestion)
                    .with_severity(Severity::Warning),
            );
        }
    }

    diagnostics.sort_by(|a, b| a.message.cmp(&b.message));
    diagnostics
}
