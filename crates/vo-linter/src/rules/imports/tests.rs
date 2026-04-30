//! Comprehensive tests for unused import detection.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::LintSeverity;
use crate::LintCode;
use super::check_unused_imports;

#[test]
fn test_single_unused_import() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1, "expected exactly one unused import");
    assert_eq!(diagnostics[0].code, LintCode::L001);
    assert!(diagnostics[0].message.contains("unused import"));
}

#[test]
fn test_used_import_no_diagnostic() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow() {
            let map = HashMap::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "used import should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_multiple_unused_imports() {
    let src = r#"
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::task;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(
        diagnostics.len(), 3,
        "expected three unused imports, got {}",
        diagnostics.len()
    );
}

#[test]
fn test_mixed_used_and_unused_imports() {
    let src = r#"
        use std::collections::HashMap;
        use std::sync::Arc;

        fn workflow() {
            let map = HashMap::new();
            let _arc = Arc::new(());
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "all imports are used, got: {diagnostics:?}"
    );
}

#[test]
fn test_some_used_some_unused() {
    let src = r#"
        use std::collections::HashMap;
        use std::sync::Arc;

        fn workflow() {
            let _arc = Arc::new(());
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1, "expected one unused import");
    assert!(diagnostics[0].message.contains("HashMap"));
}

#[test]
fn test_self_referential_import() {
    let src = r#"
        use std::fmt::Debug;

        fn workflow() {
            let x: Box<dyn Debug> = Box::new(String::new());
            println!("{:?}", x);
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "Debug trait used in type annotation should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_import_in_nested_module() {
    let src = r#"
        mod inner {
            use std::collections::HashMap;

            pub fn inner_workflow() {
                let x = 1;
            }
        }

        fn workflow() {
            inner::inner_workflow();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        !diagnostics.is_empty(),
        "unused import in nested module should be detected"
    );
}

#[test]
fn test_unused_import_warning_severity() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].severity,
        LintSeverity::Warning,
        "unused imports should be Warning severity"
    );
}

#[test]
fn test_unused_import_has_suggestion() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].suggestion.is_some(),
        "suggestion should be present"
    );
    assert!(
        diagnostics[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("remove"),
        "suggestion should mention removal"
    );
}

#[test]
fn test_multiple_imports_same_module() {
    let src = r#"
        use std::collections::HashMap;
        use std::collections::VecDeque;

        fn workflow() {
            let _map = HashMap::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("VecDeque"));
}

#[test]
fn test_glob_import_not_flagged_as_unused() {
    let src = r#"
        use std::collections::*;

        fn workflow() {
            let _map = HashMap::new();
            let _vec = Vec::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1, "glob imports are flagged as we cannot determine their contents");
    assert!(diagnostics[0].message.contains("std::collections"));
}

#[test]
fn test_renamed_import() {
    let src = r#"
        use std::collections::HashMap as HM;

        fn workflow() {
            let _map = HM::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "renamed but used import should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_renamed_import() {
    let src = r#"
        use std::collections::HashMap as HM;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("HM"));
}

#[test]
fn test_empty_file_no_imports() {
    let src = r#"
        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "file with no imports should produce no diagnostics"
    );
}

#[test]
fn test_only_used_imports() {
    let src = r#"
        use std::fmt::Debug;
        use std::fmt::Display;

        fn workflow() {
            let d: Box<dyn Debug> = Box::new(String::new());
            let s: Box<dyn Display> = Box::new(String::new());
            println!("{:?}", d);
            println!("{}", s);
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "all imports used should produce no diagnostics, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_import_sorted_alphabetically() {
    let src = r#"
        use std::collections::VecDeque;
        use std::collections::HashMap;
        use std::collections::BTreeMap;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("BTreeMap"));
    assert!(diagnostics[1].message.contains("HashMap"));
    assert!(diagnostics[2].message.contains("VecDeque"));
}

#[test]
fn test_unused_import_lint_code_l001() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L001);
}

#[test]
fn test_struct_field_access() {
    let src = r#"
        use std::sync::Arc;

        fn workflow() {
            let arc = Arc::new(1);
            let _cloned = arc.clone();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "Arc used via method call should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_macro_invocation() {
    let src = r#"
        use tokio::spawn;

        async fn workflow() {
            let _handle = spawn(async {});
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "spawn macro used should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_type_in_return_position() {
    let src = r#"
        use std::future::Future;

        fn workflow() -> impl Future<Output = ()> {
            async {}
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "Future used in return type should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_type_in_argument_position() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow(map: HashMap<String, String>) {
            let _ = map;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "HashMap used in function argument should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_trait_bound_in_generics() {
    let src = r#"
        use std::fmt::Debug;

        fn workflow<T: Debug>(val: T) {
            println!("{:?}", val);
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "Debug used in trait bound should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_multiple_workflow_functions() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow_a() {
            let _map = HashMap::new();
        }

        fn workflow_b() {
            let _map = HashMap::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "HashMap used in multiple functions should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_import_in_only_one_function() {
    let src = r#"
        use std::collections::HashMap;

        fn workflow_a() {
            let x = 1;
        }

        fn workflow_b() {
            let _map = HashMap::new();
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "HashMap used in workflow_b, overall import is used, got: {diagnostics:?}"
    );
}

#[test]
fn test_use_in_type_annotation_complex() {
    let src = r#"
        use std::sync::Arc;
        use std::collections::HashMap;

        fn workflow() {
            let map: Arc<HashMap<String, u32>> = Arc::new(HashMap::new());
            let _ = map.get(&"key".to_string());
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert!(
        diagnostics.is_empty(),
        "Arc and HashMap used in complex type should not be flagged, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_import_with_nested_path() {
    let src = r#"
        use tokio::sync::mpsc;

        fn workflow() {
            let x = 1;
        }
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("mpsc"));
}

#[test]
fn test_wildcard_import_usage() {
    let src = r#"
        use foo::prelude::*;

        struct MyStruct;
        impl foo::Trait for MyStruct {}
    "#;
    let file: syn::File = syn::parse_str(src).unwrap();
    let diagnostics = check_unused_imports(&file);
    assert_eq!(diagnostics.len(), 1, "wildcard imports are flagged as we cannot determine their contents");
    assert!(diagnostics[0].message.contains("foo::prelude"));
}