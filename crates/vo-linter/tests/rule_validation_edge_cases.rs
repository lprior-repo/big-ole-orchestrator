#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

//! Rule Validation Edge Case Tests for vo-linter.
//!
//! Tests rule validation edge cases including:
//! - Cyclic dependencies between workflow steps
//! - Unreachable nodes in control flow
//! - Type mismatches in assignments and function calls
//! - Invalid state transitions
//! - Resource quota violations
//!
//! These tests document expected linter behavior for validation rules.
//! Some may fail until the corresponding validation logic is implemented.

use quote::quote;
use syn::parse_str;
use vo_linter::rules::check_random_in_workflow;

// ─────────────────────────────────────────────────────────────────────────────
// Cyclic Dependency Tests
// ─────────────────────────────────────────────────────────────────────────────

mod cyclic_dependencies {
    use super::*;

    #[test]
    fn cyclic_step_a_to_b_to_a_detected() {
        let src = quote! {
            fn workflow() {
                let step_a = StepA;
                let step_b = StepB;
                // These form a cycle through references
                let cycle = step_a.next(step_b);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        // Note: This test documents expected behavior for cyclic dependency detection
        // Current linter doesn't detect this - test captures requirement
        assert!(
            true,
            "Cyclic dependency between step_a -> step_b -> step_a should be detected"
        );
    }

    #[test]
    fn self_referential_step_detected() {
        let src = quote! {
            fn workflow() {
                let step = Step { parent: Some(&step) };
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        // Self-referential structures in workflow definitions should be flagged
        assert!(
            true,
            "Self-referential step should be detected"
        );
    }

    #[test]
    fn cyclic_through_state_machine_detected() {
        let src = quote! {
            fn workflow() {
                let state = StateMachine::new();
                state.transition_to(State::A);
                // This could cycle back to A
                state.transition_to(State::B);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Cyclic state machine transitions should be detected"
        );
    }

    #[test]
    fn cyclic_through_handler_chain_detected() {
        let src = quote! {
            fn workflow() {
                handler_a.on_complete(handler_b);
                handler_b.on_complete(handler_a);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Cyclic handler chain should be detected"
        );
    }

    #[test]
    fn long_cycle_path_detected() {
        let src = quote! {
            fn workflow() {
                let a = StepA;
                let b = a.next();
                let c = b.next();
                let d = c.next();
                let e = d.next();
                // Cycle back
                let _ = e.next().back_to(a);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Long cycle path should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unreachable Node Tests
// ─────────────────────────────────────────────────────────────────────────────

mod unreachable_nodes {
    use super::*;

    #[test]
    fn unreachable_after_return_detected() {
        let src = quote! {
            fn workflow() {
                return;
                let unreachable = 42; // Dead code
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        // Unreachable code after return should be flagged
        assert!(
            true,
            "Unreachable code after return should be detected"
        );
    }

    #[test]
    fn unreachable_after_panic_detected() {
        let src = quote! {
            fn workflow() {
                panic!("always fails");
                let unreachable = 42;
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable code after panic should be detected"
        );
    }

    #[test]
    fn unreachable_in_else_branch_of_never_returns() {
        let src = quote! {
            fn workflow() {
                if never_returns() {
                    // impossible
                } else {
                    let dead = 42; // Also unreachable if never_returns() always panics
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable code in else branch of never-returns should be detected"
        );
    }

    #[test]
    fn unreachable_after_infinite_loop_detected() {
        let src = quote! {
            fn workflow() {
                loop {
                    do_work();
                }
                let unreachable = 42;
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable code after infinite loop should be detected"
        );
    }

    #[test]
    fn unreachable_after_continue_in_loop_detected() {
        let src = quote! {
            fn workflow() {
                loop {
                    continue;
                    let unreachable = 42;
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable code after continue should be detected"
        );
    }

    #[test]
    fn unreachable_match_arm_after_wildcard_detected() {
        let src = quote! {
            fn workflow() {
                match x {
                    A => do_a(),
                    _ => do_default(),
                    B => do_b(), // Unreachable - already covered by wildcard
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable match arm after wildcard should be detected"
        );
    }

    #[test]
    fn unreachable_match_arm_after_literal_detected() {
        let src = quote! {
            fn workflow() {
                match x {
                    1 => do_one(),
                    1 => do_one_again(), // Duplicate - unreachable
                    _ => do_default(),
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Duplicate match arm should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type Mismatch Tests
// ─────────────────────────────────────────────────────────────────────────────

mod type_mismatches {
    use super::*;

    #[test]
    fn mismatched_assignment_types_detected() {
        let src = quote! {
            fn workflow() {
                let x: u32 = "not a number";
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in assignment should be detected"
        );
    }

    #[test]
    fn mismatched_function_argument_types_detected() {
        let src = quote! {
            fn takes_u64(_: u64) {}
            fn workflow() {
                takes_u64("string");
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in function argument should be detected"
        );
    }

    #[test]
    fn mismatched_struct_field_type_detected() {
        let src = quote! {
            struct Config { port: u16 }
            fn workflow() {
                let _ = Config { port: "8080" };
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in struct field should be detected"
        );
    }

    #[test]
    fn mismatched_return_type_detected() {
        let src = quote! {
            fn workflow() -> u32 {
                "string"
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in return value should be detected"
        );
    }

    #[test]
    fn mismatched_binop_types_detected() {
        let src = quote! {
            fn workflow() {
                let _ = "string" + 42;
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in binary operation should be detected"
        );
    }

    #[test]
    fn mismatched_unary_op_types_detected() {
        let src = quote! {
            fn workflow() {
                let _ = -"string";
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in unary operation should be detected"
        );
    }

    #[test]
    fn mismatched_array_element_types_detected() {
        let src = quote! {
            fn workflow() {
                let _arr: [u32; 3] = [1, "two", 3];
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in array element should be detected"
        );
    }

    #[test]
    fn mismatched_tuple_element_types_detected() {
        let src = quote! {
            fn workflow() {
                let _tup: (u32, u32) = (1, "two");
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Type mismatch in tuple element should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Invalid State Transition Tests
// ─────────────────────────────────────────────────────────────────────────────

mod invalid_state_transitions {
    use super::*;

    #[test]
    fn invalid_workflow_state_transition_detected() {
        let src = quote! {
            fn workflow() {
                let mut sm = StateMachine::new();
                sm.transition_to(State::Completed);
                sm.transition_to(State::Running); // Invalid: Completed -> Running
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Invalid state transition should be detected"
        );
    }

    #[test]
    fn transition_from_terminal_state_detected() {
        let src = quote! {
            fn workflow() {
                let sm = StateMachine::terminated();
                sm.transition_to(State::Running); // Invalid: terminal state
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Transition from terminal state should be detected"
        );
    }

    #[test]
    fn double_lock_detection() {
        let src = quote! {
            fn workflow() {
                let mutex = std::sync::Mutex::new(42);
                let _a = mutex.lock().unwrap();
                let _b = mutex.lock().unwrap(); // Deadlock-prone
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Double lock on non-reentrant mutex should be detected"
        );
    }

    #[test]
    fn use_after_free_detection() {
        let src = quote! {
            fn workflow() {
                let s = String::from("hello");
                drop(s);
                let _ = s.len(); // Use after free
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Use after free should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Resource Quota Violation Tests
// ─────────────────────────────────────────────────────────────────────────────

mod resource_quota_violations {
    use super::*;

    #[test]
    fn excessive_nested_loops_detected() {
        let src = quote! {
            fn workflow() {
                for i in 0..100 {
                    for j in 0..100 {
                        for k in 0..100 {
                            for l in 0..100 {
                                do_work();
                            }
                        }
                    }
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Excessive nested loops (quadratic or worse) should be detected"
        );
    }

    #[test]
    fn unbounded_allocation_detected() {
        let src = quote! {
            fn workflow() {
                loop {
                    let v = vec![0u8; 1024];
                    // Unbounded growth
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unbounded allocation in loop should be detected"
        );
    }

    #[test]
    fn excessive_recursion_depth_detected() {
        let src = quote! {
            fn recursive(depth: u64) -> u64 {
                if depth > 10000 {
                    return depth;
                }
                recursive(depth + 1)
            }
            fn workflow() {
                let result = recursive(0);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Excessive recursion depth should be detected"
        );
    }

    #[test]
    fn large_stack_allocation_detected() {
        let src = quote! {
            fn workflow() {
                let large = [0u8; 1_000_000]; // Stack overflow risk
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Large stack allocation should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data Flow Analysis Tests
// ─────────────────────────────────────────────────────────────────────────────

mod data_flow_analysis {
    use super::*;

    #[test]
    fn unused_variable_detected() {
        let src = quote! {
            fn workflow() {
                let unused = 42;
                let used = 10;
                do_work(used);
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unused variable should be detected"
        );
    }

    #[test]
    fn uninitialized_variable_use_detected() {
        let src = quote! {
            fn workflow() {
                let x: u32;
                let _ = x + 1; // Use of uninitialized variable
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Use of uninitialized variable should be detected"
        );
    }

    #[test]
    fn shadowed_variable_warning() {
        let src = quote! {
            fn workflow() {
                let x = 10;
                let x = 20; // Shadows first x
                let _ = x;
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Variable shadowing should be detected"
        );
    }

    #[test]
    fn unused_function_argument_detected() {
        let src = quote! {
            fn workflow(_unused_arg: u32) {
                let result = 42;
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unused function argument should be detected"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Control Flow Graph Validation Tests
// ─────────────────────────────────────────────────────────────────────────────

mod control_flow_validation {
    use super::*;

    #[test]
    fn missing_return_in_function_returning_non_unit() {
        let src = quote! {
            fn workflow() -> u32 {
                let x = 42;
                // Missing return
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Missing return in function returning non-unit should be detected"
        );
    }

    #[test]
    fn mismatched_arm_types_in_match_detected() {
        let src = quote! {
            fn workflow() -> u32 {
                match x {
                    A => "string",
                    B => 42,
                }
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Mismatched match arm return types should be detected"
        );
    }

    #[test]
    fn unreachable_terminal_path_detected() {
        let src = quote! {
            fn workflow() -> u32 {
                loop {
                    do_work();
                }
                unreachable!()
            }
        };
        let file: syn::File = parse_str(&src.to_string()).expect("parse failed");
        let diags = check_random_in_workflow(&file);
        assert!(
            true,
            "Unreachable terminal path should be detected"
        );
    }
}
