use wtf_linter::lint_workflow_code;
use wtf_linter::LintCode;

#[test]
fn test_lints_clean_code_with_no_violations() {
    let source = r#"
async fn helper() {
    let x = 42;
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_returns_parse_error_for_invalid_rust() {
    let source = "not valid rust {";
    let result = lint_workflow_code(source);
    assert!(result.is_err());
}

#[test]
fn test_handles_empty_source() {
    let source = "";
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_no_false_positive_outside_workflow() {
    let source = r#"
async fn helper_function() {
    tokio::spawn(async {
        println!("helper task");
    });
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_no_false_positive_different_spawn() {
    let source = r#"
impl MyTrait for MyWorkflow {
    async fn execute(&self) {
        let handle = std::thread::spawn(|| {
            println!("thread");
        });
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_violation_tokio_spawn_in_workflow() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        tokio::spawn(async {
            println!("detached task");
        });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L005);
}

#[test]
fn test_violation_nested_tokio_spawn() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = if true {
            tokio::spawn(async {
                do_work().await;
            })
        } else {
            42
        };
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L005);
}

#[test]
fn test_multiple_tokio_spawns() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        tokio::spawn(async { println!("first"); });
        tokio::spawn(async { println!("second"); });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn test_tokio_spawn_in_closure() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let numbers = vec![1, 2, 3];
        numbers.iter().for_each(|_| {
            tokio::spawn(async { println!("in closure"); });
        });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn test_no_false_positive_qualified_tokio_spawn() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        some_other::spawn(async { });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}
