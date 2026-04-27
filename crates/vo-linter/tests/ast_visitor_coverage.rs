#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

//! AST Visitor Coverage Tests for vo-linter.
//!
//! Verifies that the syn-based AST traversal covers workflow definition nodes.

use quote::quote;
use syn::{parse_str, visit::Visit};

// ─────────────────────────────────────────────────────────────────────────────
// Visitor Coverage Tests - File-level Items
// ─────────────────────────────────────────────────────────────────────────────

mod file_level_items {
    use super::*;

    #[test]
    fn visitor_handles_use_declarations() {
        let src = quote! {
            use std::collections::HashMap;
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.use_item_count > 0);
    }

    #[test]
    fn visitor_handles_struct_definitions() {
        let src = quote! {
            struct WorkflowConfig { name: String }
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.struct_item_count > 0);
    }

    #[test]
    fn visitor_handles_enum_definitions() {
        let src = quote! {
            enum WorkflowState { Pending, Running }
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.enum_item_count > 0);
    }

    #[test]
    fn visitor_handles_function_items() {
        let src = quote! {
            fn workflow() { let x = 1; }
            fn other() { let y = 2; }
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert_eq!(visitor.function_item_count, 2);
    }

    #[test]
    fn visitor_handles_impl_blocks() {
        let src = quote! {
            impl Workflow for MyWorkflow { fn execute(&self) {} }
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.impl_item_count > 0);
    }

    #[test]
    fn visitor_handles_trait_definitions() {
        let src = quote! {
            trait WorkflowTrait { fn execute(&self); }
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.trait_item_count > 0);
    }

    #[test]
    fn visitor_handles_mod_items() {
        let src = quote! {
            mod inner { fn helper() {} }
            fn workflow() {}
        };
        let file: syn::File = parse_str(&src.to_string()).unwrap();
        let mut visitor = TestVisitor::default();
        visitor.visit_file(&file);
        assert!(visitor.mod_item_count > 0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Visitor Implementation
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct TestVisitor {
    use_item_count: usize,
    struct_item_count: usize,
    enum_item_count: usize,
    trait_item_count: usize,
    impl_item_count: usize,
    function_item_count: usize,
    mod_item_count: usize,
    expr_call_count: usize,
    expr_path_count: usize,
    expr_literal_count: usize,
    expr_block_count: usize,
    expr_if_count: usize,
    expr_method_call_count: usize,
}

impl<'ast> Visit<'ast> for TestVisitor {
    fn visit_item_use(&mut self, _node: &'ast syn::ItemUse) {
        self.use_item_count += 1;
    }
    fn visit_item_struct(&mut self, _node: &'ast syn::ItemStruct) {
        self.struct_item_count += 1;
    }
    fn visit_item_enum(&mut self, _node: &'ast syn::ItemEnum) {
        self.enum_item_count += 1;
    }
    fn visit_item_trait(&mut self, _node: &'ast syn::ItemTrait) {
        self.trait_item_count += 1;
    }
    fn visit_item_impl(&mut self, _node: &'ast syn::ItemImpl) {
        self.impl_item_count += 1;
    }
    fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {
        self.function_item_count += 1;
    }
    fn visit_item_mod(&mut self, _node: &'ast syn::ItemMod) {
        self.mod_item_count += 1;
    }
    fn visit_expr_call(&mut self, _node: &'ast syn::ExprCall) {
        self.expr_call_count += 1;
    }
    fn visit_expr_path(&mut self, _node: &'ast syn::ExprPath) {
        self.expr_path_count += 1;
    }
    fn visit_expr_lit(&mut self, _node: &'ast syn::ExprLit) {
        self.expr_literal_count += 1;
    }
    fn visit_expr_block(&mut self, _node: &'ast syn::ExprBlock) {
        self.expr_block_count += 1;
    }
    fn visit_expr_if(&mut self, _node: &'ast syn::ExprIf) {
        self.expr_if_count += 1;
    }
    fn visit_expr_method_call(&mut self, _node: &'ast syn::ExprMethodCall) {
        self.expr_method_call_count += 1;
    }
}
