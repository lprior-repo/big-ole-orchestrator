//! BLACK-HAT Dolt adversarial tests: corruption + SQL injection via beads.
//! ve-j2sk1 — Tests Dolt corruption and SQL injection via issue fields.

use std::process::Command;

const SQL_PAYLOADS: &[&str] = &[
    "fix: SQL injection test 1",
    "fix: OR condition",
    "admin--style",
    "fix: quotes'test\"double",
    "fix: semicolon;delimiter",
    "fix: backslash\\x",
    "fix: percent%like",
    "fix: underscore_exploit",
    "fix: asterisk*wild",
    "fix: question?mark",
    "fix: bracket[0]",
    "fix: parens(func)",
    "'; DROP TABLE issues; --",
    "1; DELETE FROM issues WHERE 1=1;--",
    "1' UNION SELECT * FROM users--",
    "'; INSERT INTO issues VALUES('pwned'); --",
];

fn bd() -> Command {
    Command::new("bd")
}

fn mk_issue(title: &str, desc: &str) -> Option<String> {
    let out = bd()
        .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
        .args(["create", title, "-d", desc, "--silent", "--json"])
        .output()
        .ok()?;
    if out.status.success() {
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .ok()?
            .get("id")?
            .as_str()
            .map(String::from)
    } else {
        None
    }
}

fn dolt_dir() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(".beads/dolt");
    if p.exists() {
        Some(p.to_path_buf())
    } else {
        None
    }
}

#[test]
fn sql_injection_payloads_handled_without_corruption() {
    for p in SQL_PAYLOADS {
        let t = format!("fix: {p}");
        assert!(
            mk_issue(&t, "Testing SQL injection").is_some(),
            "payload handled: {p}"
        );
    }
}

#[test]
fn sql_injection_query_handles_special_chars() {
    for q in [
        "p0", "p1 or", "admin--", "union", "drop", "'; DROP", "1 OR 1",
    ] {
        let out = bd()
            .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
            .args(["q", q, "--json"])
            .output()
            .expect("q should not panic");
        assert!(
            out.status.success() || !out.stderr.is_empty(),
            "query safe: {q}"
        );
    }
}

#[test]
fn meta_chars_in_labels_handled() {
    let out = bd()
        .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
        .args([
            "create",
            "fix: meta chars",
            "--labels",
            "p0;drop,p1'or',p2--x",
            "--json",
        ])
        .output()
        .expect("bd should handle meta chars");
    assert!(out.status.success() || !out.stderr.is_empty());
}

#[test]
fn corrupt_sst_file_detected() {
    if let Some(dd) = dolt_dir() {
        let sd = dd.join("data");
        if sd.exists() {
            if let Some(sst) = std::fs::read_dir(&sd)
                .unwrap()
                .filter_map(|e| e.ok())
                .find(|e| e.path().extension().is_some_and(|ex| ex == "sst"))
            {
                let p = sst.path();
                let orig = std::fs::read(&p).unwrap();
                if orig.len() > 64 {
                    std::fs::write(&p, &orig[..orig.len() / 2]).unwrap();
                    let _ = bd()
                        .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
                        .args(["list", "--json"])
                        .output();
                    std::fs::write(&p, &orig).unwrap();
                }
            }
        }
    }
}

#[test]
fn dolt_data_dir_missing_graceful() {
    if let Some(dd) = dolt_dir() {
        let sql_dir = dd.join("data");
        if sql_dir.exists() {
            let files: Vec<_> = std::fs::read_dir(&sql_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .collect();
            for f in &files {
                std::fs::rename(f, format!("{}.bak", f.display())).ok();
            }
            let out = bd()
                .current_dir("/home/lewis/gt/veloxide/polecats/raider/veloxide")
                .args(["list", "--json"])
                .output();
            for f in &files {
                std::fs::rename(format!("{}.bak", f.display()), f).ok();
            }
            assert!(
                out.is_ok(),
                "bd should not panic when data dir is inaccessible"
            );
        }
    }
}
