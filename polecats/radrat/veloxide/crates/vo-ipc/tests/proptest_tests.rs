#![allow(clippy::redundant_pattern_matching)]
use proptest::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;
use vo_ipc::SubprocessConfig;

fn executable_file() -> std::path::PathBuf {
    let directory = tempdir().unwrap();
    let file = directory.path().join("fixture.sh");
    fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&file, permissions).unwrap();
    let path = file.clone();
    // Safety: leaking directory to keep path valid during test
    std::mem::forget(directory);
    path
}

proptest! {
    #[test]
    fn subprocess_config_new_accepts_any_valid_timeout(t in 1..u64::MAX) {
        let path = executable_file();
        let result = SubprocessConfig::new(&path, t, vec![]);
        prop_assert!(matches!(result, Ok(_)));
        prop_assert_eq!(result.unwrap().timeout_ms(), t);
    }
}
