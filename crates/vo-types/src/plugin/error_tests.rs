#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod error_tests {
    use crate::plugin::*;

    fn make_plugin_id(name: &str) -> PluginId {
        PluginId::new(
            PluginName::new(name).unwrap(),
            PluginVersion::new(1, 0, 0),
            InstanceKey::new(),
        )
    }

    #[test]
    fn error_contains_all_three_fields() {
        let err = PluginHotLoadError::new(
            PluginErrorCategory::VersionIncompatibility,
            PluginErrorDetail::SchemaVersionMismatch {
                expected: SchemaVersion(1),
                actual: PluginVersion::new(2, 0, 0),
            },
            PluginErrorContext::DuringActivation,
        );
        assert!(matches!(
            err.category(),
            PluginErrorCategory::VersionIncompatibility
        ));
        assert!(matches!(
            err.detail(),
            PluginErrorDetail::SchemaVersionMismatch { .. }
        ));
        assert!(matches!(
            err.context(),
            PluginErrorContext::DuringActivation
        ));
    }

    #[test]
    fn error_display_includes_category_and_context() {
        let err = PluginHotLoadError::new(
            PluginErrorCategory::VersionIncompatibility,
            PluginErrorDetail::SchemaVersionMismatch {
                expected: SchemaVersion(1),
                actual: PluginVersion::new(2, 0, 0),
            },
            PluginErrorContext::DuringActivation,
        );
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("version incompatibility"),
            "display should include category: {msg}"
        );
    }

    #[test]
    fn error_display_includes_context_name() {
        let err = PluginHotLoadError::new(
            PluginErrorCategory::LoadFailure,
            PluginErrorDetail::PluginNotFound(make_plugin_id("missing")),
            PluginErrorContext::DuringLoad,
        );
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("load"),
            "display should include context: {msg}"
        );
    }

    #[test]
    fn all_error_categories_constructible() {
        let id = make_plugin_id("test");
        let categories = vec![
            (
                PluginErrorCategory::RegistrationFailure,
                PluginErrorDetail::PluginNotFound(id.clone()),
                PluginErrorContext::DuringRegistration,
            ),
            (
                PluginErrorCategory::LoadFailure,
                PluginErrorDetail::PluginNotFound(id.clone()),
                PluginErrorContext::DuringLoad,
            ),
            (
                PluginErrorCategory::ActivationFailure,
                PluginErrorDetail::CapabilityNotSatisfied {
                    plugin_id: id.clone(),
                    missing: CapabilityId::new("missing-cap"),
                },
                PluginErrorContext::DuringActivation,
            ),
            (
                PluginErrorCategory::DependencyFailure,
                PluginErrorDetail::UnsatisfiedDependency {
                    plugin_id: id.clone(),
                    missing: PluginVersionConstraint {
                        name: PluginName::new("dep").unwrap(),
                        range: VersionRange(">=1.0.0".to_string()),
                    },
                },
                PluginErrorContext::DuringActivation,
            ),
            (
                PluginErrorCategory::VersionIncompatibility,
                PluginErrorDetail::SchemaVersionMismatch {
                    expected: SchemaVersion(1),
                    actual: PluginVersion::new(2, 0, 0),
                },
                PluginErrorContext::DuringLoad,
            ),
            (
                PluginErrorCategory::ResourceExhaustion,
                PluginErrorDetail::ResourceBudgetExceeded {
                    plugin_id: id.clone(),
                    required: ResourceBudget {
                        memory_bytes: 2048,
                        cpu_units: 2,
                        max_instances: 1,
                    },
                    available: ResourceBudget {
                        memory_bytes: 1024,
                        cpu_units: 1,
                        max_instances: 1,
                    },
                },
                PluginErrorContext::DuringLoad,
            ),
            (
                PluginErrorCategory::QuiesceTimeout,
                PluginErrorDetail::QuiesceDeadlineExceeded(id.clone()),
                PluginErrorContext::DuringQuiesce,
            ),
            (
                PluginErrorCategory::FenceViolation,
                PluginErrorDetail::FenceRegression {
                    plugin_id: id.clone(),
                    presented_token: crate::FenceToken::new(1).unwrap(),
                    current_token: crate::FenceToken::new(5).unwrap(),
                },
                PluginErrorContext::DuringActivation,
            ),
            (
                PluginErrorCategory::IsolationViolation,
                PluginErrorDetail::IsolationBreach {
                    plugin_id: id,
                    violation_type: IsolationBreachType::CrossBoundaryAccess,
                },
                PluginErrorContext::DuringHealthCheck,
            ),
        ];
        for (category, detail, context) in categories {
            let err = PluginHotLoadError::new(category, detail, context);
            assert_eq!(err.category(), &category);
        }
    }

    #[test]
    fn all_error_contexts_constructible() {
        let contexts = [
            PluginErrorContext::DuringRegistration,
            PluginErrorContext::DuringLoad,
            PluginErrorContext::DuringActivation,
            PluginErrorContext::DuringQuiesce,
            PluginErrorContext::DuringUnload,
            PluginErrorContext::DuringHealthCheck,
        ];
        for ctx in &contexts {
            let err = PluginHotLoadError::new(
                PluginErrorCategory::LoadFailure,
                PluginErrorDetail::PluginNotFound(make_plugin_id("test")),
                ctx.clone(),
            );
            assert_eq!(err.context(), ctx);
        }
    }

    #[test]
    fn error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(PluginHotLoadError::new(
            PluginErrorCategory::LoadFailure,
            PluginErrorDetail::PluginNotFound(make_plugin_id("test")),
            PluginErrorContext::DuringLoad,
        ));
        let msg = format!("{err}");
        assert!(!msg.is_empty());
    }

    #[test]
    fn dependency_cycle_error_constructs_with_cycle() {
        let cycle = vec![
            PluginName::new("a").unwrap(),
            PluginName::new("b").unwrap(),
            PluginName::new("a").unwrap(),
        ];
        let err = PluginHotLoadError::new(
            PluginErrorCategory::DependencyFailure,
            PluginErrorDetail::DependencyCycle(cycle),
            PluginErrorContext::DuringActivation,
        );
        assert!(matches!(
            err.detail(),
            PluginErrorDetail::DependencyCycle(_)
        ));
    }
}
