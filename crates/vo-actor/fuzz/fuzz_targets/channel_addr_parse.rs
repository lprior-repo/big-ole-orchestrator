#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_actor::message_router::ChannelAddr;

/// Fuzz target for ChannelAddr parsing.
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/InvalidChannelAddr
/// Tests channel address parsing
///
/// Corpus seeds:
/// - Empty string
/// - Invalid separator
/// - Missing parts
/// - Very long strings
fuzz_target!(|data: &str| {
    let _ = ChannelAddr::parse(data);
});
