#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode, LintError};
use syn::{Block, Expr, Item, ItemImpl, Stmt};

/// Lints workflow code for disallowed thread operations.
///
/// # Errors
/// Returns `LintError::ParseError` if the source cannot be parsed.
pub fn lint_workflow_code(source: &str) -> Result<Vec<Diagnostic>, LintError> {
    let syntax_tree = syn::parse_file(source).map_err(|e| LintError::ParseError(e.to_string()))?;
    let mut collector = L006Visitor::new();
    collector.visit_items(&syntax_tree.items);
    Ok(collector.diagnostics)
}

struct L006Visitor {
    diagnostics: Vec<Diagnostic>,
    in_workflow_fn: bool,
}

impl L006Visitor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            in_workflow_fn: false,
        }
    }

    fn visit_items(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Impl(impl_item) = item {
                self.process_impl_item(impl_item);
            }
        }
    }

    fn process_impl_item(&mut self, impl_item: &ItemImpl) {
        let is_wf = is_workflow_impl(impl_item);
        let was_in_wf = self.in_workflow_fn;
        self.in_workflow_fn = is_wf;
        for impl_item in &impl_item.items {
            if let syn::ImplItem::Fn(impl_fn) = impl_item {
                if impl_fn.sig.ident == "execute" && impl_fn.sig.asyncness.is_some() {
                    self.visit_block(&impl_fn.block);
                }
            }
        }
        self.in_workflow_fn = was_in_wf;
    }

    #[allow(clippy::too_many_lines)]
    fn visit_expr(&mut self, expr: &Expr) {
        if self.in_workflow_fn {
            if let Expr::Call(call) = expr {
                if let Expr::Path(path_expr) = &*call.func {
                    if is_std_thread_spawn_path(&path_expr.path) {
                        self.diagnostics.push(Diagnostic::new(
                            LintCode::L006,
                            "std::thread::spawn() is not allowed inside a procedural workflow function. \
                             Native threads detach from the workflow context and violate determinism.",
                        )
                        .with_suggestion(
                            "Use workflow context's spawn method or convert to a child activity instead.",
                        ));
                    } else if is_std_thread_sleep_path(&path_expr.path) {
                        self.diagnostics.push(Diagnostic::new(
                            LintCode::L006b,
                            "std::thread::sleep() is not allowed inside a procedural workflow function. \
                             Use ctx.sleep() for deterministic delays.",
                        )
                        .with_suggestion("Use ctx.sleep() instead for deterministic replay."));
                    }
                }
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
        }
        match expr {
            Expr::Async(async_expr) => self.visit_block(&async_expr.block),
            Expr::Block(block) => self.visit_block(&block.block),
            Expr::If(if_expr) => {
                self.visit_expr(&if_expr.cond);
                self.visit_block(&if_expr.then_branch);
                if let Some((_, else_branch)) = &if_expr.else_branch {
                    self.visit_expr(else_branch);
                }
            }
            Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
                    self.visit_expr(&arm.body);
                }
            }
            Expr::Loop(loop_expr) => self.visit_block(&loop_expr.body),
            Expr::ForLoop(for_expr) => self.visit_block(&for_expr.body),
            Expr::While(while_expr) => self.visit_block(&while_expr.body),
            Expr::Closure(closure_expr) => self.visit_expr(&closure_expr.body),
            Expr::Return(ret_expr) => {
                if let Some(expr) = &ret_expr.expr {
                    self.visit_expr(expr);
                }
            }
            Expr::Let(let_expr) => self.visit_expr(&let_expr.expr),
            Expr::Assign(assign_expr) => {
                self.visit_expr(&assign_expr.left);
                self.visit_expr(&assign_expr.right);
            }
            Expr::MethodCall(method_call) => {
                for arg in &method_call.args {
                    self.visit_expr(arg);
                }
            }
            Expr::Tuple(tuple_expr) => {
                for elem in &tuple_expr.elems {
                    self.visit_expr(elem);
                }
            }
            Expr::Array(array_expr) => {
                for elem in &array_expr.elems {
                    self.visit_expr(elem);
                }
            }
            Expr::Cast(cast_expr) => self.visit_expr(&cast_expr.expr),
            Expr::Unary(unary_expr) => self.visit_expr(&unary_expr.expr),
            Expr::Binary(binary_expr) => {
                self.visit_expr(&binary_expr.left);
                self.visit_expr(&binary_expr.right);
            }
            Expr::Break(break_expr) => {
                if let Some(expr) = &break_expr.expr {
                    self.visit_expr(expr);
                }
            }
            Expr::Reference(reference_expr) => self.visit_expr(&reference_expr.expr),
            Expr::Field(field_expr) => self.visit_expr(&field_expr.base),
            Expr::Index(index_expr) => {
                self.visit_expr(&index_expr.expr);
                self.visit_expr(&index_expr.index);
            }
            Expr::Range(range_expr) => {
                if let Some(start) = &range_expr.start {
                    self.visit_expr(start);
                }
                if let Some(end) = &range_expr.end {
                    self.visit_expr(end);
                }
            }
            Expr::Paren(paren_expr) => self.visit_expr(&paren_expr.expr),
            Expr::Try(try_expr) => self.visit_expr(&try_expr.expr),
            Expr::TryBlock(try_block) => self.visit_block(&try_block.block),
            Expr::Yield(yield_expr) => {
                if let Some(expr) = &yield_expr.expr {
                    self.visit_expr(expr);
                }
            }
            Expr::Struct(_) | Expr::Repeat(_) | Expr::Group(_) | Expr::Await(_) | _ => {}
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.visit_expr(&init.expr);
                }
            }
            Stmt::Expr(expr, _) => self.visit_expr(expr),
            Stmt::Item(_) | Stmt::Macro(_) => {}
        }
    }
}

fn is_workflow_impl(impl_item: &ItemImpl) -> bool {
    impl_item
        .items
        .iter()
        .any(|item| matches!(item, syn::ImplItem::Fn(impl_fn) if impl_fn.sig.ident == "execute" && impl_fn.sig.asyncness.is_some()))
}

fn is_std_thread_spawn_path(path: &syn::Path) -> bool {
    path.segments.len() == 3
        && path.segments[0].ident == "std"
        && path.segments[1].ident == "thread"
        && path.segments[2].ident == "spawn"
}

fn is_std_thread_sleep_path(path: &syn::Path) -> bool {
    path.segments.len() == 3
        && path.segments[0].ident == "std"
        && path.segments[1].ident == "thread"
        && path.segments[2].ident == "sleep"
}

#[cfg(test)]
mod tests {
    use super::lint_workflow_code;
    use crate::diagnostic::LintCode;

    #[test]
    fn emits_no_diagnostic_for_code_without_thread_spawn() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        let _x = 1 + 1;
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert!(diags.is_empty());
        }
    }

    #[test]
    fn emits_diagnostic_for_std_thread_spawn_in_workflow_execute() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        std::thread::spawn(|| {});
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0].code, LintCode::L006);
            assert!(diags[0]
                .suggestion
                .as_ref()
                .is_some_and(|message| message.contains("activity")));
        }
    }

    #[test]
    fn emits_no_diagnostic_for_spawn_in_non_workflow_fn() {
        let source = r"
async fn helper() {
    std::thread::spawn(|| {});
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert!(diags.is_empty());
        }
    }

    #[test]
    fn emits_multiple_diagnostics_for_multiple_spawn_calls() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        std::thread::spawn(|| {});
        std::thread::spawn(|| {});
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert_eq!(diags.len(), 2);
            assert!(diags.iter().all(|diag| diag.code == LintCode::L006));
        }
    }

    #[test]
    fn returns_parse_error_for_invalid_rust() {
        let source = "impl Workflow for MyWf { async fn execute(&self) {";
        let result = lint_workflow_code(source);
        assert!(result.is_err());
    }

    #[test]
    fn emits_l006b_for_thread_sleep_in_workflow() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert!(diags.iter().any(|diag| diag.code == LintCode::L006b));
        }
    }

    #[test]
    fn does_not_emit_for_ctx_sleep_call() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        ctx.sleep(std::time::Duration::from_millis(1));
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            assert!(diags.is_empty());
        }
    }

    #[test]
    fn emits_multiple_for_nested_spawn_calls() {
        let source = r"
impl Workflow for MyWf {
    async fn execute(&self) {
        if true {
            std::thread::spawn(|| {});
            std::thread::spawn(|| {});
        }
    }
}
";

        let result = lint_workflow_code(source);
        assert!(result.is_ok());
        if let Ok(diags) = result {
            let l006_count = diags
                .iter()
                .filter(|diag| diag.code == LintCode::L006)
                .count();
            assert_eq!(l006_count, 2);
        }
    }
}
