//! FD Mock framework for integration testing with real file descriptors.
//!
//! This module provides utilities to mock FD3 (read) and FD4 (write) for
//! integration testing the full `read_input`, `write_success`, and `write_failure`
//! code paths that go through `is_fd_valid` and `from_raw_fd`.

use std::fs::File;
use std::io::{Read, Seek, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct Fd3Redirect {
    original_fd: libc::c_int,
    _temp_file: Option<File>,
    _temp_dir: TempDir,
}

impl Fd3Redirect {
    pub fn new(input_data: &[u8]) -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().join("input");

        let mut temp_file = File::create(&temp_path)?;
        temp_file.write_all(input_data)?;
        temp_file.flush()?;
        temp_file.seek(std::io::SeekFrom::Start(0))?;

        let original_fd = unsafe { libc::dup(3) };

        let temp_fd = temp_file.as_raw_fd();
        if unsafe { libc::dup2(temp_fd, 3) } < 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(Self {
            original_fd,
            _temp_file: Some(temp_file),
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

pub struct Fd4Redirect {
    original_fd: libc::c_int,
    _temp_dir: TempDir,
    write_path: PathBuf,
}

impl Fd4Redirect {
    pub fn new() -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let write_path = temp_dir.path().join("output");

        let original_fd = unsafe { libc::dup(4) };

        let write_file = File::create(&write_path)?;
        let write_fd = write_file.as_raw_fd();

        if unsafe { libc::dup2(write_fd, 4) } < 0 {
            return Err(std::io::Error::last_os_error());
        }

        std::mem::forget(write_file);

        Ok(Self {
            original_fd,
            _temp_dir: temp_dir,
            write_path,
        })
    }

    pub fn read_written(&self) -> std::io::Result<Vec<u8>> {
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

pub fn create_valid_envelope(key: &str, data: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "idempotency_key": key,
        "data": data,
    }))
    .expect("test: serialization should not fail")
}

pub fn create_invalid_json_envelope() -> Vec<u8> {
    b"not valid json".to_vec()
}

pub fn create_missing_key_envelope() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "wrong_key": "value"
    }))
    .expect("test: serialization should not fail")
}
