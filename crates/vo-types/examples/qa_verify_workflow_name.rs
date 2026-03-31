use vo_types::{WorkflowName, ParseError};

fn main() {
    let cases = vec![
        ("--", false),
        ("__", false),
        ("-_", false),
        ("_-", false),
        ("_abc", true),
        ("abc_def", true),
        ("abc_", false),
        ("-abc", false),
        ("abc-", false),
    ];

    let mut all_passed = true;
    println!("WorkflowName::parse Contract Verification:");
    println!("{:<10} | {:<10} | {:<10} | {:<10}", "Input", "Expected", "Actual", "Status");
    println!("{}", "-".repeat(50));

    for (input, expected_success) in cases {
        let result = WorkflowName::parse(input);
        let actual_success = result.is_ok();
        let status = if actual_success == expected_success { "✅ PASS" } else { "❌ FAIL" };
        if actual_success != expected_success {
            all_passed = false;
        }
        
        let actual_str = if actual_success { "Success" } else { "Error" };
        let expected_str = if expected_success { "Success" } else { "Error" };
        
        println!("{:<10} | {:<10} | {:<10} | {:<10}", input, expected_str, actual_str, status);
        if let Err(e) = result {
            if !expected_success {
                 // Optionally print error type for debugging
                 // println!("   Error detail: {:?}", e);
            }
        }
    }

    if all_passed {
        println!("\nQA VERIFIED");
    } else {
        println!("\nQA FAILED");
        std::process::exit(1);
    }
}
