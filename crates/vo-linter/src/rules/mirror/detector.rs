#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use syn::{visit::Visit, File, Item, ItemEnum, ItemStruct};

#[must_use]
pub fn check_mirror_types_in_tests(file: &File) -> Vec<Diagnostic> {
    let mut detector = MirrorTypeDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}

const KNOWN_HANDLER_TYPES: &[&str] = &[
    "WorkflowSseEvent",
    "WorkflowWsEvent",
    "SseBroadcaster",
    "WsBroadcaster",
    "SseState",
    "WsConnectionCount",
];

#[derive(Default)]
struct MirrorTypeDetector {
    diagnostics: Vec<Diagnostic>,
}

impl MirrorTypeDetector {
    fn check_item_struct(&mut self, item: &ItemStruct) {
        let name = &item.ident.to_string();
        if KNOWN_HANDLER_TYPES.contains(&name.as_str()) {
            self.diagnostics.push(
                Diagnostic::new(
                    LintCode::L003,
                    format!(
                        "test defines mirror type `{}` that duplicates production handler; \
                         use `vo_api::handlers::{}` from production instead",
                        name, name
                    ),
                )
                .with_suggestion(
                    "import from vo_api::handlers::<module> instead of defining locally",
                ),
            );
        }
    }

    fn check_item_enum(&mut self, item: &ItemEnum) {
        let name = &item.ident.to_string();
        if KNOWN_HANDLER_TYPES.contains(&name.as_str()) {
            self.diagnostics.push(
                Diagnostic::new(
                    LintCode::L003,
                    format!(
                        "test defines mirror type `{}` that duplicates production handler; \
                         use `vo_api::handlers::{}` from production instead",
                        name, name
                    ),
                )
                .with_suggestion(
                    "import from vo_api::handlers::<module> instead of defining locally",
                ),
            );
        }
    }
}

impl<'ast> Visit<'ast> for MirrorTypeDetector {
    fn visit_item(&mut self, node: &'ast Item) {
        match node {
            Item::Struct(s) => self.check_item_struct(s),
            Item::Enum(e) => self.check_item_enum(e),
            _ => {}
        }
        syn::visit::visit_item(self, node);
    }
}