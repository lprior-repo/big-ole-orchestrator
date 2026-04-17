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

impl RandomDetector {}

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

    #[test]
    fn test_empty_file_no_diagnostics() {
        let src = "";
        let diags = parse_and_check(src);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_whitespace_only_no_diagnostics() {
        let src = "   \n\n   \n    ";
        let diags = parse_and_check(src);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_comments_only_no_diagnostics() {
        let src = "// just a comment\n/* block comment */";
        let diags = parse_and_check(src);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_deeply_nested_random_calls() {
        let src = quote! {
            fn workflow() {
                let id = deeply::nested::call::to::Uuid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_deeply_nested_rand_random() {
        let src = quote! {
            fn workflow() {
                let id = a::b::c::d::rand::random::<u32>();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_unicode_in_function_names_not_flagged() {
        let src = quote! {
            fn workflow() {
                let x = some_function();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_very_long_line_random() {
        let long_ident = "a".repeat(10000);
        let src = format!(
            "fn workflow() {{ let id = {}::Uuid::new_v4(); }}",
            long_ident
        );
        let diags = parse_and_check(&src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_ctx_random_u64_not_flagged() {
        let src = quote! {
            fn workflow() {
                let x = ctx.random_u64();
                let y = ctx.random_u32();
                let z = self.ctx.random_u128();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_new_v4_with_tuple_args_not_detected() {
        let src = quote! {
            fn workflow() {
                let ts = Uuid::new_v4((1, 2, 3));
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_new_v5_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v5();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_new_v6_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v6();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_new_v7_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v7();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_rand_random_vec_not_detected() {
        let src = quote! {
            fn workflow() {
                let vec = rand::random::<Vec<u8>>();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_rand_thread_rng_not_detected() {
        let src = quote! {
            fn workflow() {
                let mut rng = rand::thread_rng();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_rand_small_rng_not_detected() {
        let src = quote! {
            fn workflow() {
                let mut rng = rand::small_rng();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_workflow_functions_each_detected() {
        let src = quote! {
            fn workflow_a() {
                let id1 = Uuid::new_v4();
                let id2 = Uuid::new_v4();
            }
            fn workflow_b() {
                let val1 = rand::random::<u32>();
                let val2 = rand::random::<u64>();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 4);
    }

    #[test]
    fn test_random_in_closure() {
        let src = quote! {
            fn workflow() {
                let f = || {
                    let id = Uuid::new_v4();
                };
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_async_block() {
        let src = quote! {
            fn workflow() {
                async {
                    let id = Uuid::new_v4();
                };
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_for_loop() {
        let src = quote! {
            fn workflow() {
                for i in items {
                    let id = Uuid::new_v4();
                }
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_while_loop() {
        let src = quote! {
            fn workflow() {
                while condition() {
                    let id = Uuid::new_v4();
                }
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_struct_init() {
        let src = quote! {
            fn workflow() {
                let s = SomeStruct {
                    id: Uuid::new_v4(),
                    name: "test",
                };
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_method_chain() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v4().to_string();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_array_literal() {
        let src = quote! {
            fn workflow() {
                let arr = [Uuid::new_v4(), Uuid::new_v4()];
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_random_in_return_stmt() {
        let src = quote! {
            fn workflow() -> Uuid {
                return Uuid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_random_in_macro_call_not_expanded() {
        let src = quote! {
            fn workflow() {
                let id = some_macro!(Uuid::new_v4());
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_rand_uppercase_not_detected() {
        let src = quote! {
            fn workflow() {
                let x = RAND::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_uuid_uppercase_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = UUID::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_mixed_case_rand_Rand_not_detected() {
        let src = quote! {
            fn workflow() {
                let x = rAnd::random();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }

    #[test]
    fn test_mixed_case_uuid_Uuid_not_detected() {
        let src = quote! {
            fn workflow() {
                let id = uUid::new_v4();
            }
        };
        let diags = parse_and_check(&src.to_string());
        assert!(diags.is_empty());
    }
}
