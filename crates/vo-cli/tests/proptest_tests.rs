#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::needless_for_each)]
use proptest::prelude::*;
use std::collections::HashSet;
use vo_cli::commands::check::{validate_binary_header, KNOWN_MAGICS};
use vo_cli::commands::gc::find_unpinned_directories;
use vo_cli::{parse_strict_numeric, CliError};

fn sha256_hex(seed: &str) -> String {
    format!("{:0<64}", seed)
}

proptest! {
    #[test]
    fn parse_strict_numeric_rejects_non_digits(s in ".*[^0-9].*") {
        prop_assert!(matches!(parse_strict_numeric(&s), Err(CliError::InvalidNumeric(_))));
    }

    #[test]
    fn any_4byte_non_magic_is_invalid_magic(magic in any::<[u8; 4]>()) {
        let is_known = KNOWN_MAGICS.contains(&magic);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.bin");
        std::fs::write(&path, magic).expect("write");

        let result = validate_binary_header(&path);
        if is_known {
            prop_assert!(matches!(result, Ok(_)));
        } else {
            prop_assert!(matches!(result, Err(_)));
        }
    }

    #[test]
    fn prop_unpinned_is_set_difference(
        seeds in proptest::collection::hash_set("[a-f0-9]{3}", 1..6),
        pinned_seeds in proptest::collection::hash_set("[a-f0-9]{3}", 0..6),
    ) {
        let all_hashes: HashSet<String> = seeds.iter().map(|s| sha256_hex(s)).collect();
        let pinned: HashSet<String> = pinned_seeds.iter().map(|s| sha256_hex(s)).collect();

        let dir = tempfile::tempdir().expect("tempdir");
        all_hashes.iter().for_each(|hash| {
            std::fs::create_dir_all(dir.path().join(hash)).expect("mkdir");
        });

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(find_unpinned_directories(dir.path(), &pinned));
        prop_assert!(matches!(result, Ok(_)));

        let unpinned = result.expect("ok");
        let unpinned_names: HashSet<String> = unpinned
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str().map(String::from)))
            .collect();

        let expected: HashSet<String> = all_hashes.difference(&pinned).cloned().collect();
        prop_assert_eq!(unpinned_names, expected);
    }
}
