//! BLACK-HAT hardline adversarial tests — injection, overflow, race conditions.
//! Every CLI command attacked at the parser boundary.

use std::ffi::OsString;
use std::sync::Arc;
use std::thread;
use vo_cli::cli::{interpret_cli_from, Command};

fn ovec(s: &[&str]) -> Vec<OsString> { s.iter().map(|x| OsString::from(*x)).collect() }

macro_rules! parsed {
    ($args:expr, $pat:pat => $body:expr) => {
        match &interpret_cli_from($args).unwrap().command { $pat => $body, _ => panic!("wrong cmd") }
    };
}

#[test]
fn overflow_instance_id_purge() {
    let long = "A".repeat(65536);
    parsed!(ovec(&["vo", "purge", "--instance", &long]),
        Command::Purge { instance } => assert_eq!(instance.len(), 65536));
}

#[test]
fn overflow_engine_url_status() {
    let url = "http://".to_string() + &"a".repeat(65536);
    parsed!(ovec(&["vo", "status", "i", "--engine-url", &url]),
        Command::Status { engine_url, .. } => assert!(engine_url.len() > 65536));
}

#[test]
fn overflow_workflow_id_compensate() {
    let id = "\u{202E}".repeat(8192);
    parsed!(ovec(&["vo", "compensate", &id]),
        Command::Compensate { workflow_id, .. } => assert!(workflow_id.contains('\u{202E}')));
}

#[test]
fn overflow_path_check() {
    let deep = (0..100).map(|_| "..").collect::<Vec<_>>().join("/");
    parsed!(ovec(&["vo", "check", &deep]),
        Command::Check { path } => assert!(path.to_str().unwrap().contains("..")));
}

#[test]
fn overflow_init_all_fields() {
    let big = "X".repeat(32768);
    parsed!(ovec(&["vo", "init", "--project-dir", &big, "--storage-path", &big, "--engine-url", &big]),
        Command::Init { project_dir, engine_url, storage_path } => {
            assert_eq!(project_dir.to_str().unwrap().len(), 32768);
            assert_eq!(engine_url.len(), 32768);
            assert_eq!(storage_path.to_str().unwrap().len(), 32768);
        });
}

#[test]
fn crlf_injection_in_engine_url() {
    for p in &["http://x\r\nX-Injected: true", "http://evil\nHost: x", "http://x\tH: p"] {
        parsed!(ovec(&["vo", "gc", "--engine-url", p]),
            Command::Gc { engine_url, .. } => assert!(engine_url.contains('\r') || engine_url.contains('\n') || engine_url.contains('\t')));
    }
}

#[test]
fn ssrf_urls_parse_cleanly() {
    for u in &["http://169.254.169.254/latest/meta-data/", "http://[::1]:3000", "gopher://x:25/"] {
        parsed!(ovec(&["vo", "status", "i", "--engine-url", u]),
            Command::Status { engine_url, .. } => assert_eq!(engine_url, u));
    }
}

#[test]
fn path_traversal_doctor() {
    for p in &["/dev/null", "/proc/self/environ", "/tmp/../../../etc/shadow", "."] {
        parsed!(ovec(&["vo", "doctor", "--project-dir", p]),
            Command::Doctor { project_dir } => assert_eq!(project_dir, std::path::Path::new(p)));
    }
}

#[test]
fn rebuild_sql_injection() {
    let id = "'; DROP TABLE projections; --";
    parsed!(ovec(&["vo", "rebuild", "--projection-id", id]),
        Command::Rebuild { projection_id, .. } => assert_eq!(projection_id.as_deref(), Some(id)));
}

#[test]
fn concurrent_parse_no_panic() {
    let sets = Arc::new((0..64).map(|i| ovec(&["vo", "purge", "--instance", &format!("race-{i}")])).collect::<Vec<_>>());
    let hs: Vec<_> = (0..8).map(|t| { let s = Arc::clone(&sets); thread::spawn(move || {
        for i in 0..8 { parsed!(s[t*8+i].clone(), Command::Purge { instance } => assert!(instance.starts_with("race-"))); }
    })}).collect();
    for h in hs { h.join().unwrap(); }
}

#[test]
fn concurrent_mixed_subcommands() {
    let cases = Arc::new(vec![
        ovec(&["vo", "purge", "--instance", "a"]), ovec(&["vo", "status", "b"]),
        ovec(&["vo", "gc"]), ovec(&["vo", "check", "/tmp"]),
        ovec(&["vo", "compensate", "wf-1"]), ovec(&["vo", "init"]),
        ovec(&["vo", "lock"]), ovec(&["vo", "doctor"]),
        ovec(&["vo", "rebuild"]), ovec(&["vo", "rebuild", "--list", "--force"]),
    ]);
    let hs: Vec<_> = (0..4).map(|_| { let c = Arc::clone(&cases); thread::spawn(move || {
        for a in c.iter() { interpret_cli_from(a.clone()).unwrap(); }
    })}).collect();
    for h in hs { h.join().unwrap(); }
}

#[test]
fn empty_string_value() {
    parsed!(ovec(&["vo", "purge", "--instance", ""]),
        Command::Purge { instance } => assert!(instance.is_empty()));
}

#[test]
fn whitespace_only_instance() {
    for id in &[" ", "\t", "\n", "  \n\t  "] {
        parsed!(ovec(&["vo", "status", id]),
            Command::Status { instance, .. } => assert!(instance.trim().is_empty()));
    }
}

#[test]
fn control_chars_in_fields() {
    parsed!(ovec(&["vo", "purge", "--instance", "\x00\x01\x1F\x7F"]),
        Command::Purge { instance } => assert!(instance.contains('\x00')));
}

#[test]
fn duplicate_positional_rejected() {
    assert!(interpret_cli_from(ovec(&["vo", "status", "first", "second"])).is_err());
}
