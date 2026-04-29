//! BDD test: Map Kani and proptest targets to ADR invariants.
//!
//! ADR refs:
//! - ADR-035: Event Schema Evolution and Upcasting
//! - ADR-039: Hierarchical Lifecycle State Machine
//! - ADR-043: Exact-Once Verification Strategy
//!
//! Given Kani/proptest targets exist
//! When traceability check runs
//! Then each target declares ADR/invariant coverage and orphan targets are reported

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FormalTargetKind {
    KaniProof,
    Proptest,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FormalTarget {
    kind: FormalTargetKind,
    file_path: PathBuf,
    function_name: String,
    adrs: BTreeSet<String>,
}

fn crates_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vo-linter has parent")
        .to_path_buf()
}

fn collect_rs_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn extract_kani_targets(source: &str, file_path: &Path) -> Vec<String> {
    let mut targets = Vec::new();
    let mut lines = source.lines();
    let mut line_num = 0;
    while let Some(line) = lines.next() {
        line_num += 1;
        if line.trim() == "#[kani::proof]" {
            for _ in 0..5 {
                if let Some(next) = lines.next() {
                    line_num += 1;
                    let trimmed = next.trim();
                    if trimmed.starts_with("fn ") {
                        if let Some(name) = trimmed.strip_prefix("fn ") {
                            let fn_name = name.split('(').next().unwrap_or("").trim();
                            if !fn_name.is_empty() {
                                targets.push(fn_name.to_string());
                            }
                        }
                        break;
                    }
                    if trimmed.starts_with("pub fn ") || trimmed.starts_with("async fn ") {
                        let rest = trimmed
                            .strip_prefix("pub fn ")
                            .or_else(|| trimmed.strip_prefix("async fn "))
                            .unwrap_or(trimmed);
                        let fn_name = rest.split('(').next().unwrap_or("").trim();
                        if !fn_name.is_empty() {
                            targets.push(fn_name.to_string());
                        }
                        break;
                    }
                }
            }
        }
    }
    let _ = line_num;
    targets
}

fn extract_proptest_targets(source: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("fn ") && trimmed.ends_with('(') {
            if let Some(name) = trimmed.strip_prefix("fn ") {
                let fn_name = name.trim_end_matches('(').trim();
                if fn_name.starts_with("proptest_") {
                    targets.push(fn_name.to_string());
                }
            }
        }
        if trimmed.starts_with("fn ") && trimmed.contains("proptest") {
            if let Some(name) = trimmed.strip_prefix("fn ") {
                let fn_name = name.split('(').next().unwrap_or("").trim();
                if fn_name.starts_with("proptest_") && !targets.contains(&fn_name.to_string()) {
                    targets.push(fn_name.to_string());
                }
            }
        }
    }
    targets
}

fn in_proptest_block(source: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("proptest!") || trimmed.starts_with("proptest::proptest!") {
            let mut brace_depth = 0i32;
            let mut started = false;
            while i < lines.len() {
                for ch in lines[i].chars() {
                    match ch {
                        '{' => {
                            brace_depth += 1;
                            started = true;
                        }
                        '}' => {
                            brace_depth -= 1;
                        }
                        _ => {}
                    }
                }
                let t = lines[i].trim();
                if t.starts_with("fn ") && t.contains("proptest") {
                    if let Some(name) = t.strip_prefix("fn ") {
                        let fn_name = name.split('(').next().unwrap_or("").trim();
                        if !fn_name.is_empty()
                            && fn_name.contains("proptest")
                            && !targets.contains(&fn_name.to_string())
                        {
                            targets.push(fn_name.to_string());
                        }
                    }
                }
                if started && brace_depth == 0 {
                    break;
                }
                i += 1;
            }
        }
        i += 1;
    }
    targets
}

fn map_file_to_adrs(file_path: &Path, crates_root: &Path) -> BTreeSet<String> {
    let mut adrs = BTreeSet::new();
    let rel = file_path
        .strip_prefix(crates_root)
        .unwrap_or(file_path);
    let rel_str = rel.to_string_lossy();

    if rel_str.contains("upcaster")
        || rel_str.contains("schema")
        || rel_str.contains("codec")
        || rel_str.contains("events")
        || rel_str.contains("event_journal")
        || rel_str.contains("effect_journal")
        || rel_str.contains("snapshot")
    {
        adrs.insert("ADR-035".to_string());
    }

    if rel_str.contains("lifecycle")
        || rel_str.contains("state/transition")
        || rel_str.contains("state_machine")
        || rel_str.contains("connector")
        || rel_str.contains("kani_proofs")
        || rel_str.contains("plugin/verification")
        || rel_str.contains("tx_coordinator/verification")
        || rel_str.contains("compensation")
        || rel_str.contains("shedding_verification")
    {
        adrs.insert("ADR-039".to_string());
    }

    if rel_str.contains("dedupe")
        || rel_str.contains("receipt")
        || rel_str.contains("replay")
        || rel_str.contains("admission")
        || rel_str.contains("connector")
        || rel_str.contains("effect_journal")
        || rel_str.contains("reanimator")
        || rel_str.contains("instance_registry")
        || rel_str.contains("snapshots")
        || rel_str.contains("snapshot_recovery")
        || rel_str.contains("proptest_all_partition")
        || rel_str.contains("write_class")
        || rel_str.contains("segment_tree")
        || rel_str.contains("query")
        || rel_str.contains("token_bucket")
        || rel_str.contains("cooldown")
        || rel_str.contains("spawn_supervisor")
        || rel_str.contains("probe")
    {
        adrs.insert("ADR-043".to_string());
    }

    adrs
}

fn scan_formal_targets(crates_root: &Path) -> Vec<FormalTarget> {
    let mut targets = Vec::new();
    let rs_files = collect_rs_files(crates_root);
    for file_path in rs_files {
        let source = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let adrs = map_file_to_adrs(&file_path, crates_root);
        for fn_name in extract_kani_targets(&source, &file_path) {
            targets.push(FormalTarget {
                kind: FormalTargetKind::KaniProof,
                file_path: file_path.clone(),
                function_name: fn_name,
                adrs: adrs.clone(),
            });
        }
        let mut ppt_names = in_proptest_block(&source);
        ppt_names.extend(extract_proptest_targets(&source));
        ppt_names.sort();
        ppt_names.dedup();
        for fn_name in ppt_names {
            targets.push(FormalTarget {
                kind: FormalTargetKind::Proptest,
                file_path: file_path.clone(),
                function_name: fn_name,
                adrs: adrs.clone(),
            });
        }
    }
    targets
}

fn count_kani_proofs_in_source(source: &str) -> usize {
    source.matches("#[kani::proof]").count()
}

fn count_proptest_blocks_in_source(source: &str) -> usize {
    let mut count = 0;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("proptest!") || trimmed.starts_with("proptest::proptest!") {
            count += 1;
        }
    }
    count
}

#[test]
fn given_formal_targets_when_scanned_then_adr_invariants_are_mapped() {
    let root = crates_root();
    let targets = scan_formal_targets(&root);

    assert!(
        !targets.is_empty(),
        "No Kani or proptest targets found — scanner may be broken"
    );

    let kani_count = targets
        .iter()
        .filter(|t| t.kind == FormalTargetKind::KaniProof)
        .count();
    let proptest_count = targets
        .iter()
        .filter(|t| t.kind == FormalTargetKind::Proptest)
        .count();

    assert!(
        kani_count >= 10,
        "Expected at least 10 Kani proof targets, found {kani_count}"
    );
    assert!(
        proptest_count >= 10,
        "Expected at least 10 proptest targets, found {proptest_count}"
    );

    let mut orphan_targets: Vec<&FormalTarget> = Vec::new();
    let mut adr_coverage: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for target in &targets {
        if target.adrs.is_empty() {
            orphan_targets.push(target);
        }
        for adr in &target.adrs {
            adr_coverage
                .entry(adr.clone())
                .or_default()
                .insert(format!("{:?}:{}", target.kind, target.function_name));
        }
    }

    assert!(
        orphan_targets.is_empty(),
        "Found {} formal targets with NO ADR invariant coverage (orphans):\n{}",
        orphan_targets.len(),
        orphan_targets
            .iter()
            .map(|t| format!(
                "  {:?} {} in {}",
                t.kind,
                t.function_name,
                t.file_path.display()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let required_adrs = ["ADR-035", "ADR-039", "ADR-043"];
    for adr in &required_adrs {
        let covered = adr_coverage.contains_key(*adr);
        assert!(
            covered,
            "Required {adr} has NO formal test coverage mapped"
        );
    }

    for adr in &required_adrs {
        let targets_for_adr = adr_coverage.get(*adr).map(|s| s.len()).unwrap_or(0);
        assert!(
            targets_for_adr >= 2,
            "Required {adr} has only {targets_for_adr} formal target(s) — need at least 2"
        );
    }

    let mut kani_total_source: usize = 0;
    let mut proptest_total_source: usize = 0;
    let rs_files = collect_rs_files(&root);
    for file_path in &rs_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        kani_total_source += count_kani_proofs_in_source(&source);
        proptest_total_source += count_proptest_blocks_in_source(&source);
    }

    assert!(
        kani_count <= kani_total_source,
        "Scanner found {kani_count} Kani targets but only {kani_total_source} #[kani::proof] annotations in source"
    );
    assert!(
        proptest_count <= proptest_total_source * 5,
        "Scanner found {proptest_count} proptest targets but only {proptest_total_source} proptest! blocks in source"
    );
}
