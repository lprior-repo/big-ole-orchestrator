use std::path::PathBuf;
use vo_ipc::{run_subprocess, IpcError, SubprocessConfig};

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
