//! Procedural macros for the Veloxide SDK.
//!
//! This crate provides derive macros and attribute macros for the Veloxide SDK:
//!
//! # Macros
//!
//! - [`task_macro`] / `#[task]` - Generates executable entrypoints from functions
//!
//! # Example
//!
//! ```ignore
//! #[task]
//! fn my_task() {
//!     // task implementation
//! }
//! ```
//!
//! When applied to a function, the `#[task]` macro generates a `main()` function
//! that calls the annotated function, making it easy to compile workflow tasks
//! as standalone executables.

#![allow(dead_code, unused_variables)]

use proc_macro::TokenStream;

mod error;
mod task;

use task::{generate_task_entrypoint, parse_attributes, parse_task};

#[proc_macro_attribute]
pub fn task_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    internal_task_macro(attr.into(), item.into()).into()
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn internal_task_macro(
    attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if item.is_empty() {
        return quote::quote! { compile_error!("expected a function item"); };
    }

    if let Err(err) = parse_attributes(&attr) {
        return error_to_compile_error(&err);
    }

    let task_def = match parse_task(&item) {
        Ok(def) => def,
        Err(err) => return error_to_compile_error(&err),
    };

    match generate_task_entrypoint(&task_def) {
        Ok(main_fn) => {
            quote::quote! {
                #item
                #main_fn
            }
        }
        Err(err) => error_to_compile_error(&err),
    }
}

fn error_to_compile_error(err: &error::Error) -> proc_macro2::TokenStream {
    let msg = err.to_string();
    quote::quote! { compile_error!(#msg); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use quote::quote;

    #[test]
    fn task_macro_generates_synchronous_executable_main_wrapper() {
        let attr = quote! {};
        let item = quote! { fn my_task() {} };
        let expected = quote! { fn my_task() {} fn main() { my_task(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_generates_asynchronous_executable_main_wrapper() {
        let attr = quote! {};
        let item = quote! { async fn my_task() {} };
        let expected = quote! {
            async fn my_task() {}
            fn main() {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build current-thread runtime");
                rt.block_on(async { my_task().await; })
            }
        };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_dynamically_uses_annotated_function_identifier() {
        let attr = quote! {};
        let item = quote! { fn initialize_system() {} };
        let expected = quote! { fn initialize_system() {} fn main() { initialize_system(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_rejects_empty_token_stream_boundary() {
        let attr = quote! {};
        let item = quote! {};
        let expected = quote! { compile_error!("expected a function item"); };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_processes_empty_macro_attributes_boundary() {
        let attr = quote! {};
        let item = quote! { fn my_task() {} };
        let expected = quote! { fn my_task() {} fn main() { my_task(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_rejects_unsupported_macro_attributes() {
        let attr = quote! { retries = 3 };
        let item = quote! { fn my_task() {} };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(output.contains("unsupported attribute") && output.contains("retries"));
    }

    #[test]
    fn task_macro_emits_compile_error_for_non_function_items() {
        let attr = quote! {};
        let item = quote! { struct MyTask; };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(output.contains("invalid input item"));
    }

    #[test]
    fn task_macro_rejects_exactly_1_argument_boundary() {
        let attr = quote! {};
        let item = quote! { fn task(a: i32) {} };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(output.contains("unsupported signature"));
    }

    #[test]
    fn task_macro_handles_function_visibility_boundary() {
        let attr = quote! {};
        let item = quote! { pub fn my_task() {} };
        let expected = quote! { pub fn my_task() {} fn main() { my_task(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_handles_functions_with_complex_return_types() {
        let attr = quote! {};
        let item = quote! { fn my_task() -> Result<(), std::io::Error> { Ok(()) } };
        let expected = quote! { fn my_task() -> Result<(), std::io::Error> { Ok(()) } fn main() -> Result<(), std::io::Error> { my_task() } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_handles_minimum_valid_input() {
        let attr = quote! {};
        let item = quote! { fn a(){} };
        let expected = quote! { fn a(){} fn main() { a(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_handles_deeply_nested_blocks() {
        let attr = quote! {};
        let item = quote! { fn a() { { { {} } } } };
        let expected = quote! { fn a() { { { {} } } } fn main() { a(); } };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_rejects_exactly_one_attribute() {
        let attr = quote! { foo };
        let item = quote! { fn a() {} };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(output.contains("unsupported attribute") && output.contains("foo"));
    }

    #[test]
    fn task_macro_rejects_too_many_attributes() {
        let mut attrs = Vec::new();
        for _ in 0..256 {
            attrs.push(quote! { foo });
        }
        let attr = quote! { #(#attrs)* };
        let item = quote! { fn a() {} };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(output.contains("too many macro attributes") && output.contains("256"));
    }

    #[test]
    fn task_macro_generates_main_for_generic_sync_function() {
        let attr = quote! {};
        let item = quote! { fn generic_task<T: Default>() -> T { T::default() } };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(
            !output.contains("compile_error"),
            "should not emit compile_error: {}",
            output
        );
        assert!(
            output.contains("fn main"),
            "should generate main: {}",
            output
        );
    }

    #[test]
    fn task_macro_generates_main_for_generic_async_function() {
        let attr = quote! {};
        let item = quote! { async fn generic_task<T: Send>() where T: Default {} };
        let result = internal_task_macro(attr, item);
        let output = result.to_string();
        assert!(
            !output.contains("compile_error"),
            "should not emit compile_error: {}",
            output
        );
        assert!(
            output.contains("fn main"),
            "should generate main: {}",
            output
        );
    }

    #[test]
    fn task_macro_rejects_struct_item() {
        let attr = quote! {};
        let item = quote! { struct MyStruct { field: i32 } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "struct should be rejected: {}",
            result_str
        );
        assert!(
            result_str.contains("can only be applied to functions"),
            "error message should mention functions: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_enum_item() {
        let attr = quote! {};
        let item = quote! { enum MyEnum { A, B } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "enum should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_trait_item() {
        let attr = quote! {};
        let item = quote! { trait MyTrait { fn method(); } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "trait should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_impl_block() {
        let attr = quote! {};
        let item = quote! { impl MyStruct { fn method() {} } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "impl block should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_multiple_attributes_beyond_limit() {
        let attr = quote! { foo bar baz };
        let item = quote! { fn my_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "too many attrs should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_accepts_unknown_attribute_token() {
        let attr = quote! { unknown };
        let item = quote! { fn my_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error") && result_str.contains("unsupported attribute"),
            "unknown attribute should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_function_with_visibility_pub() {
        let attr = quote! {};
        let item = quote! { pub fn public_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            !result_str.contains("compile_error"),
            "pub fn should be accepted: {}",
            result_str
        );
        assert!(
            result_str.contains("fn main"),
            "should generate main: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_function_with_visibility_pub_crate() {
        let attr = quote! {};
        let item = quote! { pub(crate) fn crate_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            !result_str.contains("compile_error"),
            "pub(crate) fn should be accepted: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_nested_path_function_definition() {
        let attr = quote! {};
        let item = quote! { fn outer::inner_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "nested path function definition should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_empty_function_body() {
        let attr = quote! {};
        let item = quote! { fn empty_task() {} };
        let result = internal_task_macro(attr, item);
        let expected = quote! { fn empty_task() {} fn main() { empty_task(); } };
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_handles_single_expression_body() {
        let attr = quote! {};
        let item = quote! { fn single_expr() -> i32 { 42 } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            !result_str.contains("compile_error"),
            "single expression body should be accepted: {}",
            result_str
        );
        assert!(
            result_str.contains("fn main"),
            "should generate main: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_const_fn() {
        let attr = quote! {};
        let item = quote! { const fn const_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "const fn should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_static_fn() {
        let attr = quote! {};
        let item = quote! { static fn static_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "static fn should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_unsafe_fn() {
        let attr = quote! {};
        let item = quote! { unsafe fn unsafe_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            !result_str.contains("compile_error"),
            "unsafe fn should be accepted: {}",
            result_str
        );
        assert!(
            result_str.contains("unsafe"),
            "unsafe fn should generate unsafe block: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_extern_fn() {
        let attr = quote! {};
        let item = quote! { extern "C" fn extern_task() {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error") || result_str.contains("extern"),
            "extern fn should be handled: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_nested_blocks() {
        let attr = quote! {};
        let item = quote! { fn nested() { if true { loop { break; } } } };
        let result = internal_task_macro(attr, item);
        let expected = quote! { fn nested() { if true { loop { break; } } } fn main() { nested(); } };
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_rejects_async_with_explicit_return_type_mismatch() {
        let attr = quote! {};
        let item = quote! { async fn async_with_return() -> i32 {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error") && result_str.contains("async"),
            "async with explicit return type mismatch should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_handles_closure_not_fn() {
        let attr = quote! {};
        let item = quote! { let x = || {}; };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "closure should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_macro_invocation() {
        let attr = quote! {};
        let item = quote! { macro_rules! my_macro { () => {} } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "macro_rules should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_type_alias() {
        let attr = quote! {};
        let item = quote! { type MyType = i32; };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "type alias should be rejected: {}",
            result_str
        );
    }

    #[test]
    fn task_macro_rejects_union() {
        let attr = quote! {};
        let item = quote! { union MyUnion { a: i32 } };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("compile_error"),
            "union should be rejected: {}",
            result_str
        );
    }

    proptest! {
        #[test]
        fn task_macro_no_panic(attr_str in ".*", item_str in ".*") {
            let attr: proc_macro2::TokenStream = attr_str.parse().unwrap_or_else(|_| quote!{});
            let item: proc_macro2::TokenStream = item_str.parse().unwrap_or_else(|_| quote!{});
            let _ = internal_task_macro(attr, item);
        }
    }
}
