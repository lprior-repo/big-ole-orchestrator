mod resource_quota_boundary {
    use crate::resource_quota::{
        CpuQuota, NamespaceQuota, NamespaceRegistry, OvercommitPolicy, QuotaEnforcer, QuotaError,
        ResourceKind,
    };
    use std::num::NonZeroU64;
    use std::time::Instant;

    fn make_enforcer_with_cpu(ns: &str, cores: u64) -> QuotaEnforcer {
        let mut registry = NamespaceRegistry::new();
        registry
            .register(
                NamespaceQuota::new(ns).with_cpu(CpuQuota::new(NonZeroU64::new(cores).unwrap())),
            )
            .unwrap();
        QuotaEnforcer::new(registry)
    }

    #[test]
    fn quota_error_display_quota_exceeded() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: "test-ns".to_string(),
            requested: 100,
            available: 50,
        };
        let msg = err.to_string();
        assert!(msg.contains("cpu"));
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }

    #[test]
    fn quota_error_display_namespace_not_found() {
        let err = QuotaError::NamespaceNotFound("missing".to_string());
        let msg = err.to_string();
        assert!(msg.contains("missing"));
    }

    #[test]
    fn quota_error_display_not_configured() {
        let err = QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            namespace: "test-ns".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("memory"));
        assert!(msg.contains("not configured"));
    }

    #[test]
    fn enforcer_check_unconfigured_resource_returns_not_configured() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        let result = enforcer.check_memory("test-ns", 1);
        assert!(matches!(result, Err(QuotaError::QuotaNotConfigured { .. })));
    }

    #[test]
    fn enforcer_check_unknown_namespace_returns_not_found() {
        let enforcer = QuotaEnforcer::new(NamespaceRegistry::new());
        let result = enforcer.check_cpu("no-such-ns", 1);
        assert!(matches!(result, Err(QuotaError::NamespaceNotFound { .. })));
    }

    #[test]
    fn enforcer_check_zero_request_always_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 0).is_ok());
    }

    #[test]
    fn enforcer_check_exact_limit_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 100).is_ok());
    }

    #[test]
    fn enforcer_check_over_limit_fails() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        let result = enforcer.check_cpu("test-ns", 101);
        assert!(matches!(result, Err(QuotaError::QuotaExceeded { .. })));
    }

    #[test]
    fn overcommit_policy_default_is_no_overcommit() {
        assert_eq!(OvercommitPolicy::default(), OvercommitPolicy::NoOvercommit);
    }

    #[test]
    fn namespace_quota_empty_name_accepted() {
        let quota = NamespaceQuota::new("");
        assert_eq!(quota.namespace, "");
    }

    #[test]
    fn enforcer_check_below_limit_passes() {
        let enforcer = make_enforcer_with_cpu("test-ns", 100);
        assert!(enforcer.check_cpu("test-ns", 50).is_ok());
        assert!(enforcer.check_cpu("test-ns", 51).is_ok());
    }

    #[test]
    fn enforcer_check_over_single_limit_fails() {
        let enforcer = make_enforcer_with_cpu("test-ns", 10);
        let result = enforcer.check_cpu("test-ns", 11);
        assert!(matches!(result, Err(QuotaError::QuotaExceeded { .. })));
    }

    #[test]
    fn enforcer_overcommit_allows_exceeding_limit() {
        let mut registry = NamespaceRegistry::new();
        registry
            .register(
                NamespaceQuota::new("test-ns")
                    .with_cpu(CpuQuota::new(NonZeroU64::new(10).unwrap()))
                    .with_overcommit(OvercommitPolicy::AllowOvercommit),
            )
            .unwrap();
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("test-ns", 100).is_ok());
    }

    #[test]
    fn quota_error_display_all_variants() {
        let errors = vec![
            QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                namespace: "ns".to_string(),
                requested: 10,
                available: 5,
            },
            QuotaError::NamespaceNotFound("ghost".to_string()),
            QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Memory,
                namespace: "ns".to_string(),
            },
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }
}
