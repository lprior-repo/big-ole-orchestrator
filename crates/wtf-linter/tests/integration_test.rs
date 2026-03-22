use wtf_linter::{lint_workflow_source, LintCode, Severity};

#[test]
fn test_integration_l006_l006b_violations() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        // L006: std::thread::spawn
        std::thread::spawn(|| {});

        // L006b: std::thread::sleep
        std::thread::sleep(std::time::Duration::from_secs(1));

        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source);
    assert!(
        result.is_ok(),
        "lint_workflow_source should not return parse error"
    );
    let lint_result = result.unwrap();
    let diagnostics = lint_result.diagnostics;

    assert!(
        lint_result.has_errors,
        "Should detect violations, has_errors should be true"
    );

    let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();

    assert!(
        codes.contains(&LintCode::L006),
        "Should detect L006 std::thread::spawn, got codes: {:?}",
        codes
    );
    assert!(
        codes.contains(&LintCode::L006b),
        "Should detect L006b std::thread::sleep, got codes: {:?}",
        codes
    );

    assert_eq!(
        codes.len(),
        2,
        "Should produce exactly 2 diagnostics for L006+L006b, got {}: {:?}",
        codes.len(),
        codes
    );
}

#[test]
fn test_lint_result_has_errors() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        std::thread::spawn(|| {});
        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source).unwrap();
    assert!(result.has_errors);
    assert_eq!(result.diagnostics.len(), 1);
}

#[test]
fn test_lint_result_no_errors() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = 42;
        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source).unwrap();
    assert!(!result.has_errors);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_thread_sleep_detection() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        std::thread::sleep(std::time::Duration::from_millis(100));
        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source).unwrap();
    let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&LintCode::L006b),
        "Should detect L006b std::thread::sleep, got: {:?}",
        codes
    );
}

#[test]
fn test_thread_spawn_and_sleep_together() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        std::thread::spawn(|| {});
        std::thread::sleep(std::time::Duration::from_secs(1));
        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source).unwrap();
    let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&LintCode::L006));
    assert!(codes.contains(&LintCode::L006b));
}

#[test]
fn test_no_false_positive_ctx_sleep() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        ctx.sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}
"#;
    let result = lint_workflow_source(source).unwrap();
    assert!(!result.has_errors, "ctx.sleep should not trigger L006b");
}

#[test]
fn test_no_false_positive_thread_outside_workflow() {
    let source = r#"
async fn helper() {
    std::thread::spawn(|| {});
    std::thread::sleep(std::time::Duration::from_secs(1));
}
"#;
    let result = lint_workflow_source(source).unwrap();
    assert!(
        !result.has_errors,
        "thread::spawn and thread::sleep outside workflow should not be flagged"
    );
}

#[test]
fn test_warning_only_diagnostics_do_not_set_has_errors() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let xs = vec![1,2,3];
        xs.iter().for_each(|_| {
            let _ = ctx.now();
        });
        Ok(())
    }
}
"#;

    let result = lint_workflow_source(source).unwrap_or_else(|_| unreachable!());
    assert!(
        !result.has_errors,
        "warning-only results should not be errorful"
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity == Severity::Warning));
}
