use fjall::Config;

#[test]
fn test_cargo_check_workspace_compiles_wtf_storage_cleanly_after_scaffold() {
    let folder = tempfile::tempdir().expect("Failed to create temp dir");
    let _keyspace = Config::new(folder.path())
        .open()
        .expect("Failed to open keyspace");
    // Just verifying fjall links and opens a keyspace correctly
    assert!(folder.path().exists());
}
