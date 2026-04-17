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
}

use crate::error::Error;

pub fn parse_attributes(attr: &TokenStream) -> Result<(), Error> {
    if attr.is_empty() {
        return Ok(());
    }

    let attr_str = attr.to_string();
    if attr_str.is_empty() {
        return Err(Error::EmptyAttribute);
    }

    let attr_count = attr_str.split_whitespace().count();
    if attr_count > 255 {
        return Err(Error::TooManyAttributes { count: attr_count });
    }

    let first_attr = attr_str.split_whitespace().next().unwrap_or("");
    if first_attr == "retries" {
        return Err(Error::UnsupportedAttribute {
            attribute: first_attr.to_string(),
        });
    }

    Err(Error::UnsupportedAttribute {
        attribute: first_attr.to_string(),
    })
}

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

    if !parsed.sig.generics.params.is_empty()
        || parsed.sig.generics.lt_token.is_some()
        || parsed.sig.generics.where_clause.is_some()
    {
        return Err(Error::GenericFunction);
    }

    let return_type = match parsed.sig.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => {
            if parsed.sig.asyncness.is_some() {
                return Err(Error::AsyncReturnTypeMismatch {
                    ident: parsed.sig.ident.to_string(),
                    return_type: quote::quote! { #ty }.to_string(),
                });
            }
            Some(*ty)
        }
    };

    Ok(TaskDef {
        ident: parsed.sig.ident.to_string(),
        is_async: parsed.sig.asyncness.is_some(),
        is_unsafe: parsed.sig.unsafety.is_some(),
        return_type,
        generics: parsed.sig.generics,
    })
}

#[allow(clippy::unnecessary_wraps)]
pub fn generate_task_entrypoint(task: &TaskDef) -> Result<TokenStream, Error> {
    let ident =
        syn::parse_str::<syn::Ident>(&task.ident).map_err(|_| Error::IdentParsingFailed {
            ident: task.ident.clone(),
        })?;

    let ret_type = match &task.return_type {
        Some(ty) => quote::quote! { -> #ty },
        None => quote::quote! {},
    };

    let call = if task.is_async {
        quote::quote! { #ident().await }
    } else {
        quote::quote! { #ident() }
    };

    let call_or_unsafe = if task.is_unsafe {
        quote::quote! { unsafe { #call } }
    } else {
        call
    };

    let body = if task.return_type.is_some() {
        quote::quote! { #call_or_unsafe }
    } else {
        quote::quote! { #call_or_unsafe; }
    };

    let (impl_generics, ty_generics, where_clause) = task.generics.split_for_impl();

    let wrapper = if task.is_async {
        quote::quote! {
            fn main (#impl_generics) #ret_type #where_clause {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build current-thread runtime");
                rt.block_on(async { #body })
            }
        }
    } else {
        quote::quote! {
            fn main (#impl_generics) #ret_type #where_clause {
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
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
        };
        let result = parse_task(&input);
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn parse_task_rejects_async_with_return_type() {
        let input = quote! { async fn my_task() -> i32 {} };
        let result = parse_task(&input);
        assert!(matches!(
            result,
            Err(Error::AsyncReturnTypeMismatch {
                ident,
                return_type
            }) if ident == "my_task" && return_type == "i32"
        ));
    }

    #[test]
    fn parse_task_accepts_async_without_return_type() {
        let input = quote! { async fn my_task() {} };
        let result = parse_task(&input).unwrap();
        assert!(result.is_async);
        assert!(result.return_type.is_none());
    }

    #[test]
    fn parse_attributes_accepts_empty() {
        let attr = quote! {};
        assert_eq!(parse_attributes(&attr), Ok(()));
    }

    #[test]
    fn parse_attributes_rejects_non_empty() {
        let attr = quote! { foo };
        let result = parse_attributes(&attr);
        assert!(
            matches!(result, Err(Error::UnsupportedAttribute { attribute }) if attribute == "foo")
        );
    }

    #[test]
    fn parse_attributes_rejects_retries() {
        let attr = quote! { retries = 3 };
        let result = parse_attributes(&attr);
        assert!(
            matches!(result, Err(Error::UnsupportedAttribute { attribute }) if attribute == "retries")
        );
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
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
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
        };
        let result = generate_task_entrypoint(&task);
        assert!(
            matches!(result, Err(Error::IdentParsingFailed { ident }) if ident == "123invalid")
        );
    }

    #[test]
    fn generate_task_entrypoint_rejects_empty_ident() {
        let task = TaskDef {
            ident: String::new(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
        };
        let result = generate_task_entrypoint(&task);
        assert!(matches!(result, Err(Error::IdentParsingFailed { ident }) if ident.is_empty()));
    }

    #[test]
    fn generate_task_entrypoint_rejects_whitespace_ident() {
        let task = TaskDef {
            ident: " ".to_string(),
            is_async: false,
            is_unsafe: false,
            return_type: None,
            generics: syn::Generics::default(),
        };
        let result = generate_task_entrypoint(&task);
        assert!(matches!(result, Err(Error::IdentParsingFailed { ident }) if ident == " "));
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
            };
            let _ = generate_task_entrypoint(&task);
        }
    }
}
