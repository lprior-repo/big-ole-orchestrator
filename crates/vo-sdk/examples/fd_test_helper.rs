use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::path::PathBuf;
use tempfile::TempDir;

struct Fd3Redirect {
    original_fd: libc::c_int,
    _temp_dir: TempDir,
}

impl Fd3Redirect {
    fn new(input_data: &[u8]) -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().join("input");

        let mut temp_file = File::create(&temp_path)?;
        temp_file.write_all(input_data)?;
        temp_file.flush()?;
        drop(temp_file);

        let original_fd = unsafe { libc::dup(3) };

        unsafe { libc::close(3) };

        let temp_file2 = std::fs::File::open(&temp_path)?;
        let temp_fd = temp_file2.into_raw_fd();

        if temp_fd != 3 {
            unsafe { libc::close(temp_fd) };
            if original_fd >= 0 {
                unsafe { libc::dup2(original_fd, 3) };
                unsafe { libc::close(original_fd) };
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Expected temp_fd=3, got {}", temp_fd),
            ));
        }

        Ok(Self {
            original_fd,
            _temp_dir: temp_dir,
        })
    }
}

impl Drop for Fd3Redirect {
    fn drop(&mut self) {
        if self.original_fd >= 0 {
            unsafe { libc::dup2(self.original_fd, 3) };
            unsafe { libc::close(self.original_fd) };
        }
    }
}

struct Fd4Redirect {
    original_fd: libc::c_int,
    _temp_dir: TempDir,
    write_path: PathBuf,
}

impl Fd4Redirect {
    fn new() -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let write_path = temp_dir.path().join("output");

        let write_file = std::fs::File::create(&write_path)?;
        let write_fd = write_file.into_raw_fd();

        let original_fd = unsafe { libc::dup(4) };

        if unsafe { libc::dup2(write_fd, 4) } < 0 {
            unsafe { libc::close(write_fd) };
            if original_fd >= 0 {
                unsafe { libc::close(original_fd) };
            }
            return Err(std::io::Error::last_os_error());
        }

        if write_fd != 4 {
            unsafe { libc::close(write_fd) };
        }

        Ok(Self {
            original_fd,
            _temp_dir: temp_dir,
            write_path,
        })
    }

    fn read_written(&self) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(&self.write_path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        Ok(contents)
    }
}

impl Drop for Fd4Redirect {
    fn drop(&mut self) {
        if self.original_fd >= 0 {
            unsafe { libc::dup2(self.original_fd, 4) };
            unsafe { libc::close(self.original_fd) };
        }
    }
}

fn create_valid_envelope(key: &str, data: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "idempotency_key": key,
        "data": data,
    }))
    .expect("test: serialization should not fail")
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: fd_test_helper <test_name>");
        std::process::exit(1);
    }

    let test_name = &args[1];
    let result = match test_name.as_str() {
        "read_input_with_fd3" => test_read_input_with_fd3(),
        "write_success_with_fd4" => test_write_success_with_fd4(),
        "write_failure_with_fd4" => test_write_failure_with_fd4(),
        "double_read_blocked_via_fd" => test_double_read_blocked_via_fd(),
        "double_write_blocked_via_fd" => test_double_write_blocked_via_fd(),
        _ => {
            eprintln!("Unknown test: {}", test_name);
            std::process::exit(1);
        }
    };

    match result {
        Ok(_) => {
            println!("PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("FAIL: {:?}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug)]
enum TestError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Sdk(vo_sdk::SdkError),
    Assertion(String),
}

impl From<std::io::Error> for TestError {
    fn from(e: std::io::Error) -> Self {
        TestError::Io(e)
    }
}
impl From<serde_json::Error> for TestError {
    fn from(e: serde_json::Error) -> Self {
        TestError::Json(e)
    }
}
impl From<vo_sdk::SdkError> for TestError {
    fn from(e: vo_sdk::SdkError) -> Self {
        TestError::Sdk(e)
    }
}

fn test_read_input_with_fd3() -> Result<(), TestError> {
    let input_data = create_valid_envelope("test-key", &serde_json::json!({"a": 1}));

    let _redirect = Fd3Redirect::new(&input_data)?;

    let result = vo_sdk::read_input()?;
    if result.idempotency_key().as_str() != "test-key" {
        return Err(TestError::Assertion(format!(
            "Expected idempotency_key 'test-key', got '{}'",
            result.idempotency_key().as_str()
        )));
    }
    if result.data() != &serde_json::json!({"a": 1}) {
        return Err(TestError::Assertion(format!(
            "Expected data {{'a': 1}}, got {}",
            result.data()
        )));
    }
    Ok(())
}

fn test_write_success_with_fd4() -> Result<(), TestError> {
    let redirect = Fd4Redirect::new()?;

    let output = serde_json::json!({"result": "ok", "value": 42});
    vo_sdk::write_success(&output)?;

    let written = redirect.read_written()?;
    let parsed: serde_json::Value = serde_json::from_slice(&written)?;
    if parsed["status"] != "success" {
        return Err(TestError::Assertion(format!(
            "Expected status 'success', got {}",
            parsed["status"]
        )));
    }
    if parsed["output"] != output {
        return Err(TestError::Assertion(format!(
            "Expected output {:?}, got {:?}",
            output, parsed["output"]
        )));
    }
    Ok(())
}

fn test_write_failure_with_fd4() -> Result<(), TestError> {
    let redirect = Fd4Redirect::new()?;

    vo_sdk::write_failure(vo_sdk::TaskFailureKind::User, "test error message")?;

    let written = redirect.read_written()?;
    let parsed: serde_json::Value = serde_json::from_slice(&written)?;
    if parsed["status"] != "failure" {
        return Err(TestError::Assertion(format!(
            "Expected status 'failure', got {}",
            parsed["status"]
        )));
    }
    if parsed["kind"] != "User" {
        return Err(TestError::Assertion(format!(
            "Expected kind 'User', got {}",
            parsed["kind"]
        )));
    }
    if parsed["message"] != "test error message" {
        return Err(TestError::Assertion(format!(
            "Expected message 'test error message', got {}",
            parsed["message"]
        )));
    }
    Ok(())
}

fn test_double_read_blocked_via_fd() -> Result<(), TestError> {
    let input_data = create_valid_envelope("test-key", &serde_json::json!({"a": 1}));
    let _redirect = Fd3Redirect::new(&input_data)?;

    let first = vo_sdk::read_input();
    if first.is_err() {
        return Err(TestError::Assertion(format!(
            "First read should succeed, got {:?}",
            first
        )));
    }

    let second = vo_sdk::read_input();
    if !matches!(second, Err(vo_sdk::SdkError::FdNotOpen)) {
        return Err(TestError::Assertion(format!(
            "Second read should fail with FdNotOpen, got {:?}",
            second
        )));
    }
    Ok(())
}

fn test_double_write_blocked_via_fd() -> Result<(), TestError> {
    let _redirect = Fd4Redirect::new()?;

    let first = vo_sdk::write_success(&serde_json::json!({"first": true}));
    if first.is_err() {
        return Err(TestError::Assertion(format!(
            "First write should succeed, got {:?}",
            first
        )));
    }

    let second = vo_sdk::write_success(&serde_json::json!({"second": true}));
    if !matches!(second, Err(vo_sdk::SdkError::AlreadyWritten)) {
        return Err(TestError::Assertion(format!(
            "Second write should fail with AlreadyWritten, got {:?}",
            second
        )));
    }
    Ok(())
}
