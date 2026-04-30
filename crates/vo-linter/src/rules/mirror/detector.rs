#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use syn::{visit::Visit, File, Item};

#[must_use]
pub fn check_mirror_types_in_api_test(file: &File) -> Vec<Diagnostic> {
    let mut detector = MirrorDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}

#[derive(Default)]
struct MirrorDetector {
    diagnostics: Vec<Diagnostic>,
}

impl MirrorDetector {
    fn check_struct_is_mirror(&self, name: &str) -> bool {
        name.contains("Mirror") || name.contains("Fake") || name.contains("Mock")
    }

    fn check_mirror_comment(&self, attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if let syn::Meta::NameValue(meta) = &attr.meta {
                if meta.path.is_ident("mirror_of") || meta.path.is_ident("fake") {
                    return true;
                }
                if let syn::Expr::Lit(lit) = &meta.value {
                    if let syn::Lit::Str(s) = &lit.lit {
                        return s.value().contains("mirror");
                    }
                }
            }
            false
        })
    }
}

impl<'ast> Visit<'ast> for MirrorDetector {
    fn visit_item(&mut self, node: &'ast Item) {
        if let Item::Struct(item_struct) = node {
            let name = item_struct.ident.to_string();
            if name.contains("SseEvent")
                || name.contains("Event")
                || name.contains("Handler")
                || name.contains("Broadcaster")
            {
                if self.check_struct_is_mirror(&name)
                    || self.check_mirror_comment(&item_struct.attrs)
                {
                    self.diagnostics.push(
                        Diagnostic::new(
                            LintCode::L003,
                            format!("API test defines local mirror type `{}` instead of production handler", name),
                        )
                        .with_suggestion("use production handler type from handlers/sse.rs instead"),
                    );
                }
            }
        }
        syn::visit::visit_item(self, node);
    }
}
