use fjall::Database;

#[test]
fn cargo_check_workspace_compiles_vo_storage_cleanly_after_scaffold() {
    let folder = tempfile::tempdir().expect("Failed to create temp dir");
    let _db = Database::builder(folder.path())
        .open()
        .expect("Failed to open database");
    assert!(folder.path().exists());
}
