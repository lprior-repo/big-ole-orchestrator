mod workflow_version_boundary {
    use crate::workflow_version::{WorkflowVersion, WorkflowVersionError};
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    #[test]
    fn short_hash_rejected() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse("aabbccdd").unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(matches!(result, Err(WorkflowVersionError::HashTooShort)));
    }

    #[test]
    fn exact_64_char_hash_accepted() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse(&"a".repeat(64)).unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(result.is_ok());
        let wv = result.unwrap();
        assert_eq!(wv.schema_version(), 1);
    }

    #[test]
    fn long_hash_accepted() {
        let name = WorkflowName::parse("test-wf").unwrap();
        let hash = BinaryHash::parse(&"b".repeat(128)).unwrap();
        let ts = TimestampMs::now();
        let result = WorkflowVersion::new(name, hash, ts);
        assert!(result.is_ok());
    }

    #[test]
    fn binary_path_includes_hash_and_name() {
        let name = WorkflowName::parse("my-wf").unwrap();
        let hash = BinaryHash::parse(&"c".repeat(64)).unwrap();
        let ts = TimestampMs::now();
        let wv = WorkflowVersion::new(name, hash, ts).unwrap();
        let path = wv.binary_path();
        assert!(path.contains("my-wf"));
        assert!(path.contains(&"c".repeat(64)));
    }
}
