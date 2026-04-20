use std::path::PathBuf;
use vo_ipc::{run_subprocess, Fd4Envelope, IpcError, SubprocessConfig, TaskResult};

#[tokio::test]
async fn fd4_huge_length_no_payload_should_be_error() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd4.py");

    let config = SubprocessConfig::new(path, 1000, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::Fd4ReadFailed { .. }) => {
            // Success: handled the error
        }
        Ok(_) => {
            panic!("Should have failed to read fd4");
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn fd4_length_too_large_should_not_oom() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd4.py");

    // We'll use a modified python script that writes a length larger than max usize if possible
    // but here we already have 1GB.
    let config = SubprocessConfig::new(path, 1000, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;
    // If it OOMs, this test will just crash or the runner will kill it.
    assert!(matches!(result, Err(IpcError::Fd4ReadFailed { .. })));
}

/// RED QUEEN test: IPC message ordering attack via partial input.
///
/// This test verifies that when a malicious subprocess responds based on PARTIAL
/// input (reading only the length header and a few bytes before responding),
/// the IPC layer correctly handles the response.
///
/// The subprocess reads only 5 bytes (4-byte length + 1-byte payload) of a
/// larger request, then immediately sends a response and exits WITHOUT reading
/// the rest of the request.
///
/// Attack scenario:
/// 1. Parent sends a large request to subprocess via fd3
/// 2. Subprocess reads partial data and immediately responds via fd4
/// 3. Parent receives the response (which was generated from partial input)
/// 4. Parent's subsequent write to fd3 gets broken pipe (subprocess exited)
///
/// The key security property we verify:
/// - The response is valid and parseable
/// - But the response's identity fields will NOT match expected values
/// - This is caught by validate_identity() at a higher level
///
/// This test documents the current behavior and verifies the IPC layer
/// doesn't crash or panic when handling such an adversarial scenario.
#[tokio::test]
async fn fd4_partial_input_response_is_handled_correctly() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd4_partial.py");

    // Large payload that subprocess will only partially read
    let large_payload = b"test_payload_for_ordering_attack";
    let config = SubprocessConfig::new(path, 1000, large_payload.to_vec()).unwrap();
    let result = run_subprocess(config).await;

    // The subprocess WILL successfully send a response (based on partial input)
    // The IPC layer handles this gracefully - no crash, no panic
    match result {
        Ok(output) => {
            // Verify we got some response data
            assert!(
                !output.fd4_bytes.is_empty(),
                "Should receive response from partial-input attack"
            );

            // Try to parse the response as an Fd4Envelope
            let response_result = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);

            // The response IS valid JSON and IS a valid Fd4Envelope structure
            // But the instance_id will be "adversary-response" not the expected value
            assert!(
                response_result.is_ok(),
                "Response should be parseable as Fd4Envelope"
            );

            let response = response_result.unwrap();
            // Verify this is our adversarial response (not a legitimate one)
            assert_eq!(response.instance_id, "adversary-response");
            assert_eq!(response.node_id, "partial-input-attack");

            // The response indicates it only read partial data
            if let TaskResult::Success { output } = response.result {
                let output_str = output.as_str().unwrap_or("");
                assert!(
                    output_str.contains("partial_input_response"),
                    "Response should indicate partial input attack"
                );
                assert!(
                    output_str.contains("attack"),
                    "Response should document the attack"
                );
            } else {
                panic!("Expected success result from adversarial subprocess");
            }
        }
        Err(e) => {
            // If we get an error, it's still acceptable - the IPC layer handled it
            // But ideally we should receive the response to verify the attack worked
            panic!(
                "IPC layer should handle partial-input attack gracefully: {:?}",
                e
            );
        }
    }
}

/// RED QUEEN test: Verify that identity validation catches adversarial responses.
///
/// When a subprocess sends a response based on partial input, the response's
/// identity fields (instance_id, node_id) will be wrong. The validate_identity()
/// function should catch this at the caller's level.
///
/// This test verifies the identity mismatch is detectable.
#[test]
fn fd4_response_identity_mismatch_is_detectable() {
    use vo_ipc::envelope::validate_identity;
    use vo_ipc::Fd4Envelope;
    use vo_ipc::TaskResult;

    // Craft a response with WRONG identity (simulating adversarial response)
    let malicious_response = Fd4Envelope {
        version: 1,
        instance_id: "adversary-response".to_string(),
        node_id: "partial-input-attack".to_string(),
        result: TaskResult::Success {
            output: serde_json::json!({"attack": true}),
        },
    };

    // Validate against EXPECTED identity
    let result = validate_identity(&malicious_response, "expected-instance", "expected-node");

    // The validation SHOULD fail because instance_id and node_id don't match
    assert!(
        result.is_err(),
        "Identity validation should detect adversarial response with wrong identity"
    );

    // Verify the error message contains useful information
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("adversary-response"));
    assert!(format!("{}", err).contains("expected-instance"));
}

#[tokio::test]
async fn fd3_burst_write_handled_gracefully() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd3_burst.py");

    let config = SubprocessConfig::new(path, 2000, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            assert!(output.fd4_bytes.is_empty());
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            assert_eq!(exit_code, 42);
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn fd4_burst_write_handled_gracefully() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd4_burst.py");

    let config = SubprocessConfig::new(path, 2000, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::Fd4ReadFailed { .. }) => {}
        Ok(_) => panic!("Should have failed reading huge fd4 payload"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn immediate_exit_ignores_ipc() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_immediate_exit.py");

    let config = SubprocessConfig::new(path, 500, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            assert!(output.fd4_bytes.is_empty());
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            assert_eq!(exit_code, 0);
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn fd4_closed_before_read_returns_error() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd4_closed.py");

    let config = SubprocessConfig::new(path, 500, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::Fd4ReadFailed { .. }) => {}
        Ok(_) => panic!("Should have failed reading from closed fd4"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn fd3_closed_before_read_handled() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_fd3_closed.py");

    let config = SubprocessConfig::new(path, 500, b"test".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            assert!(!output.fd4_bytes.is_empty());
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn partial_write_recovery_works() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/adversary_partial_write.py");

    let payload = b"test_payload_for_partial_write_recovery";
    let config = SubprocessConfig::new(path, 1000, payload.to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            assert!(!output.fd4_bytes.is_empty());
            let response_result = serde_json::from_slice::<Fd4Envelope>(&output.fd4_bytes);
            assert!(response_result.is_ok(), "Response should be parseable");
            let response = response_result.unwrap();
            assert_eq!(response.instance_id, "partial-write-test");
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
