use super::*;
use quote::quote;

fn parse_src(src: &str) -> syn::File {
    syn::parse_str(src).unwrap()
}

#[test]
fn detector_finds_assert_true_in_test() {
    let src = r#"
        #[test]
        fn test_something() {
            let result = 42;
            assert!(true);
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("trivial assertion")),
        "should detect assert!(true), got: {diags:#?}");
}

#[test]
fn detector_finds_assert_eq_true_in_test() {
    let src = r#"
        #[test]
        fn test_value() {
            assert_eq!(42, true);
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("trivial assertion")),
        "should detect assert_eq with true, got: {diags:#?}");
}

#[test]
fn detector_finds_ignore_attr() {
    let src = r#"
        #[test]
        #[ignore]
        fn test_skipped() {
            assert_eq!(1, 1);
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("ignored test")),
        "should detect #[ignore], got: {diags:#?}");
}

#[test]
fn detector_finds_todo_in_test() {
    let src = r#"
        #[test]
        fn test_incomplete() {
            todo!();
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("todo")),
        "should detect todo!(), got: {diags:#?}");
}

#[test]
fn detector_finds_commented_out_handler() {
    let src = r#"
        mod tests {
            // fn on_workflow_started_handler(ctx: Context) { }
            #[test]
            fn test_actual() {
                assert_eq!(1, 1);
            }
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("commented-out handler")),
        "should detect commented-out handler, got: {diags:#?}");
}

#[test]
fn detector_finds_commented_out_test() {
    let src = r#"
        #[cfg(test)]
        mod tests {
            // fn test_old_behavior() { }
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("commented-out test")),
        "should detect commented-out test, got: {diags:#?}");
}

#[test]
fn detector_finds_commented_out_tokio_test() {
    let src = r#"
        mod tests {
            // #[tokio::test]
            // async fn test_async_handler() { }
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().any(|d| d.message.contains("commented-out")),
        "should detect commented-out tokio::test, got: {diags:#?}");
}

#[test]
fn detector_passes_clean_test() {
    let src = r#"
        #[test]
        fn test_proper() {
            let result = compute_value();
            assert_eq!(result, expected());
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    assert!(diags.iter().all(|d| !d.message.contains("trivial assertion")
        && !d.message.contains("ignored")
        && !d.message.contains("todo")),
        "should have no placeholder issues, got: {diags:#?}");
}

#[test]
fn detector_ignores_assert_true_outside_test() {
    let src = r#"
        fn compute_value() -> bool {
            assert!(true);
            true
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    let assert_diag = diags.iter().filter(|d| d.message.contains("trivial assertion")).count();
    assert_eq!(assert_diag, 0,
        "should not flag assert!(true) outside test functions, got: {diags:#?}");
}

#[test]
fn detector_finds_multiple_issues() {
    let src = r#"
        #[test]
        #[ignore]
        fn test_multiple() {
            assert!(true);
            todo!();
        }
    "#;
    let file = parse_src(src);
    let diags = check_placeholder_tests(&file, src);
    let ignored_count = diags.iter().filter(|d| d.message.contains("ignored test")).count();
    let todo_count = diags.iter().filter(|d| d.message.contains("todo")).count();
    let trivial_count = diags.iter().filter(|d| d.message.contains("trivial assertion")).count();
    assert_eq!(ignored_count, 1, "should detect ignored test");
    assert_eq!(todo_count, 1, "should detect todo!");
    assert_eq!(trivial_count, 1, "should detect trivial assertion");
}
