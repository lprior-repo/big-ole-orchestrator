use crate::{diagnostic::Diagnostic, rules::placeholder::check_placeholder_tests, Rule};
use syn::File;

pub struct PlaceholderRule;

impl Rule for PlaceholderRule {
    fn id(&self) -> &'static str {
        "L004"
    }

    fn name(&self) -> &'static str {
        "placeholder test (todo!, unimplemented!, assert!(true), #[ignore], constant-only assertion)"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        check_placeholder_tests(file)
    }
}
