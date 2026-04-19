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
    if !call.args.is_empty() {
        return false;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    fn parse_and_check(src: &str) -> Vec<Diagnostic> {
        let file: File = syn::parse_str(src).unwrap();
        check_random_in_workflow(&file)
    }

    #[test]
    fn test_uuid_new_v4_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message().contains("non-deterministic"));
    }

    #[test]
    fn test_rand_random_detected() {
        let src = quote! {
            fn workflow() {
                let value: u32 = rand::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message().contains("non-deterministic"));
    }

    #[test]
    fn test_multiple_randoms_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v4();
                let value: u32 = rand::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_no_random_no_diagnostics() {
        let src = quote! {
            fn workflow() {
                let id = ctx.random_u64();
                let value = some_deterministic_fn();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_new_v1_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v1();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_other_uuid_v4_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = MyUuid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_other_rand_not_detected() {
        let src = quote! {
            fn workflow() {
                let value: u32 = MyRand::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_nested_random_calls() {
        let src = quote! {
            fn workflow() {
                let id = some_fn(Uuid::new_v4());
                let val = another_fn(rand::random(), ctx.random_u64());
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_random_in_if_expr() {
        let src = quote! {
            fn workflow() {
                if Uuid::new_v4() == some_id {
                    do_something();
                }
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_match_arm() {
        let src = quote! {
            fn workflow() {
                match rand::random::<u32>() {
                    0 => do_zero(),
                    _ => do_other(),
                }
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_multiple_workflow_functions() {
        let src = quote! {
            fn workflow1() {
                let id = Uuid::new_v4();
            }

            fn workflow2() {
                let id = Uuid::new_v4();
                let val = rand::random::<u64>();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 3);
    }

    #[test]
    fn test_rand_random_with_type_annotation() {
        let src = quote! {
            fn workflow() {
                let x: u64 = rand::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_rand_random_generic() {
        let src = quote! {
            fn workflow() {
                let x = rand::random::<u64>();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_uuid_new_v4_with_args_not_detected() {
        let src = quote! {
            fn workflow() {
                let ts = Uuid::new_v4(&mut bytes);
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_qualifed_uuid_new_v4() {
        let src = quote! {
            fn workflow() {
                use uuid::Uuid;
                let id = Uuid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }
}
