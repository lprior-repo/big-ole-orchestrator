#![allow(unexpected_cfgs)]

use proc_macro2::TokenStream;
use syn::Type;

#[derive(Debug, PartialEq)]
pub struct TaskDef {
    pub ident: String,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub return_type: Option<Type>,
    pub generics: syn::Generics,
    pub args: Vec<(String, Type)>,
}

use crate::error::Error;

pub fn parse_task(item: &TokenStream) -> Result<TaskDef, Error> {
    if item.is_empty() {
        return Err(Error::ParseFailure);
    }

    let parsed: syn::ItemFn = if let Ok(f) = syn::parse2(item.clone()) {
        f
    } else {
        if syn::parse2::<syn::Item>(item.clone()).is_ok() {
            return Err(Error::InvalidInputItem);
        }
        return Err(Error::ParseFailure);
    };

    let args: Vec<(String, Type)> = parsed
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                let ident = if let syn::Pat::Ident(ident) = &*pat_type.pat {
                    ident.ident.to_string()
                } else {
                    return None;
                };
                Some((ident, (*pat_type.ty).clone()))
            } else {
                None
            }
        })
        .collect();

    let return_type = match parsed.sig.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(*ty),
    };

    Ok(TaskDef {
        ident: parsed.sig.ident.to_string(),
        is_async: parsed.sig.asyncness.is_some(),
        is_unsafe: parsed.sig.unsafety.is_some(),
        return_type,
        generics: parsed.sig.generics,
        args,
    })
}

#[allow(clippy::unnecessary_wraps)]
pub fn generate_task_entrypoint(task: &TaskDef) -> Result<TokenStream, Error> {
    let ident = syn::parse_str::<syn::Ident>(&task.ident).map_err(|_| Error::IdentParsingFailed)?;

    let is_generic = !task.generics.params.is_empty()
        || task.generics.lt_token.is_some()
        || task.generics.where_clause.is_some();

    // Generic tasks: main() has no return type (type params can't appear in main's signature)
    let ret_type = if is_generic {
        quote::quote! {}
    } else {
        match &task.return_type {
            Some(ty) => quote::quote! { -> #ty },
            None => quote::quote! {},
        }
    };

    let arg_idents: Vec<syn::Ident> = task
        .args
        .iter()
        .filter_map(|(name, _)| syn::parse_str::<syn::Ident>(name).ok())
        .collect();

    let env_bindings: Vec<TokenStream> = task
        .args
        .iter()
        .filter_map(|(name, _)| {
            let ident = syn::parse_str::<syn::Ident>(name).ok()?;
            let env_name = name.to_uppercase();
            Some(quote::quote! {
                let #ident = std::env::var(#env_name).unwrap_or_default();
            })
        })
        .collect();

    let call = if task.is_async {
        if arg_idents.is_empty() {
            quote::quote! { #ident().await }
        } else {
            quote::quote! { #ident(#(#arg_idents),*).await }
        }
    } else {
        if arg_idents.is_empty() {
            quote::quote! { #ident() }
        } else {
            quote::quote! { #ident(#(#arg_idents),*) }
        }
    };

    let call_or_unsafe = if task.is_unsafe {
        quote::quote! { unsafe { #call } }
    } else {
        call
    };

    // Generic tasks always get a semicolon (no return type propagation)
    let body = if is_generic || task.return_type.is_none() {
        quote::quote! { #call_or_unsafe; }
    } else {
        quote::quote! { #call_or_unsafe }
    };

    let wrapper = if task.is_async {
        if env_bindings.is_empty() {
            quote::quote! {
                fn main () #ret_type {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build current-thread runtime");
                    rt.block_on(async { #body })
                }
            }
        } else {
            quote::quote! {
                fn main () #ret_type {
                    #(#env_bindings)*
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build current-thread runtime");
                    rt.block_on(async { #body })
                }
            }
        }
    } else {
        if env_bindings.is_empty() {
            quote::quote! {
                fn main () #ret_type {
                    #body
                }
            }
        } else {
            quote::quote! {
                fn main () #ret_type {
                    #(#env_bindings)*
                    #body
                }
            }
        }
    };

    Ok(wrapper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn parse_task_rejects_non_function_items() {
        let input = quote! { struct MyTask; };
        let result = parse_task(&input);
        assert_eq!(result, Err(Error::InvalidInputItem));
    }

    #[test]
    fn parse_task_rejects_invalid_syntax() {
        let input = quote! { fn my_task() { } ; }; // extra semicolon
        let result = parse_task(&input);
        assert_eq!(result, Err(Error::ParseFailure));
    }

    #[test]
    fn parse_task_rejects_empty_token_stream() {
        let input = quote! {};
        let result = parse_task(&input);
        assert_eq!(result, Err(Error::ParseFailure));
    }

    #[test]
    fn parse_task_accepts_missing_or_empty_attributes() {
        let input = quote! { fn a(){} };
        let expected = TaskDef {
            ident: "a".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_handles_complex_return_type() {
        let input = quote! { fn my_task() -> Result<(), std::io::Error> {} };
        let expected_ty: Type = parse_quote!(Result<(), std::io::Error>);
        let expected = TaskDef {
            ident: "my_task".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: Some(expected_ty),
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_accepts_minimum_valid_token_stream() {
        let input = quote! { fn a(){} };
        let expected = TaskDef {
            ident: "a".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_accepts_single_argument() {
        let input = quote! { fn task(a: i32) {} };
        let expected_ty: Type = parse_quote!(i32);
        let expected = TaskDef {
            ident: "task".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![("a".to_string(), expected_ty)],
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_accepts_multiple_arguments() {
        let input = quote! { fn task(a: i32, b: String, c: Vec<u8>) {} };
        let expected_a: Type = parse_quote!(i32);
        let expected_b: Type = parse_quote!(String);
        let expected_c: Type = parse_quote!(Vec<u8>);
        let expected = TaskDef {
            ident: "task".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![
                ("a".to_string(), expected_a),
                ("b".to_string(), expected_b),
                ("c".to_string(), expected_c),
            ],
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn generate_task_entrypoint_processes_minimum_taskdef() {
        let task = TaskDef {
            ident: "a".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let expected = quote! { fn main() { a(); } };
        let result = generate_task_entrypoint(&task).unwrap();
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn generate_task_entrypoint_processes_taskdef_with_complex_return_type() {
        let expected_ty: Type = parse_quote!(Result<(), std::io::Error>);
        let task = TaskDef {
            ident: "run".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: Some(expected_ty),
            generics: syn::Generics::default(),
            args: vec![],
        };
        let expected = quote! { fn main() -> Result<(), std::io::Error> { run() } };
        let result = generate_task_entrypoint(&task).unwrap();
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn generate_task_entrypoint_rejects_invalid_ident() {
        let task = TaskDef {
            ident: "123invalid".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = generate_task_entrypoint(&task);
        assert!(matches!(result, Err(Error::IdentParsingFailed)));
    }

    #[test]
    fn generate_task_entrypoint_rejects_empty_ident() {
        let task = TaskDef {
            ident: String::new(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = generate_task_entrypoint(&task);
        assert!(matches!(result, Err(Error::IdentParsingFailed)));
    }

    #[test]
    fn generate_task_entrypoint_rejects_whitespace_ident() {
        let task = TaskDef {
            ident: " ".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = generate_task_entrypoint(&task);
        assert!(matches!(result, Err(Error::IdentParsingFailed)));
    }

    #[test]
    fn parse_task_accepts_generic_function() {
        let input = quote! { fn generic_task<T: Default>() -> T { T::default() } };
        let result = parse_task(&input);
        assert!(
            result.is_ok(),
            "generic function should be accepted, got: {:?}",
            result
        );
        let def = result.unwrap();
        assert_eq!(def.ident, "generic_task");
        assert!(!def.generics.params.is_empty());
    }

    #[test]
    fn parse_task_accepts_async_generic_with_where_clause() {
        let input = quote! { async fn complex<'a, T>() where T: Send + 'a {} };
        let result = parse_task(&input);
        assert!(
            result.is_ok(),
            "async generic with where clause should be accepted, got: {:?}",
            result
        );
        let def = result.unwrap();
        assert_eq!(def.ident, "complex");
        assert!(def.generics.where_clause.is_some());
    }

    #[test]
    fn generate_task_entrypoint_omits_generics_from_main_for_generic_task() {
        let input = quote! { fn generic_task<T: Default>() -> T { T::default() } };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        // fn main<T> is invalid Rust — main must not have generics
        assert!(
            !output.contains("fn main <"),
            "main should not have generics: {}",
            output
        );
        assert!(
            output.contains("fn main ()"),
            "main should be non-generic: {}",
            output
        );
    }

    #[test]
    fn generate_task_entrypoint_calls_generic_function() {
        let input = quote! { fn generic_task<T: Default>() -> T { T::default() } };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        assert!(
            output.contains("generic_task ()"),
            "main should call the generic function: {}",
            output
        );
    }

    #[test]
    fn generate_task_entrypoint_omits_generic_return_type_from_main() {
        let input = quote! { fn generic_task<T: Default>() -> T { T::default() } };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        // main() must not have -> T since T is not in main's scope
        assert!(
            !output.contains("-> T"),
            "main should not have generic return type: {}",
            output
        );
    }

    #[test]
    fn parse_task_detects_async_function() {
        let input = quote! { async fn my_async_task() {} };
        let result = parse_task(&input).unwrap();
        assert_eq!(result.ident, "my_async_task");
        assert!(result.is_async, "async fn should have is_async = true");
        assert!(!result.is_unsafe);
        assert!(result.return_type.is_none());
        assert!(result.args.is_empty());
    }

    #[test]
    fn parse_task_detects_async_with_return_type() {
        let input = quote! { async fn fetch() -> Result<Vec<u8>, Error> {} };
        let expected_ty: Type = parse_quote!(Result<Vec<u8>, Error>);
        let result = parse_task(&input).unwrap();
        assert!(result.is_async, "async fn with return type should have is_async = true");
        assert_eq!(result.return_type, Some(expected_ty));
    }

    #[test]
    fn parse_task_detects_async_with_args() {
        let input = quote! { async fn process(url: String, timeout: u64) {} };
        let result = parse_task(&input).unwrap();
        assert!(result.is_async);
        assert_eq!(result.args.len(), 2);
        assert_eq!(result.args[0].0, "url");
        assert_eq!(result.args[1].0, "timeout");
    }

    #[test]
    fn parse_task_detects_async_unsafe_function() {
        let input = quote! { async unsafe fn low_level_op() {} };
        let result = parse_task(&input).unwrap();
        assert!(result.is_async, "async unsafe fn should have is_async = true");
        assert!(result.is_unsafe, "async unsafe fn should have is_unsafe = true");
    }

    #[test]
    fn parse_task_sync_function_has_is_async_false() {
        let input = quote! { fn sync_task() {} };
        let result = parse_task(&input).unwrap();
        assert!(!result.is_async, "sync fn should have is_async = false");
    }

    #[test]
    fn generate_task_entrypoint_async_no_args() {
        let input = quote! { async fn my_async_task() {} };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        assert!(output.contains("fn main ()"), "should generate main: {}", output);
        assert!(
            output.contains("tokio :: runtime :: Builder :: new_current_thread ()"),
            "should use tokio runtime: {}",
            output
        );
        assert!(output.contains("block_on"), "should call block_on: {}", output);
        assert!(output.contains("my_async_task () . await"), "should await call: {}", output);
    }

    #[test]
    fn generate_task_entrypoint_async_with_args() {
        let input = quote! { async fn fetch(url: String) {} };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        assert!(output.contains("std :: env :: var (\"URL\")"), "should read env var URL: {}", output);
        assert!(
            output.contains("fetch (url) . await"),
            "should call fetch(url).await: {}",
            output
        );
    }

    #[test]
    fn generate_task_entrypoint_async_with_return_type() {
        let input = quote! { async fn work() -> Result<(), Error> {} };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        assert!(
            output.contains("-> Result"),
            "should propagate return type to main: {}",
            output
        );
        assert!(
            output.contains("Error"),
            "should contain Error type in return: {}",
            output
        );
        assert!(
            !output.contains("work () . await ;"),
            "async fn with return type should NOT have semicolon after await: {}",
            output
        );
    }

    #[test]
    fn generate_task_entrypoint_async_unsafe() {
        let task = TaskDef {
            ident: "low_level".to_string(),
            is_async: true,
            is_unsafe: true,
            return_type: None,
            generics: syn::Generics::default(),
            args: vec![],
        };
        let result = generate_task_entrypoint(&task).unwrap();
        let output = result.to_string();
        assert!(
            output.contains("unsafe"),
            "async unsafe should generate unsafe block: {}",
            output
        );
        assert!(
            output.contains("low_level () . await"),
            "should await the call: {}",
            output
        );
    }

    #[test]
    fn generate_task_entrypoint_async_generic_uses_tokio() {
        let input = quote! { async fn generic_async<T: Send>() where T: Default {} };
        let def = parse_task(&input).unwrap();
        let result = generate_task_entrypoint(&def).unwrap();
        let output = result.to_string();
        assert!(
            output.contains("tokio :: runtime :: Builder :: new_current_thread ()"),
            "async generic should use tokio runtime: {}",
            output
        );
        assert!(
            output.contains("block_on"),
            "async generic should use block_on: {}",
            output
        );
    }

    proptest! {
        #[test]
        fn parse_task_no_panic(item_str in ".*") {
            let item: proc_macro2::TokenStream = item_str.parse().unwrap_or_else(|_| quote!{});
            let _ = parse_task(&item);
        }

        #[test]
        fn generate_task_entrypoint_no_panic(
            ident in "[a-zA-Z_][a-zA-Z0-9_]*",
            is_async in proptest::bool::ANY
        ) {
            let task = TaskDef {
                ident,
                is_async,
                is_unsafe: false,
                return_type: None,
                generics: syn::Generics::default(),
                args: vec![],
            };
            let _ = generate_task_entrypoint(&task);
        }
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn generate_task_entrypoint_infallible() {
        let ident_bytes: [u8; 4] = kani::any();
        let is_async: bool = kani::any();

        if let Ok(s) = std::str::from_utf8(&ident_bytes) {
            let task = TaskDef {
                ident: s.to_string(),
                is_async,
                is_unsafe: false,
                return_type: None,
                generics: syn::Generics::default(),
                args: vec![],
            };
            let _ = generate_task_entrypoint(&task);
        }
    }
}
