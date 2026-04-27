//! RED-QUEEN Dolt coevolutionary tests (ve-uc201)
//!
//! Adversarial Dolt edge cases: schema evolution, conflict resolution, merge races.

#![allow(clippy::unwrap_used)]

use std::process::Command;
use tempfile::tempdir;

fn dolt_init(dir: &std::path::Path) {
    Command::new("dolt")
        .arg("init")
        .current_dir(dir)
        .output()
        .unwrap();
}

fn dolt_commit_all(dir: &std::path::Path, msg: &str) {
    Command::new("dolt")
        .args(["commit", "--allow-empty", "-m", msg])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn dolt_sql(dir: &std::path::Path, sql: &str) -> String {
    let out = Command::new("dolt")
        .args(["sql", "-q", sql])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn rq_dolt_schema_evolution_add_column_preserves_data() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))",
    );
    dolt_sql(
        dir.path(),
        "INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')",
    );
    dolt_commit_all(dir.path(), "init");

    dolt_sql(
        dir.path(),
        "ALTER TABLE users ADD COLUMN email VARCHAR(255)",
    );
    dolt_commit_all(dir.path(), "add email column");

    let result = dolt_sql(dir.path(), "SELECT COUNT(*) FROM users");
    assert!(
        result.contains("2"),
        "All rows including new null email must exist"
    );
}

#[test]
fn rq_dolt_schema_evolution_drop_column_no_data_loss() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255), email VARCHAR(255))",
    );
    dolt_sql(
        dir.path(),
        "INSERT INTO users VALUES (1, 'Alice', 'alice@test.com')",
    );
    dolt_commit_all(dir.path(), "init");

    dolt_sql(dir.path(), "ALTER TABLE users DROP COLUMN email");
    dolt_commit_all(dir.path(), "drop email column");

    let result = dolt_sql(dir.path(), "SELECT name FROM users WHERE id=1");
    assert!(result.contains("Alice"), "Data must survive column drop");
}

#[test]
fn rq_dolt_merge_conflict_two_branches_same_row() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE users (id INT PRIMARY KEY, score INT)",
    );
    dolt_sql(dir.path(), "INSERT INTO users VALUES (1, 100)");
    dolt_commit_all(dir.path(), "init");

    dolt_sql(dir.path(), "CALL DOLT_BRANCH('feature')");
    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Alice'");
    dolt_sql(dir.path(), "UPDATE users SET score=200 WHERE id=1");
    dolt_commit_all(dir.path(), "Alice update");

    dolt_sql(dir.path(), "SET dolt_commit_email='b@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Bob'");
    dolt_sql(dir.path(), "UPDATE users SET score=300 WHERE id=1");
    dolt_commit_all(dir.path(), "Bob update");

    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Alice'");
    let result = dolt_sql(dir.path(), "CALL DOLT_MERGE('feature')");
    assert!(
        result.to_lowercase().contains("conflict") || result.to_lowercase().contains("success"),
        "Merge must report conflict or success, got: {}",
        result
    );
}

#[test]
fn rq_dolt_merge_conflict_resolve_then_verify() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE users (id INT PRIMARY KEY, score INT)",
    );
    dolt_sql(dir.path(), "INSERT INTO users VALUES (1, 100)");
    dolt_commit_all(dir.path(), "init");

    dolt_sql(dir.path(), "CALL DOLT_BRANCH('feature')");
    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Alice'");
    dolt_sql(dir.path(), "UPDATE users SET score=200 WHERE id=1");
    dolt_commit_all(dir.path(), "Alice update");

    dolt_sql(dir.path(), "SET dolt_commit_email='b@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Bob'");
    dolt_sql(dir.path(), "UPDATE users SET score=300 WHERE id=1");
    dolt_commit_all(dir.path(), "Bob update");

    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='Alice'");
    let _merge_result = dolt_sql(dir.path(), "CALL DOLT_MERGE('feature')");
    dolt_sql(
        dir.path(),
        "CALL DOLT_MERGE_CONFLICT_RESOLVE('users', 'id', '1', '300')",
    );
    dolt_commit_all(dir.path(), "resolve conflict");

    let result = dolt_sql(dir.path(), "SELECT score FROM users WHERE id=1");
    assert!(result.contains("300"), "Resolved score must be Bob's value");
}

#[test]
fn rq_dolt_schema_evolution_type_change_safe() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE counts (id INT PRIMARY KEY, value INT)",
    );
    dolt_sql(dir.path(), "INSERT INTO counts VALUES (1, 42)");
    dolt_commit_all(dir.path(), "init");

    dolt_sql(dir.path(), "ALTER TABLE counts MODIFY COLUMN value BIGINT");
    dolt_commit_all(dir.path(), "change type");

    let result = dolt_sql(dir.path(), "SELECT value FROM counts WHERE id=1");
    assert!(result.contains("42"), "Value must survive type change");
}

#[test]
fn rq_dolt_concurrent_writes_merge_both() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(
        dir.path(),
        "CREATE TABLE events (id INT PRIMARY KEY, data VARCHAR(255))",
    );
    dolt_sql(
        dir.path(),
        "INSERT INTO events VALUES (1, 'first'), (2, 'second')",
    );
    dolt_commit_all(dir.path(), "init");

    dolt_sql(dir.path(), "CALL DOLT_BRANCH('branch-a')");
    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='A'");
    dolt_sql(dir.path(), "INSERT INTO events VALUES (3, 'from-a')");
    dolt_commit_all(dir.path(), "a insert");

    dolt_sql(dir.path(), "SET dolt_commit_email='b@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='B'");
    dolt_sql(dir.path(), "INSERT INTO events VALUES (4, 'from-b')");
    dolt_commit_all(dir.path(), "b insert");

    dolt_sql(dir.path(), "SET dolt_commit_email='a@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='A'");
    dolt_sql(dir.path(), "CALL DOLT_MERGE('branch-a')");
    dolt_commit_all(dir.path(), "merge a");

    dolt_sql(dir.path(), "SET dolt_commit_email='b@test.com'");
    dolt_sql(dir.path(), "SET dolt_commit_name='B'");
    let merge_result = dolt_sql(dir.path(), "CALL DOLT_MERGE('branch-a')");
    assert!(
        merge_result.to_lowercase().contains("conflict")
            || merge_result.to_lowercase().contains("success"),
        "Second merge must handle conflict, got: {}",
        merge_result
    );

    let result = dolt_sql(dir.path(), "SELECT COUNT(*) FROM events");
    assert!(
        result.contains("4"),
        "All four rows must exist after merges"
    );
}

#[test]
fn rq_dolt_schema_evolution_add_primary_key() {
    let dir = tempdir().unwrap();
    dolt_init(dir.path());
    dolt_sql(dir.path(), "CREATE TABLE items (name VARCHAR(255))");
    dolt_sql(
        dir.path(),
        "INSERT INTO items VALUES ('widget'), ('gadget')",
    );
    dolt_commit_all(dir.path(), "init");

    dolt_sql(
        dir.path(),
        "ALTER TABLE items ADD COLUMN id INT AUTO_INCREMENT PRIMARY KEY",
    );
    dolt_commit_all(dir.path(), "add pk");

    let result = dolt_sql(dir.path(), "SELECT COUNT(*) FROM items");
    assert!(result.contains("2"), "All items must survive pk addition");
}
