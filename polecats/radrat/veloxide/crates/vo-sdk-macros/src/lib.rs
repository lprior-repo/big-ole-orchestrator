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

    proptest! {
        #[test]
        fn task_macro_no_panic(attr_str in ".*", item_str in ".*") {
            // we parse string as token streams for testing
            let attr: proc_macro2::TokenStream = attr_str.parse().unwrap_or_else(|_| quote!{});
            let item: proc_macro2::TokenStream = item_str.parse().unwrap_or_else(|_| quote!{});
            let _ = internal_task_macro(attr, item);
        }
    }
}
