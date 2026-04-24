#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::Credentials;

/// Fuzz target for Credentials parsing (username, password, api_key).
///
/// Input type: &str (arbitrary strings)
/// Risk class: Panic/InvalidFormat/EmptyCredentials
/// Tests credential parsing from strings
///
/// Corpus seeds:
/// - Empty strings
/// - Only whitespace
/// - Very long strings (10KB+)
/// - Unicode characters
/// - Special characters
fuzz_target!(|data: &str| {
    let _ = Credentials::parse_username(data);
    let _ = Credentials::parse_password(data);
    let _ = Credentials::parse_api_key(data);
});
