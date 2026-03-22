use wtf_linter::lint_workflow_code_l004 as lint_workflow_code;
use wtf_linter::LintCode;

fn check(source: &str) -> Result<Vec<wtf_linter::Diagnostic>, wtf_linter::LintError> {
    lint_workflow_code(source)
}

#[test]
fn test_emits_diagnostic_for_map_with_ctx_activity() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_emits_diagnostic_for_for_each_with_ctx_sleep() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().for_each(|x| ctx.sleep(Duration::ZERO));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_emits_diagnostic_for_fold_with_ctx_random() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let _ = items.iter().fold(0u64, |acc, _| ctx.random_u64());
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_emits_diagnostic_for_filter_map_with_ctx_activity() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let _ = items.iter().filter_map(|x| Some(ctx.activity("test", x)));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_no_diagnostic_for_ctx_in_regular_for_loop() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        for item in items {
            ctx.activity("test", item);
        }
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result.is_empty());
}

#[test]
fn test_no_diagnostic_for_ctx_outside_closure() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let _ = ctx.random_u64();
        items.iter().map(|y| y + 1);
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result.is_empty());
}

#[test]
fn test_no_diagnostic_for_non_target_method() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let _ = items.iter().map(|y| y + 1);
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result.is_empty());
}

#[test]
fn test_emits_multiple_diagnostics_for_multiple_violations() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| ctx.activity("a", x));
        other.iter().for_each(|y| ctx.sleep(Duration::ZERO));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 2);
}

#[test]
fn test_emits_diagnostic_for_flat_map_with_ctx() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let _ = items.iter().flat_map(|x| ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_emits_diagnostic_for_nested_closure_with_ctx() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| {
            let inner = || ctx.activity("test", x);
            inner()
        });
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_parse_error_returns_err() {
    let source = "async fn workflow { // missing parentheses";
    let result = check(source);
    assert!(result.is_err());
}

#[test]
fn test_diagnostic_has_warning_severity() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result[0].severity, wtf_linter::Severity::Warning);
}

#[test]
fn test_diagnostic_has_suggestion() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result[0].suggestion.is_some());
}

#[test]
fn test_no_diagnostic_for_closure_without_ctx() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| x + 1);
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result.is_empty());
}

#[test]
fn test_emits_diagnostic_for_and_then_with_ctx() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().and_then(|x| ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_emits_diagnostic_for_map_with_ctx_field_access() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| x.ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, LintCode::L004);
}

#[test]
fn test_no_false_positive_for_named_variable_ctx() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        let local_ctx = ctx;
        items.iter().map(|x| local_ctx.activity("test", x));
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert!(result.is_empty());
}

#[test]
fn test_multiple_ctx_calls_in_same_closure() {
    let source = r#"
impl MyWorkflow for TestWorkflow {
    async fn execute(ctx: &Ctx) -> Result<(), Error> {
        items.iter().map(|x| {
            ctx.activity("a", x);
            ctx.sleep(Duration::ZERO);
            x
        });
        Ok(())
    }
}
"#;
    let result = check(source).expect("should parse");
    assert_eq!(result.len(), 1);
}
