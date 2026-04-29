use vo_cli::{dispatch, interpret_cli_from};

#[tokio::test]
async fn e2e_rebuild_list_dispatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let path = dir.path().to_str().expect("path");

    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--project-dir", path, "--list"]).expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_rebuild_not_initialized_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        path,
        "--projection-id",
        "p1",
    ])
    .expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_rebuild_with_projection_id_dispatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        path,
        "--projection-id",
        "orders",
    ])
    .expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_ok());
}
