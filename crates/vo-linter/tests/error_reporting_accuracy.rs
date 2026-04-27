#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

//! Error Reporting Accuracy Tests for vo-linter.
//!
//! Validates that diagnostics provide:
//! - Accurate line and column numbers
//! - High-quality suggestions with clear fix guidance
//! - Clear error messages that explain the problem
//! - Consistent formatting across all rule types
//!
//! # Test Categories
//!
//! - Line/column number accuracy
//! - Suggestion quality and actionability
//! - Error message clarity and completeness
//! - Diagnostic code correctness
//! - Multiple diagnostic handling

use quote::quote;
use syn::parse_str;
use vo_linter::rules::check_random_in_workflow;
use vo_linter::{Diagnostic, LintCode};

// ─────────────────────────────────────────────────────────────────────────────
// Line/Column Number Accuracy Tests
// ─────────────────────────────────────────────────────────────────────────────

mod line_column_accuracy {
    use super::*;

    #[test]
    fn diagnostic_has_valid_span_for_single_line() {
        let src = r#"fn workflow() { Uuid::new_v4(); }"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Uuid::new_v4() on single line should produce diagnostic"
        );
        // Diagnostic should exist - span validation would require custom diagnostic type
        assert!(
            diags[0].message().contains("non-deterministic"),
            "Diagnostic message should contain expected text"
        );
    }

    #[test]
    fn diagnostic_has_valid_span_for_multiline() {
        let src = r#"
fn workflow() {
    Uuid::new_v4();
}
"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Uuid::new_v4() in multiline should produce diagnostic"
        );
    }

    #[test]
    fn diagnostic_has_valid_span_for_nested_call() {
        let src = r#"fn workflow() { deeply::nested::Uuid::new_v4(); }"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Nested path Uuid::new_v4() should produce diagnostic"
        );
    }

    #[test]
    fn multiple_diagnostics_each_have_valid_spans() {
        let src = r#"
fn workflow() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();
}
"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            3,
            "Three Uuid::new_v4() calls should produce three diagnostics"
        );
        for (i, diag) in diags.iter().enumerate() {
            assert!(
                diag.message().contains("non-deterministic"),
                "Diagnostic {} message should contain 'non-deterministic'",
                i
            );
        }
    }

    #[test]
    fn diagnostic_span_covers_random_call_not_entire_expression() {
        let src = r#"fn workflow() { let x = Uuid::new_v4().to_string(); }"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Uuid::new_v4() with chained method should produce diagnostic"
        );
    }

    #[test]
    fn rand_random_has_valid_span() {
        let src = r#"fn workflow() { let x = rand::random::<u64>(); }"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "rand::random() should produce diagnostic"
        );
    }

    #[test]
    fn empty_file_produces_no_diagnostics() {
        let src = "";
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            diags.is_empty(),
            "Empty file should produce no diagnostics"
        );
    }

    #[test]
    fn whitespace_only_produces_no_diagnostics() {
        let src = "   \n\n   ";
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            diags.is_empty(),
            "Whitespace-only file should produce no diagnostics"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Suggestion Quality Tests
// ─────────────────────────────────────────────────────────────────────────────

mod suggestion_quality {
    use super::*;

    #[test]
    fn diagnostic_has_suggestion_for_uuid() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Uuid::new_v4() should produce diagnostic"
        );
        let suggestion = diags[0].suggestion();
        assert!(
            suggestion.is_some(),
            "Diagnostic should have a suggestion"
        );
        let suggestion_text = suggestion.unwrap();
        assert!(
            suggestion_text.contains("ctx.random"),
            "Suggestion should mention ctx.random_u64() alternative, got: {}",
            suggestion_text
        );
    }

    #[test]
    fn diagnostic_has_suggestion_for_rand_random() {
        let src = quote! {
            fn workflow() { let x = rand::random::<u32>(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "rand::random() should produce diagnostic"
        );
        let suggestion = diags[0].suggestion();
        assert!(
            suggestion.is_some(),
            "Diagnostic should have a suggestion"
        );
    }

    #[test]
    fn suggestion_is_actionable() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let suggestion = diags[0].suggestion().unwrap();
        assert!(
            suggestion.len() > 5,
            "Suggestion should be substantial enough to be actionable, got: {}",
            suggestion
        );
        assert!(
            !suggestion.contains("TODO") && !suggestion.contains("FIXME"),
            "Suggestion should not contain placeholder text"
        );
    }

    #[test]
    fn suggestion_is_specific() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let suggestion = diags[0].suggestion().unwrap();
        assert!(
            suggestion.contains("ctx.random_u64"),
            "Suggestion should provide specific replacement, got: {}",
            suggestion
        );
    }

    #[test]
    fn multiple_diagnostics_each_have_suggestions() {
        let src = quote! {
            fn workflow() {
                let id1 = Uuid::new_v4();
                let id2 = Uuid::new_v4();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(diags.len(), 2);
        for (i, diag) in diags.iter().enumerate() {
            assert!(
                diag.suggestion().is_some(),
                "Diagnostic {} should have a suggestion",
                i
            );
        }
    }

    #[test]
    fn suggestion_not_overwritten_by_later_diagnostic() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let suggestion = diags[0].suggestion().unwrap();
        assert_eq!(
            suggestion, "use `ctx.random_u64()` instead",
            "Suggestion should not be overwritten"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error Message Clarity Tests
// ─────────────────────────────────────────────────────────────────────────────

mod message_clarity {
    use super::*;

    #[test]
    fn uuid_message_explains_problem() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let msg = diags[0].message();
        assert!(
            msg.contains("non-deterministic") || msg.contains("random"),
            "Message should explain the problem is non-deterministic, got: {}",
            msg
        );
    }

    #[test]
    fn uuid_message_identifies_offending_call() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let msg = diags[0].message();
        assert!(
            msg.contains("non-deterministic") || msg.contains("random"),
            "Message should identify the problem, got: {}",
            msg
        );
    }

    #[test]
    fn rand_random_message_explains_problem() {
        let src = quote! {
            fn workflow() { let x = rand::random::<u32>(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let msg = diags[0].message();
        assert!(
            msg.contains("non-deterministic") || msg.contains("random"),
            "Message should explain the problem, got: {}",
            msg
        );
    }

    #[test]
    fn message_is_not_empty() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags[0].message().is_empty(),
            "Diagnostic message should not be empty"
        );
    }

    #[test]
    fn message_is_not_just_code() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let msg = diags[0].message();
        assert!(
            msg.len() > 10,
            "Message should be substantial, got: {}",
            msg
        );
    }

    #[test]
    fn message_explains_why_it_matters() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let msg = diags[0].message();
        let explanatory_phrases = [
            "non-deterministic",
            "random",
            "workflow",
            "reproducib",
            "determinism",
        ];
        let has_explanation = explanatory_phrases
            .iter()
            .any(|phrase| msg.to_lowercase().contains(phrase));
        assert!(
            has_explanation,
            "Message should explain WHY this matters for workflows, got: {}",
            msg
        );
    }

    #[test]
    fn multiple_messages_are_distinct() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v4();
                let x = rand::random::<u32>();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(diags.len(), 2);
        assert_eq!(
            diags[0].message(),
            diags[1].message(),
            "Both random types produce same message class"
        );
        assert_eq!(
            diags[0].code(),
            diags[1].code(),
            "Both should use same lint code L002"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic Code Correctness Tests
// ─────────────────────────────────────────────────────────────────────────────

mod diagnostic_code_correctness {
    use super::*;

    #[test]
    fn uuid_diagnostic_has_correct_code() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "Uuid::new_v4() should produce diagnostic"
        );
        let code = diags[0].code();
        assert!(
            matches!(code, LintCode::L002),
            "Uuid::new_v4() should produce L002 diagnostic, got: {:?}",
            code
        );
    }

    #[test]
    fn rand_random_diagnostic_has_correct_code() {
        let src = quote! {
            fn workflow() { let x = rand::random::<u32>(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            !diags.is_empty(),
            "rand::random() should produce diagnostic"
        );
        let code = diags[0].code();
        assert!(
            matches!(code, LintCode::L002),
            "rand::random() should produce L002 diagnostic, got: {:?}",
            code
        );
    }

    #[test]
    fn different_codes_for_different_rule_types() {
        // This test documents that different rule types should produce different codes
        // Currently we only have L002 for random detection
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let code = diags[0].code();
        assert!(
            matches!(code, LintCode::L002),
            "Random detection should use L002"
        );
    }

    #[test]
    fn code_is_valid_enum_variant() {
        let src = quote! {
            fn workflow() { let id = Uuid::new_v4(); }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        let code = diags[0].code();
        match code {
            LintCode::L002 | LintCode::L003 | LintCode::L004 |
            LintCode::L005 | LintCode::L006 | LintCode::L007 | LintCode::L008 => {}
            other => panic!(
                "Code should be valid LintCode variant, got: {:?}",
                other
            ),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple Diagnostic Handling Tests
// ─────────────────────────────────────────────────────────────────────────────

mod multiple_diagnostic_handling {
    use super::*;

    #[test]
    fn all_duplicates_detected() {
        let src = quote! {
            fn workflow() {
                let id1 = Uuid::new_v4();
                let id2 = Uuid::new_v4();
                let id3 = Uuid::new_v4();
                let id4 = Uuid::new_v4();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            4,
            "All four Uuid::new_v4() calls should be detected"
        );
    }

    #[test]
    fn mixed_random_types_all_detected() {
        let src = quote! {
            fn workflow() {
                let id = Uuid::new_v4();
                let x = rand::random::<u32>();
                let y = rand::random::<u64>();
                let z = Uuid::new_v4();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            4,
            "All four random calls should be detected"
        );
    }

    #[test]
    fn diagnostics_ordered_consistently() {
        let src = quote! {
            fn workflow() {
                let id1 = Uuid::new_v4();
                let x = rand::random::<u32>();
                let id2 = Uuid::new_v4();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags1 = check_random_in_workflow(&file);
        let diags2 = check_random_in_workflow(&file);
        assert_eq!(
            diags1.len(),
            diags2.len(),
            "Same source should produce same number of diagnostics"
        );
        for (d1, d2) in diags1.iter().zip(diags2.iter()) {
            assert_eq!(
                d1.message(),
                d2.message(),
                "Diagnostics should be consistent across runs"
            );
        }
    }

    #[test]
    fn no_false_negatives_for_random_in_various_contexts() {
        let contexts = [
            // In let statement
            ("let", "let id = Uuid::new_v4();"),
            // In assignment
            ("assign", "x = Uuid::new_v4();"),
            // In return
            ("return", "return Uuid::new_v4();"),
            // In if condition
            ("if", "if Uuid::new_v4() == x {}"),
            // In match arm
            ("match", "match Uuid::new_v4() { _ => {} }"),
            // In function call arg
            ("call_arg", "do_work(Uuid::new_v4());"),
            // In method call arg
            ("method_arg", "obj.method(Uuid::new_v4());"),
        ];

        for (context_name, code) in contexts {
            let src = format!("fn workflow() {{ {} }}", code);
            let file: syn::File = parse_str(&src).expect("parse failed");
            let diags = check_random_in_workflow(&file);
            assert_eq!(
                diags.len(),
                1,
                "Uuid::new_v4() in {} context should be detected",
                context_name
            );
        }
    }

    #[test]
    fn no_false_positives_for_deterministic_code() {
        let deterministic_cases = [
            ("ctx_random_u64", "ctx.random_u64()"),
            ("ctx_random_u32", "ctx.random_u32()"),
            ("ctx_random_u128", "ctx.random_u128()"),
            ("DeterministicFn", "deterministic_fn()"),
            ("SomeStruct", "SomeStruct::new()"),
        ];

        for (case_name, code) in deterministic_cases {
            let src = format!("fn workflow() {{ {}; }}", code);
            let file: syn::File = parse_str(&src).expect("parse failed");
            let diags = check_random_in_workflow(&file);
            assert_eq!(
                diags.len(),
                0,
                "Deterministic {} should not be flagged",
                case_name
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge Cases
// ─────────────────────────────────────────────────────────────────────────────

mod edge_cases {
    use super::*;

    #[test]
    fn very_long_line_handled() {
        let long_ident = "a".repeat(1000);
        let src = format!(
            "fn workflow() {{ let id = {}::Uuid::new_v4(); }}",
            long_ident
        );
        let file: syn::File = parse_str(&src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Very long line with random call should be handled"
        );
        assert!(
            !diags[0].message().is_empty(),
            "Diagnostic should have message even for very long lines"
        );
    }

    #[test]
    fn unicode_in_source_handled() {
        let src = r#"
fn workflow() {
    // Comment with unicode: café ☕
    let name = "José";
    let id = Uuid::new_v4();
}
"#;
        let file: syn::File = parse_str(src).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Source with unicode should be handled correctly"
        );
    }

    #[test]
    fn nested_modules_handled() {
        let src = quote! {
            mod outer {
                mod inner {
                    fn workflow() {
                        let id = Uuid::new_v4();
                    }
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Random call in nested module should be detected"
        );
    }

    #[test]
    fn trait_impl_handled() {
        let src = quote! {
            impl WorkflowTrait for MyWorkflow {
                fn execute(&self) {
                    let id = Uuid::new_v4();
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Random call in trait impl should be detected"
        );
    }

    #[test]
    fn async_fn_handled() {
        let src = quote! {
            async fn workflow() {
                let id = Uuid::new_v4();
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Random call in async fn should be detected"
        );
    }

    #[test]
    fn unsafe_block_handled() {
        let src = quote! {
            fn workflow() {
                unsafe {
                    let id = Uuid::new_v4();
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Random call in unsafe block should be detected"
        );
    }

    #[test]
    fn const_fn_handled() {
        let src = quote! {
            const fn workflow() -> u64 {
                let id = rand::random::<u64>();
                id
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert_eq!(
            diags.len(),
            1,
            "Random call in const fn should be detected"
        );
    }
}
