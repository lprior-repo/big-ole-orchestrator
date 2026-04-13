#![allow(unexpected_cfgs)]

use proc_macro2::TokenStream;
use syn::Type;

#[derive(Debug, PartialEq)]
pub struct TaskDef {
    pub ident: String,
    pub is_async: bool,
    pub return_type: Option<Type>,
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

    if !parsed.sig.inputs.is_empty() {
        return Err(Error::UnsupportedSignature);
    }

    let return_type = match parsed.sig.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(*ty),
    };

    Ok(TaskDef {
        ident: parsed.sig.ident.to_string(),
        is_async: parsed.sig.asyncness.is_some(),
        return_type,
    })
}

#[allow(clippy::unnecessary_wraps)]
pub fn generate_task_entrypoint(task: &TaskDef) -> Result<TokenStream, Error> {
    let ident = syn::parse_str::<syn::Ident>(&task.ident).map_err(|_| Error::ParseFailure)?;

    let ret_type = match &task.return_type {
        Some(ty) => quote::quote! { -> #ty },
        None => quote::quote! {},
    };

    let call = if task.is_async {
        quote::quote! { #ident().await }
    } else {
        quote::quote! { #ident() }
    };

    let body = if task.return_type.is_some() {
        quote::quote! { #call }
    } else {
        quote::quote! { #call; }
    };

    let wrapper = if task.is_async {
        quote::quote! {
            fn main() #ret_type {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build current-thread runtime");
                rt.block_on(async { #body })
            }
        }
    } else {
        quote::quote! {
            fn main() #ret_type {
                #body
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
            return_type: None,
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
            return_type: Some(expected_ty),
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
            return_type: None,
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_rejects_exactly_1_argument() {
        let input = quote! { fn task(a: i32) {} };
        let result = parse_task(&input);
        assert_eq!(result, Err(Error::UnsupportedSignature));
    }

    #[test]
    fn parse_task_rejects_maximum_arguments() {
        let mut args = Vec::new();
        for i in 0usize..256 {
            let ident = quote::format_ident!("arg_{}", i);
            args.push(quote! { #ident: i32 });
        }
        let input = quote! { fn task(#(#args),*) {} };
        let result = parse_task(&input);
        assert_eq!(result, Err(Error::UnsupportedSignature));
    }

    #[test]
    fn generate_task_entrypoint_processes_minimum_taskdef() {
        let task = TaskDef {
            ident: "a".to_string(),
            is_async: false,
            return_type: None,
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
            return_type: Some(expected_ty),
        };
        let expected = quote! { fn main() -> Result<(), std::io::Error> { run() } };
        let result = generate_task_entrypoint(&task).unwrap();
        assert_eq!(result.to_string(), expected.to_string());
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
                return_type: None,
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
                return_type: None,
            };
            let _ = generate_task_entrypoint(&task);
        }
    }
}
