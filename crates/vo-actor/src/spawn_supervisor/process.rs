use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub pid: u32,
    pub executable: PathBuf,
    pub args: Vec<String>,
}

impl ProcessHandle {
    #[must_use]
    pub fn new(pid: u32, executable: PathBuf, args: Vec<String>) -> Self {
        Self {
            pid,
            executable,
            args,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_handle_new() {
        let handle = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec!["arg1".to_string()]);
        assert_eq!(handle.pid, 123);
        assert_eq!(handle.executable, PathBuf::from("/bin/true"));
        assert_eq!(handle.args, vec!["arg1"]);
    }

    #[test]
    fn process_handle_debug() {
        let handle = ProcessHandle::new(456, PathBuf::from("/bin/false"), vec![]);
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("ProcessHandle"));
        assert!(debug_str.contains("456"));
    }
}
