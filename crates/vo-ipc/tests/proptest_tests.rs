#![allow(clippy::all, dead_code, unused_imports)]

use proptest::prelude::*;
use std::os::unix::process::ExitStatusExt;
use vo_ipc::{MAX_STDERR_BYTES, TRUNCATION_MARKER};

#[path = "../src/config.rs"]
mod config;
#[path = "../src/envelope.rs"]
mod envelope;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/run.rs"]
mod run;
#[path = "../src/stderr.rs"]
mod stderr;

use config::parse_fd3_payload_as_argv;
use run::{encode_fd4_payload, map_exit_code};
use stderr::{finalize_capture, update_capture, StderrCapture};

proptest! {
    #[test]
    fn bounded_capture_never_exceeds_cap_plus_marker(input in proptest::collection::vec(any::<u8>(), 0..(MAX_STDERR_BYTES + 2048))) {
        let capture = finalize_capture(update_capture(StderrCapture::empty(), &input));
        let max = MAX_STDERR_BYTES + TRUNCATION_MARKER.len();
        prop_assert!(capture.bytes.len() <= max);
    }

    #[test]
    fn bounded_capture_preserves_prefix(input in proptest::collection::vec(any::<u8>(), 0..8192)) {
        let capture = finalize_capture(update_capture(StderrCapture::empty(), &input));
        let prefix_len = input.len().min(MAX_STDERR_BYTES);
        prop_assert_eq!(&capture.bytes[..prefix_len], &input[..prefix_len]);
    }

    #[test]
    fn bounded_capture_sets_marker_iff_truncated(input in proptest::collection::vec(any::<u8>(), 0..(MAX_STDERR_BYTES + 2048))) {
        let capture = finalize_capture(update_capture(StderrCapture::empty(), &input));
        prop_assert_eq!(capture.bytes.ends_with(TRUNCATION_MARKER.as_bytes()), input.len() > MAX_STDERR_BYTES);
    }

    #[test]
    fn fd4_encoding_round_trips(payload in proptest::collection::vec(any::<u8>(), 0..8192)) {
        let encoded = encode_fd4_payload(&payload);
        let declared = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        prop_assert_eq!(declared, payload.len());
        prop_assert_eq!(&encoded[4..], payload.as_slice());
    }

    #[test]
    fn argv_parser_matches_string_split(words in proptest::collection::vec("[A-Za-z0-9_-]{1,8}", 0..16)) {
        let joined = words.join(" ");
        let parsed: Vec<String> = parse_fd3_payload_as_argv(joined.as_bytes())
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        prop_assert_eq!(parsed, words);
    }

    #[test]
    fn exit_code_mapping_preserves_all_u8_codes(code in any::<u8>()) {
        let status = std::process::ExitStatus::from_raw(i32::from(code) << 8);
        prop_assert_eq!(map_exit_code(status), i32::from(code));
    }
}
