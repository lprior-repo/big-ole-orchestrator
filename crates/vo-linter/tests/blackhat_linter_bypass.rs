//! BLACK-HAT adversarial bypass tests for vo-linter.
//!
//! Each test documents a known bypass vector. Tests that FAIL reveal gaps
//! where the linter should catch evasion but currently doesn't.

#![allow(clippy::unwrap_used)]

use quote::quote;
use syn::{parse_str, File};
use vo_linter::rules::check_random_in_workflow;

fn parse_and_check(src: &str) -> usize {
    let file: File = parse_str(src).expect("parse must succeed");
    check_random_in_workflow(&file).len()
}

#[test]
fn bypass_type_alias_uuid() {
    let src = quote! {
        type MyId = uuid::Uuid;
        fn workflow() { let id = MyId::new_v4(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "FIXED: type alias MyId now resolves to uuid::Uuid, new_v4 detected"
    );
}

#[test]
fn bypass_use_renamed_import() {
    let src = quote! {
        use uuid::Uuid as GenId;
        fn workflow() { let id = GenId::new_v4(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "FIXED: `use X as Y` renames now resolved, linter detects GenId::new_v4()"
    );
}

#[test]
fn bypass_macro_wrapping_random() {
    let src = quote! {
        fn workflow() { let id = some_macro!(Uuid::new_v4()); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        0,
        "CONFIRMED BYPASS: macro arguments are not expanded by syn"
    );
}

#[test]
fn bypass_uppercase_uuid() {
    let src = quote! {
        fn workflow() { let id = UUID::new_v4(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        0,
        "CONFIRMED BYPASS: case-sensitive match misses UUID"
    );
}

#[test]
fn bypass_thread_rng_gen() {
    let src = quote! {
        fn workflow() { let mut rng = rand::thread_rng(); let x = rng.gen::<u64>(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "FIXED: rand::thread_rng() is now detected as non-deterministic (rng.gen() via local var still not caught)"
    );
}

#[test]
fn bypass_os_random() {
    let src = quote! {
        fn workflow() { let key = rand::rngs::OsRng.next_u64(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        0,
        "CONFIRMED BYPASS: OsRng provides non-deterministic bytes undetected"
    );
}

#[test]
fn bypass_random_via_helper() {
    let src = quote! {
        fn workflow() { let id = get_random_id(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        0,
        "FALSE NEGATIVE: helper function may contain random but is not inlined"
    );
}

#[test]
fn edge_random_in_raw_string() {
    let src = r#"fn workflow() { let s = "Uuid::new_v4()"; }"#;
    assert_eq!(
        parse_and_check(src),
        0,
        "string literals containing linter patterns must not be flagged"
    );
}

#[test]
fn positive_fully_qualified_uuid() {
    let src = quote! {
        fn workflow() { let id = uuid::Uuid::new_v4(); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "SHOULD DETECT: fully qualified uuid::Uuid::new_v4()"
    );
}

#[test]
fn positive_random_in_let_else() {
    let src = quote! {
        fn workflow() {
            let Some(id) = Some(Uuid::new_v4()) else { return; };
        }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "SHOULD DETECT: random inside let-else expression"
    );
}

#[test]
fn positive_random_in_try_context() {
    let file: File = parse_str(
        &quote! {
            fn workflow() { let id = some_fallible(Uuid::new_v4())?; }
        }
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        check_random_in_workflow(&file).len(),
        1,
        "SHOULD DETECT: random inside ? operator context"
    );
}

#[test]
fn positive_random_in_closure_arg() {
    let src = quote! {
        fn workflow() { items.iter().map(|_| Uuid::new_v4()); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "SHOULD DETECT: random inside closure passed to iterator adapter"
    );
}

#[test]
fn positive_random_in_match_guard() {
    let src = quote! {
        fn workflow() {
            match val { x if x > rand::random() => {} _ => {} }
        }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        1,
        "SHOULD DETECT: random inside match guard"
    );
}

#[test]
fn positive_double_random_in_tuple() {
    let src = quote! {
        fn workflow() { let (a, b) = (Uuid::new_v4(), rand::random::<u32>()); }
    };
    assert_eq!(
        parse_and_check(&src.to_string()),
        2,
        "SHOULD DETECT: both randoms in tuple destructuring"
    );
}
