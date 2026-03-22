use wtf_linter::lint_workflow_code_l006 as lint_workflow_code;
use wtf_linter::LintCode;

#[test]
fn test_emits_no_diagnostic_for_code_without_thread_spawn() {
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
    std::thread::spawn(|| {
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
        tokio::spawn(async {
            println!("tokio spawn");
        });
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_violation_std_thread_spawn_in_workflow() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        std::thread::spawn(|| {
            println!("detached thread");
        });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L006);
}

#[test]
fn test_violation_nested_std_thread_spawn_in_closure() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = if true {
            std::thread::spawn(|| {
                do_work();
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
    assert_eq!(diagnostics[0].code, LintCode::L006);
}

#[test]
fn test_violation_std_thread_spawn_in_if_branch() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        if true {
            std::thread::spawn(|| {
                do_work();
            });
        }
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L006);
}

#[test]
fn test_multiple_std_thread_spawns_in_same_workflow() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        std::thread::spawn(|| { println!("first"); });
        std::thread::spawn(|| { println!("second"); });
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
fn test_std_thread_spawn_in_closure_within_workflow() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let numbers = vec![1, 2, 3];
        numbers.iter().for_each(|_| {
            std::thread::spawn(|| { println!("in closure"); });
        });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    let diagnostics = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L006);
}

#[test]
fn test_no_false_positive_tokio_spawn() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        tokio::spawn(async { });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_diagnostic_contains_correct_lint_code() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = std::thread::spawn(|| {});
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source).expect("should parse");
    assert!(!result.is_empty());
    assert_eq!(result[0].code, LintCode::L006);
}

#[test]
fn test_diagnostic_contains_suggestion() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = std::thread::spawn(|| {});
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source).expect("should parse");
    assert!(!result.is_empty());
    assert!(
        result[0].suggestion.is_some(),
        "diagnostic should have suggestion"
    );
}

#[test]
fn test_no_false_positive_qualified_thread_spawn() {
    let source = r#"
impl WorkflowFn for MyWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        some_other::thread::spawn(|| { });
        Ok(())
    }
}
"#;
    let result = lint_workflow_code(source);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}
