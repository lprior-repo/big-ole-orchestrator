#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub pid: u32,
    pub command: String,
}

impl ProcessHandle {
    #[must_use]
    pub fn new(pid: u32, command: String) -> Self {
        Self { pid, command }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_handle_new() {
        let handle = ProcessHandle::new(123, "test command".to_string());
        assert_eq!(handle.pid, 123);
        assert_eq!(handle.command, "test command");
    }

    #[test]
    fn process_handle_debug() {
        let handle = ProcessHandle::new(456, "debug test".to_string());
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("ProcessHandle"));
        assert!(debug_str.contains("456"));
    }
}