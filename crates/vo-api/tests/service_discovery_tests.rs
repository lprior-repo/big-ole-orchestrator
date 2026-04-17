use vo_types::discovery::{
    enforce_pin, validate_discovery_path, DiscoveryPath, DiscoveryPathError, PinEnforcementError,
    VersionConstraint, VersionPin, VERSION_BASE_PATH,
};

mod discovery_path_parsing {
    use super::*;

    #[test]
    fn parse_valid_discovery_path() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &vo_types::BinaryHash::parse("abcdef0123456789").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
        assert_eq!(path.version_root(), VERSION_BASE_PATH);
    }

    #[test]
    fn parse_valid_discovery_path_with_file_prefix() {
        let path =
            DiscoveryPath::parse("file:///var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &vo_types::BinaryHash::parse("abcdef0123456789").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
    }

    #[test]
    fn parse_discovery_path_with_long_hash() {
        let path =
            DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789abcdef/my-binary").unwrap();
        assert_eq!(
            path.binary_hash(),
            &vo_types::BinaryHash::parse("abcdef0123456789abcdef").unwrap()
        );
        assert_eq!(path.binary_name(), "my-binary");
    }

    #[test]
    fn parse_invalid_path_wrong_prefix() {
        let result = DiscoveryPath::parse("/other/path/abcdef0123456789/binary");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn parse_invalid_path_no_hash() {
        let result = DiscoveryPath::parse("/var/wtf/versions//my-binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn parse_invalid_path_invalid_hash_format() {
        let result = DiscoveryPath::parse("/var/wtf/versions/notahext/my-binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn parse_invalid_path_odd_hex_length() {
        let result = DiscoveryPath::parse("/var/wtf/versions/abcdef012345678/my-binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn parse_invalid_path_short_hash() {
        let result = DiscoveryPath::parse("/var/wtf/versions/ab/my-binary");
        assert!(matches!(result, Err(DiscoveryPathError::InvalidHash(_))));
    }

    #[test]
    fn parse_invalid_path_missing_binary_name() {
        let result = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn parse_invalid_path_empty_string() {
        let result = DiscoveryPath::parse("");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn parse_invalid_path_only_prefix() {
        let result = DiscoveryPath::parse("/var/wtf/versions/");
        assert!(matches!(
            result,
            Err(DiscoveryPathError::InvalidFormat { .. })
        ));
    }
}

mod discovery_path_serialization {
    use super::*;

    #[test]
    fn discovery_path_display() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.to_string(),
            "/var/wtf/versions/abcdef0123456789/my-binary"
        );
    }

    #[test]
    fn discovery_path_to_string_lossy() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert_eq!(
            path.to_string_lossy(),
            "/var/wtf/versions/abcdef0123456789/my-binary"
        );
    }

    #[test]
    fn discovery_path_roundtrip() {
        let original =
            DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        let serialized = original.to_string();
        let deserialized = DiscoveryPath::parse(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn discovery_path_with_file_prefix_roundtrip() {
        let original =
            DiscoveryPath::parse("file:///var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        let serialized = original.to_string();
        let deserialized = DiscoveryPath::parse(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}

mod discovery_path_transformations {
    use super::*;

    #[test]
    fn with_binary_name() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/original").unwrap();
        let new_path = path.with_binary_name("new-binary".to_string());
        assert_eq!(new_path.binary_name(), "new-binary");
        assert_eq!(new_path.binary_hash(), path.binary_hash());
        assert_eq!(new_path.version_root(), path.version_root());
    }

    #[test]
    fn with_hash() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        let new_hash = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let new_path = path.with_hash(new_hash.clone());
        assert_eq!(new_path.binary_name(), "my-binary");
        assert_eq!(new_path.binary_hash(), &new_hash);
        assert_eq!(new_path.version_root(), path.version_root());
    }
}

mod discovery_path_validation {
    use super::*;

    #[test]
    fn validate_discovery_path_valid() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my-binary").unwrap();
        assert!(validate_discovery_path(&path).is_ok());
    }

    #[test]
    fn validate_discovery_path_empty_name() {
        let result = DiscoveryPath::new(
            VERSION_BASE_PATH.to_string(),
            vo_types::BinaryHash::parse("abcdef0123456789").unwrap(),
            String::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn validate_discovery_path_name_with_separator() {
        let result = DiscoveryPath::new(
            VERSION_BASE_PATH.to_string(),
            vo_types::BinaryHash::parse("abcdef0123456789").unwrap(),
            "foo/bar".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn validate_discovery_path_name_with_multiple_separators() {
        let result = DiscoveryPath::new(
            VERSION_BASE_PATH.to_string(),
            vo_types::BinaryHash::parse("abcdef0123456789").unwrap(),
            "foo/bar/baz".to_string(),
        );
        assert!(result.is_err());
    }
}

mod version_constraint_matching {
    use super::*;

    #[test]
    fn exact_constraint_matches_same_hash() {
        let hash = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(constraint.matches(&hash, &hash));
    }

    #[test]
    fn exact_constraint_no_match_different_hash() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Exact;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn compatible_constraint_same_prefix() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("abcdef01deadbeef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn compatible_constraint_different_prefix() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(!constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn compatible_constraint_first_8_chars_identical() {
        let hash1 = vo_types::BinaryHash::parse("abcdef00aaaa1111").unwrap();
        let hash2 = vo_types::BinaryHash::parse("abcdef00bbbb2222").unwrap();
        let constraint = VersionConstraint::Compatible;
        assert!(constraint.matches(&hash1, &hash2));
    }

    #[test]
    fn latest_constraint_always_matches() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let constraint = VersionConstraint::Latest;
        assert!(constraint.matches(&hash1, &hash2));
        assert!(constraint.matches(&hash2, &hash1));
    }
}

mod version_pin_enforcement {
    use super::*;

    #[test]
    fn enforce_pin_success() {
        let hash = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 1000);
        assert!(enforce_pin(&pin, &hash).is_ok());
    }

    #[test]
    fn enforce_pin_mismatch() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let pin = VersionPin::new(hash1, 1000);
        let result = enforce_pin(&pin, &hash2);
        assert!(matches!(
            result,
            Err(PinEnforcementError::HashMismatch { .. })
        ));
    }

    #[test]
    fn version_pin_accessors() {
        let hash = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 12345);
        assert_eq!(pin.pin_hash(), &hash);
        assert_eq!(pin.pinned_at_ms(), 12345);
    }
}

mod discovery_path_error_display {
    use super::*;

    #[test]
    fn invalid_format_error_display() {
        let err = DiscoveryPathError::InvalidFormat {
            reason: "path must start with /var/wtf/versions/".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("invalid discovery path format"));
        assert!(display.contains("/var/wtf/versions/"));
    }

    #[test]
    fn invalid_hash_error_display() {
        let hash_err = vo_types::ParseError::Empty {
            type_name: "BinaryHash",
        };
        let err = DiscoveryPathError::InvalidHash(hash_err);
        let display = format!("{}", err);
        assert!(display.contains("invalid binary hash"));
    }

    #[test]
    fn pin_mismatch_error_display() {
        let hash1 = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let hash2 = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let err = PinEnforcementError::HashMismatch {
            expected: hash1,
            actual: hash2,
        };
        let display = format!("{}", err);
        assert!(display.contains("hash mismatch"));
        assert!(display.contains("abcdef0123456789"));
        assert!(display.contains("1234567890abcdef"));
    }
}

mod service_discovery_integration {
    use super::*;

    #[test]
    fn full_discovery_flow() {
        let hash = vo_types::BinaryHash::parse("deadbeef01234567").unwrap();
        let path =
            DiscoveryPath::parse("/var/wtf/versions/deadbeef01234567/workflow-engine").unwrap();

        assert!(validate_discovery_path(&path).is_ok());

        let pin = VersionPin::new(hash.clone(), 1700000000000);
        assert!(enforce_pin(&pin, &hash).is_ok());

        let constraint = VersionConstraint::Compatible;
        assert!(constraint.matches(&hash, &hash));
    }

    #[test]
    fn discovery_path_for_different_service_types() {
        let services = vec![
            ("workflow-engine", "abcdef0123456789"),
            ("task-queue", "1234567890abcdef"),
            ("storage-backend", "fedcba9876543210"),
        ];

        for (name, hash_str) in services {
            let path = format!("/var/wtf/versions/{}/{}", hash_str, name);
            let parsed = DiscoveryPath::parse(&path).unwrap();
            assert_eq!(parsed.binary_name(), name);
            assert_eq!(parsed.binary_hash().as_str(), hash_str);
        }
    }

    #[test]
    fn version_constraint_scenarios() {
        let pinned = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();

        let exact_match = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let exact_no_match = vo_types::BinaryHash::parse("1234567890abcdef").unwrap();
        let compatible_match = vo_types::BinaryHash::parse("abcdef01deadbeef").unwrap();

        assert!(VersionConstraint::Exact.matches(&exact_match, &pinned));
        assert!(!VersionConstraint::Exact.matches(&exact_no_match, &pinned));

        assert!(VersionConstraint::Compatible.matches(&compatible_match, &pinned));
        assert!(!VersionConstraint::Compatible.matches(&exact_no_match, &pinned));

        assert!(VersionConstraint::Latest.matches(&exact_match, &pinned));
        assert!(VersionConstraint::Latest.matches(&exact_no_match, &pinned));
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn empty_version_root_in_path_parse() {
        let path = DiscoveryPath::new(
            String::new(),
            vo_types::BinaryHash::parse("abcdef0123456789").unwrap(),
            "binary".to_string(),
        )
        .unwrap();
        assert_eq!(path.version_root(), "");
    }

    #[test]
    fn long_binary_name() {
        let long_name = "a".repeat(255);
        let path =
            DiscoveryPath::parse(&format!("/var/wtf/versions/abcdef0123456789/{}", long_name))
                .unwrap();
        assert_eq!(path.binary_name(), long_name);
    }

    #[test]
    fn binary_name_with_dashes_and_underscores() {
        let path = DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my_binary-v2").unwrap();
        assert_eq!(path.binary_name(), "my_binary-v2");
    }

    #[test]
    fn version_pin_zero_timestamp() {
        let hash = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), 0);
        assert_eq!(pin.pinned_at_ms(), 0);
    }

    #[test]
    fn version_pin_max_timestamp() {
        let hash = vo_types::BinaryHash::parse("abcdef0123456789").unwrap();
        let pin = VersionPin::new(hash.clone(), u64::MAX);
        assert_eq!(pin.pinned_at_ms(), u64::MAX);
    }

    #[test]
    fn parse_path_with_special_chars_in_name() {
        let path =
            DiscoveryPath::parse("/var/wtf/versions/abcdef0123456789/my.binary_v2.0").unwrap();
        assert_eq!(path.binary_name(), "my.binary_v2.0");
    }
}
