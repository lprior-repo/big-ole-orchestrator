use super::{check_random_in_workflow, RandomRule};
use crate::diagnostic::Diagnostic;
use crate::Rule;
use quote::quote;
use syn::File;

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
fn test_mixed_case_rand_rand_not_detected() {
    let src = quote! {
        fn workflow() {
            let x = rAnd::random();
        }
    };
    let diags = parse_and_check(&src.to_string());
    assert!(diags.is_empty());
}

#[test]
fn test_mixed_case_uuid_uuid_not_detected() {
    let src = quote! {
        fn workflow() {
            let id = uUid::new_v4();
        }
    };
    let diags = parse_and_check(&src.to_string());
    assert!(diags.is_empty());
}

#[test]
fn test_random_rule_id() {
    let rule = RandomRule;
    assert_eq!(rule.id(), "L002");
}

#[test]
fn test_random_rule_name() {
    let rule = RandomRule;
    assert_eq!(rule.name(), "non-deterministic random call");
}

#[test]
fn test_random_rule_execute_empty() {
    let rule = RandomRule;
    let src = "";
    let file: File = syn::parse_str(src).unwrap();
    let diags = rule.execute(&file);
    assert!(diags.is_empty());
}

#[test]
fn test_random_rule_execute_finds_uuid() {
    let rule = RandomRule;
    let src = quote! { fn workflow() { let id = Uuid::new_v4(); } }.to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].message(),
        "non-deterministic random call in workflow function"
    );
}

#[test]
fn test_random_rule_execute_finds_rand() {
    let rule = RandomRule;
    let src = quote! { fn workflow() { let x = rand::random::<u32>(); } }.to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 1);
}

#[test]
fn test_random_rule_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RandomRule>();
}

#[test]
fn test_random_rule_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<RandomRule>();
}

#[test]
fn test_random_rule_execute_multiple_produces_multiple_diagnostics() {
    let rule = RandomRule;
    let src = quote! {
        fn workflow() {
            let id = Uuid::new_v4();
            let val = rand::random::<u64>();
            let id2 = Uuid::new_v4();
        }
    }
    .to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 3);
    for diag in &diags {
        assert!(diag.message().contains("non-deterministic"));
    }
}

#[test]
fn test_random_rule_execute_with_use_renamed_import() {
    let rule = RandomRule;
    let src = quote! {
        use uuid::Uuid as GenId;
        fn workflow() { let id = GenId::new_v4(); }
    }
    .to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 1);
}

#[test]
fn test_random_rule_execute_with_fully_qualified_path() {
    let rule = RandomRule;
    let src = quote! { fn workflow() { let id = uuid::Uuid::new_v4(); } }.to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 1);
}

#[test]
fn test_random_rule_execute_preserves_suggestion() {
    let rule = RandomRule;
    let src = quote! { fn workflow() { let id = Uuid::new_v4(); } }.to_string();
    let file: File = syn::parse_str(&src).unwrap();
    let diags = rule.execute(&file);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].suggestion().is_some());
    assert_eq!(
        diags[0].suggestion(),
        Some("use `ctx.random_u64()` instead")
    );
}
