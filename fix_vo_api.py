import re

def fix_tests():
    with open("crates/vo-api/src/types/tests.rs", "r") as f:
        content = f.read()

    # Clean up accidental double matches from previous script
    content = content.replace("assert!(matches!(matches!(", "assert!(matches!(")
    content = content.replace("Ok(_)));", "Ok(_)));\n").replace(")));\n", ")));")

    # WorkflowName
    content = content.replace("assert!(matches!(WorkflowName::new(name), Ok(_)));", "WorkflowName::new(name).unwrap();")
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {name} to be invalid");', 'assert!(matches!(result, Err(crate::types::errors::ParseError::EmptyWorkflowName) | Err(crate::types::errors::ParseError::InvalidWorkflowNameFormat)), "Expected {name} to be invalid");')
    
    # SignalName
    content = content.replace("assert!(matches!(SignalName::new(name), Ok(_)));", "SignalName::new(name).unwrap();")
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {name} to be invalid");', 'assert!(matches!(result, Err(crate::types::errors::ParseError::EmptySignalName) | Err(crate::types::errors::ParseError::InvalidSignalNameFormat)), "Expected {name} to be invalid");')

    # InvocationId
    content = content.replace("assert!(matches!(result, Ok(_)),", "assert!(result.is_ok(),")
    content = content.replace('assert!(result.is_ok(), "Valid ULID should pass");', 'result.unwrap();')
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {id} to be invalid");', 'assert!(matches!(result, Err(crate::types::errors::ParseError::InvalidUlidFormat)), "Expected {id} to be invalid");')

    # Timestamp
    content = content.replace('assert!(matches!(result, Ok(_)), "Expected {ts} to be valid");', 'result.unwrap();')
    content = content.replace('assert!(matches!(result, Err(_)), "Expected {ts} to be invalid");', 'assert!(matches!(result, Err(crate::types::errors::ParseError::InvalidTimestampFormat)), "Expected {ts} to be invalid");')

    # RetryAfterSeconds
    content = content.replace('assert!(matches!(result, Err(_)));', 'assert!(matches!(result, Err(crate::types::errors::ValidationError::InvalidRetryAfterSeconds)));')

    # WorkflowStatus validate
    content = content.replace("assert!(matches!(status_before.validate(), Err(_)));", "assert!(matches!(status_before.validate(), Err(crate::types::errors::InvariantViolation::UpdatedBeforeStarted)));")
    content = content.replace("assert!(matches!(status_after.validate(), Ok(_)));", "status_after.validate().unwrap();")

    # JournalResponse validate
    content = content.replace("assert!(matches!(unsorted.validate(), Err(_)));", "assert!(matches!(unsorted.validate(), Err(crate::types::errors::InvariantViolation::EntriesNotSorted)));")
    content = content.replace("assert!(matches!(sorted.validate(), Ok(_)));", "sorted.validate().unwrap();")

    # ErrorResponse
    content = content.replace('assert!(matches!(err, Ok(_)),', 'assert!(err.is_ok(),')
    content = content.replace('assert!(err.is_ok(), "at_capacity with retry should be ok");', 'err.unwrap();')
    content = content.replace('assert!(matches!(err, Err(_)), "at_capacity without retry should fail");', 'assert!(matches!(err, Err(crate::types::errors::InvariantViolation::InvalidRetryForErrorType)), "at_capacity without retry should fail");')
    content = content.replace('assert!(err.is_ok(), "not_found without retry should be ok");', 'err.unwrap();')
    content = content.replace('assert!(matches!(err, Err(_)), "not_found with retry should fail");', 'assert!(matches!(err, Err(crate::types::errors::InvariantViolation::InvalidRetryForErrorType)), "not_found with retry should fail");')

    # StartWorkflowResponse validate
    content = content.replace("assert!(matches!(resp.validate(), Ok(_)));", "resp.validate().unwrap();")
    content = content.replace("assert!(matches!(resp.validate(), Err(_)));", "assert!(matches!(resp.validate(), Err(crate::types::errors::InvariantViolation::InvalidStatusForResponse)));")

    with open("crates/vo-api/src/types/tests.rs", "w") as f:
        f.write(content)

    # red_queen_tests.rs
    with open("crates/vo-types/src/red_queen_tests.rs", "r") as f:
        rq = f.read()
    
    rq = rq.replace("    assert!(matches!(result, Err(_)));\n    assert!(matches!(\n        result,\n        Err(RetryPolicyError::InvalidMultiplier { .. })\n    ));", "    assert!(matches!(\n        result,\n        Err(RetryPolicyError::InvalidMultiplier { .. })\n    ));")
    rq = rq.replace("    assert!(matches!(result, Err(_)));", "    assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })));")
    rq = rq.replace('prop_assert!(matches!(result, Err(_)), "multiplier {} should be rejected", multiplier);', 'prop_assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "multiplier {} should be rejected", multiplier);')

    with open("crates/vo-types/src/red_queen_tests.rs", "w") as f:
        f.write(rq)

fix_tests()
