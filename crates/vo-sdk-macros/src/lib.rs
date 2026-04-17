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

use task::{generate_task_entrypoint, parse_task};

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

    if !attr.is_empty() {
        let attr_str = attr.to_string();
        if attr_str.is_empty() {
            return quote::quote! { compile_error!("macro attribute is empty"); };
        }
        let attr_count = attr_str.split_whitespace().count();
        if attr_count > 255 {
            return quote::quote! { compile_error!("too many macro attributes (max 255)"); };
        }
        if attr_str.starts_with("retries") {
            return quote::quote! { compile_error!("unsupported attribute: retries"); };
        }
        return quote::quote! { compile_error!("unsupported attribute"); };
    }

    match parse_task(&item) {
        Ok(task_def) => {
            if let Ok(main_fn) = generate_task_entrypoint(&task_def) {
                quote::quote! {
                    #item
                    #main_fn
                }
            } else {
                quote::quote! { compile_error!("generation failed"); }
            }
        }
        Err(error::Error::InvalidInputItem) => {
            quote::quote! { compile_error!("#[task] can only be applied to functions"); }
        }
        Err(error::Error::UnsupportedSignature) => {
            quote::quote! { compile_error!("task functions cannot have arguments"); }
        }
        Err(error::Error::GenericFunction) => {
            quote::quote! { compile_error!("generic functions are not supported"); }
        }
        Err(error::Error::ParseFailure) => {
            quote::quote! { compile_error!("parse error"); }
        }
        Err(error::Error::EmptyAttribute) => {
            quote::quote! { compile_error!("macro attribute is empty"); }
        }
        Err(error::Error::TooManyAttributes) => {
            quote::quote! { compile_error!("too many macro attributes (max 255)"); }
        }
        Err(error::Error::IdentParsingFailed) => {
            quote::quote! { compile_error!("failed to parse function identifier"); }
        }
        Err(error::Error::AsyncReturnTypeMismatch) => {
            quote::quote! { compile_error!("async functions cannot have a return type"); }
        }
    }
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
        let expected = quote! { compile_error!("unsupported attribute: retries"); };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_emits_compile_error_for_non_function_items() {
        let attr = quote! {};
        let item = quote! { struct MyTask; };
        let expected = quote! { compile_error!("#[task] can only be applied to functions"); };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_accepts_single_argument_boundary() {
        let attr = quote! {};
        let item = quote! { fn task(a: i32) {} };
        let result = internal_task_macro(attr, item);
        let result_str = result.to_string();
        assert!(
            result_str.contains("fn main"),
            "missing fn main in: {}",
            result_str
        );
        assert!(result_str.contains("env"), "missing env in: {}", result_str);
        assert!(result_str.contains("\"A\""), "missing A in: {}", result_str);
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
        let expected = quote! { compile_error!("unsupported attribute"); };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn task_macro_rejects_too_many_attributes() {
        let mut attrs = Vec::new();
        for _ in 0..256 {
            attrs.push(quote! { foo });
        }
        let attr = quote! { #(#attrs)* };
        let item = quote! { fn a() {} };
        let expected = quote! { compile_error!("too many macro attributes (max 255)"); };
        let result = internal_task_macro(attr, item);
        assert_eq!(result.to_string(), expected.to_string());
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
