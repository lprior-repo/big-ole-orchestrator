//! Rule implementation for detecting mirror types in API tests.

use crate::{diagnostic::Diagnostic, Rule};
use syn::File;

pub struct MirrorRule;

impl Rule for MirrorRule {
    fn id(&self) -> &'static str {
        "L003"
    }

    fn name(&self) -> &'static str {
        "local mirror type in API test"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        super::check_mirror_types_in_api_test(file)
    }
}