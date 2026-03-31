use vo_types::WorkflowName;

fn main() {
    let cases = vec![
        ("invalid--name", "Consecutive Hyphens"),
        ("-invalid-name", "Leading Hyphen"),
        ("invalid-name-", "Trailing Hyphen"),
        ("invalid_name", "Invalid Character (underscore)"),
        ("invalid@name", "Invalid Character (@)"),
        ("valid-workflow-name-123", "Valid Name"),
    ];

    for (input, description) in cases {
        let result = WorkflowName::parse(input);
        match result {
            Ok(_) => println!("✅ Case: {} | Input: '{}' | Result: Success", description, input),
            Err(e) => println!("❌ Case: {} | Input: '{}' | Result: Error: {:?}", description, input, e),
        }
    }
}
