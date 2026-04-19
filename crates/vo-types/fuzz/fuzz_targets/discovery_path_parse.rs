#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::DiscoveryPath;

/// Fuzz target for DiscoveryPath parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidPath
/// Tests discovery path string parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid path separators
/// - Very long paths
/// - Unicode
fuzz_target!(|data: &str| {
    let _ = DiscoveryPath::parse(data);
});
