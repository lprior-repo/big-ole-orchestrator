import re

def fix_api_tests():
    path = "crates/vo-api/src/types/tests.rs"
    with open(path, 'r') as f:
        content = f.read()

    # Fix syntax errors first
    content = content.replace("assert!(matches!(matches!(", "assert!(matches!(")
    
    # Now replace the redundant ones with specific assertions
    content = content.replace("assert!(matches!(WorkflowName::new(name), Ok(_)));", "WorkflowName::new(name).unwrap();")
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {name} to be invalid");', 'assert!(matches!(result, Err(ParseError::InvalidName { .. })), "Expected {name} to be invalid");')
    content = content.replace("assert!(matches!(SignalName::new(name), Ok(_)));", "SignalName::new(name).unwrap();")
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {id} to be invalid");', 'assert!(matches!(result, Err(ParseError::InvalidId { .. })));') # Wait, ParseError has InvalidName, InvalidUlid? We'll check the error variant later. Let's just use matches!(..., Err(_)) if no variant is specified, but wait! The prompt says "concrete assertions on the exact variant". I'll manually check variants for vo-api.
    
    with open(path, 'w') as f:
        f.write(content)

fix_api_tests()
