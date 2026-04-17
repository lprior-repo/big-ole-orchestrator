//! BDD tests for Schema Evolution & Version Compatibility.
//!
//! Given/When/Then scenarios covering schema versioning, forward/backward compatibility,
//! event upcasting, and migration as defined in ADR-035.

#[cfg(test)]
mod bdd_matching_version_deploys {
    //! Scenario 1: Matching schema version deploys successfully.
    //!
    //! Given a workflow compiled at schema version 1
    //! When deployed to an engine whose max supported version is 1
    //! Then deployment succeeds without errors

    use crate::types::{
        extract_schema_version, State, WorkflowSpec, MAX_SUPPORTED_SCHEMA_VERSION,
    };
    use serde_json::json;

    #[test]
    fn bdd_matching_schema_version_state_deploys_successfully() {
        // GIVEN a State compiled at schema version 1
        let state = State::default();

        // WHEN the version is checked against MAX_SUPPORTED_SCHEMA_VERSION
        let engine_max = MAX_SUPPORTED_SCHEMA_VERSION;

        // THEN the state version matches and is within supported range
        assert_eq!(state.version(), engine_max);
        assert!(state.version() <= engine_max);
    }

    #[test]
    fn bdd_matching_schema_version_workflow_spec_deploys_successfully() {
        // GIVEN a WorkflowSpec compiled at schema version 1
        let spec = WorkflowSpec::default();

        // WHEN the version is checked against MAX_SUPPORTED_SCHEMA_VERSION
        let engine_max = MAX_SUPPORTED_SCHEMA_VERSION;

        // THEN the spec version matches and is within supported range
        assert_eq!(spec.version(), engine_max);
        assert!(spec.version() <= engine_max);
    }

    #[test]
    fn bdd_matching_schema_version_extract_succeeds() {
        // GIVEN a workflow spec JSON with version 1
        let payload = json!({"version": 1});

        // WHEN extract_schema_version is called
        let result = extract_schema_version(&payload, None);

        // THEN it succeeds without errors
        assert_eq!(result.unwrap(), 1);
    }
}

#[cfg(test)]
mod bdd_future_version_rejected {
    //! Scenario 2: Future schema version rejected.
    //!
    //! Given a workflow compiled at schema version 2
    //! When deployed to an engine whose max supported version is 1
    //! Then deployment rejects with a version incompatibility error

    use crate::events::Error;
    use crate::types::extract_schema_version;

    #[test]
    fn bdd_future_schema_version_rejected_by_extract_schema_version() {
        // GIVEN a workflow payload with schema version 2
        let payload = serde_json::json!({"version": 2});

        // WHEN extract_schema_version is called on an engine with max version 1
        let result = extract_schema_version(&payload, None);

        // THEN it rejects with UnsupportedSchemaVersion error
        assert_eq!(result, Err(Error::UnsupportedSchemaVersion(2)));
    }

    #[test]
    fn bdd_future_event_envelope_version_rejected() {
        // GIVEN an event envelope with schema version 2
        let envelope_json = r#"{
            "version": 2,
            "instance_id": "wf-test",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "Test"}
        }"#;

        // WHEN parsed by an engine whose max supported version is 1
        let result = crate::events::EventEnvelope::from_str(envelope_json);

        // THEN it rejects with UnsupportedEnvelopeVersion
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::UnsupportedEnvelopeVersion(2));
    }

    #[test]
    fn bdd_envelope_is_supported_returns_false_for_future_version() {
        // GIVEN an envelope manually constructed with version > MAX_SUPPORTED_VERSION
        let envelope = crate::events::EventEnvelope {
            schema_version: crate::events::MAX_SUPPORTED_VERSION + 1,
            instance_id: "wf-test".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({"type": "Test"}),
            metadata: crate::events::EventMetadata::default(),
        };

        // WHEN is_supported is checked
        // THEN it returns false
        assert!(!envelope.is_supported());
    }

    #[test]
    fn bdd_future_version_3_also_rejected() {
        // GIVEN a payload with schema version 3
        let payload = serde_json::json!({"version": 3});

        // WHEN extract_schema_version is called
        let result = extract_schema_version(&payload, None);

        // THEN it rejects with UnsupportedSchemaVersion
        assert_eq!(result, Err(Error::UnsupportedSchemaVersion(3)));
    }
}

#[cfg(test)]
mod bdd_event_upcasting_after_upgrade {
    //! Scenario 3: Event upcasting after engine upgrade.
    //!
    //! Given a workflow instance with v1-format events stored
    //! When replayed after an engine upgrade that introduced v2 schema
    //! Then old v1 events are correctly upcasted to v2 format during replay

    use crate::events::upcaster::{Upcaster, VersionRegistry};
    use crate::events::EventEnvelope;
    use crate::events::EventMetadata;
    use serde_json::json;

    struct AddNewFieldUpcaster;
    impl Upcaster for AddNewFieldUpcaster {
        fn source_version(&self) -> u8 {
            0
        }
        fn target_version(&self) -> u8 {
            1
        }
        fn upcast(
            &self,
            payload: &serde_json::Value,
        ) -> Result<serde_json::Value, crate::events::upcaster::UpcasterError> {
            let mut result = payload.clone();
            if let Some(obj) = result.as_object_mut() {
                obj.insert("priority".to_string(), json!("normal"));
                if let Some(v) = obj.get_mut("version") {
                    *v = json!(1);
                }
            }
            Ok(result)
        }
    }

    #[test]
    fn bdd_v1_event_upcasted_to_v2_during_replay() {
        // GIVEN a workflow instance with v0-format events stored
        let v0_envelope = EventEnvelope {
            schema_version: 0,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "StepCompleted", "version": 0, "step": "A"}),
            metadata: EventMetadata::default(),
        };

        // WHEN replayed after an engine upgrade with v0→v1 upcaster registered
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(AddNewFieldUpcaster));

        let result = v0_envelope.upcast_payload(&registry, 1);

        // THEN old events are correctly upcasted to current schema format
        let upcasted = result.expect("upcast should succeed");
        assert_eq!(upcasted["version"], 1);
        assert_eq!(upcasted["priority"], "normal");
        assert_eq!(upcasted["step"], "A");
    }

    #[test]
    fn bdd_same_version_returns_original_payload() {
        // GIVEN a v1 event
        let v1_envelope = EventEnvelope {
            schema_version: 1,
            instance_id: "wf-456".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "StepStarted", "version": 1}),
            metadata: EventMetadata::default(),
        };

        // WHEN upcast to the same version (1 → 1)
        let registry = VersionRegistry::new();
        let result = v1_envelope.upcast_payload(&registry, 1);

        // THEN the original payload is returned unchanged
        let upcasted = result.expect("same-version upcast should succeed");
        assert_eq!(upcasted["version"], 1);
        assert_eq!(upcasted["type"], "StepStarted");
    }

    #[test]
    fn bdd_upcast_chain_broken_returns_error() {
        // GIVEN a v0 event but no upcaster registered for v0→v1
        let v0_envelope = EventEnvelope {
            schema_version: 0,
            instance_id: "wf-789".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: json!({"type": "StepCompleted", "version": 0}),
            metadata: EventMetadata::default(),
        };

        // WHEN upcast to v1 without a registered upcaster
        let registry = VersionRegistry::new();
        let result = v0_envelope.upcast_payload(&registry, 1);

        // THEN it returns an UpcasterNotFound error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::events::Error::UpcasterNotFound { from: 0, to: 1 }
        ));
    }
}

#[cfg(test)]
mod bdd_optional_field_ignored_by_old_engine {
    //! Scenario 4: New optional field ignored by old engine.
    //!
    //! Given a spec with a new optional field added in a later schema version
    //! When an older engine reads the spec
    //! Then the unknown field is gracefully ignored without error

    use crate::types::State;

    #[test]
    fn bdd_state_with_unknown_optional_field_deserializes_successfully() {
        // GIVEN a State JSON with an unknown optional field "new_field"
        let json_with_extra = serde_json::json!({
            "version": 1,
            "new_field": "some_value",
            "another_future_field": 42
        });

        // WHEN an older engine deserializes the spec
        let result: Result<State, _> = serde_json::from_value(json_with_extra);

        // THEN the unknown field is gracefully ignored without error
        assert!(result.is_ok());
        assert_eq!(result.unwrap().version(), 1);
    }

    #[test]
    fn bdd_workflow_spec_with_unknown_optional_field_deserializes_successfully() {
        // GIVEN a WorkflowSpec JSON with unknown optional fields
        let json_with_extra = serde_json::json!({
            "version": 1,
            "future_timeout_ms": 5000,
            "experimental_retry": true
        });

        // WHEN an older engine deserializes the spec
        let result: Result<crate::types::WorkflowSpec, _> = serde_json::from_value(json_with_extra);

        // THEN the unknown fields are gracefully ignored
        assert!(result.is_ok());
        assert_eq!(result.unwrap().version(), 1);
    }

    #[test]
    fn bdd_snapshot_with_unknown_optional_field_deserializes_successfully() {
        // GIVEN a Snapshot JSON with unknown optional fields
        let json_with_extra = serde_json::json!({
            "version": 1,
            "future_metadata": "ignored",
            "new_projection": {}
        });

        // WHEN an older engine deserializes the snapshot
        let result: Result<crate::types::Snapshot, _> = serde_json::from_value(json_with_extra);

        // THEN the unknown fields are gracefully ignored
        assert!(result.is_ok());
        assert_eq!(result.unwrap().version(), 1);
    }
}

#[cfg(test)]
mod bdd_removed_required_field_produces_migration_error {
    //! Scenario 5: Removed required field produces migration error.
    //!
    //! Given a spec where a required field has been removed in a new schema version
    //! When an old engine attempts to read the spec
    //! Then an error is returned with a migration guide indicating how to resolve

    use crate::events::Error;
    use crate::types::extract_schema_version;

    #[test]
    fn bdd_missing_version_field_returns_migration_error() {
        // GIVEN a spec JSON where the required "version" field has been removed
        let payload_no_version = serde_json::json!({"type": "WorkflowSpec"});

        // WHEN an old engine attempts to extract the schema version
        let result = extract_schema_version(&payload_no_version, None);

        // THEN an error is returned (MissingSchemaVersion serves as migration guidance)
        assert_eq!(result, Err(Error::MissingSchemaVersion));
    }

    #[test]
    fn bdd_missing_version_field_with_fallback_returns_legacy_version() {
        // GIVEN a spec JSON without the version field
        let payload_no_version = serde_json::json!({"type": "LegacySpec"});

        // WHEN an old engine uses a fallback policy for backward compatibility
        let result = extract_schema_version(&payload_no_version, Some(0));

        // THEN the fallback version is used, allowing graceful migration
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn bdd_invalid_version_type_returns_error() {
        // GIVEN a spec where version is present but with an invalid type
        let payload_bad_type = serde_json::json!({"version": "not-a-number"});

        // WHEN extract_schema_version is called
        let result = extract_schema_version(&payload_bad_type, None);

        // THEN an error is returned indicating the format issue
        assert_eq!(result, Err(Error::InvalidSchemaVersionFormat));
    }

    #[test]
    fn bdd_non_object_input_returns_error() {
        // GIVEN a payload that is not a JSON object at all
        let payload_array = serde_json::json!([1, 2, 3]);

        // WHEN extract_schema_version is called
        let result = extract_schema_version(&payload_array, None);

        // THEN an error is returned
        assert_eq!(result, Err(Error::InvalidSchemaVersionFormat));
    }
}

#[cfg(test)]
mod bdd_adr035_upcaster_registration {
    //! Scenario 6: ADR-035 event upcaster registration.

    use crate::events::upcaster::{Upcaster, VersionRegistry};
    use serde_json::json;

    #[test]
    fn bdd_custom_upcaster_transforms_old_events_to_new_schema() {
        struct RenameFieldUpcaster;
        impl Upcaster for RenameFieldUpcaster {
            fn source_version(&self) -> u8 { 0 }
            fn target_version(&self) -> u8 { 1 }
            fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, crate::events::upcaster::UpcasterError> {
                let mut r = payload.clone();
                if let Some(obj) = r.as_object_mut() {
                    if let Some(old) = obj.remove("old_name") { obj.insert("new_name".to_string(), old); }
                    if let Some(v) = obj.get_mut("version") { *v = json!(1); }
                }
                Ok(r)
            }
        }

        // GIVEN event schema evolution per ADR-035
        let old_payload = json!({"type": "StepCompleted", "version": 0, "old_name": "step_a"});

        // WHEN a custom upcaster is registered for v0→v1
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(RenameFieldUpcaster));
        let result = registry.upcast_payload(old_payload, 0, 1);

        // THEN old events are correctly transformed to the new schema format
        let upcasted = result.expect("upcast should succeed");
        assert_eq!(upcasted["version"], 1);
        assert_eq!(upcasted["new_name"], "step_a");
        assert!(upcasted.get("old_name").is_none());
    }

    #[test]
    fn bdd_multiple_upcasters_can_be_registered() {
        struct V0ToV1;
        impl Upcaster for V0ToV1 {
            fn source_version(&self) -> u8 { 0 }
            fn target_version(&self) -> u8 { 1 }
            fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, crate::events::upcaster::UpcasterError> {
                let mut r = payload.clone(); r.as_object_mut().unwrap().insert("stage".to_string(), json!("v1")); Ok(r)
            }
        }
        struct V1ToV2;
        impl Upcaster for V1ToV2 {
            fn source_version(&self) -> u8 { 1 }
            fn target_version(&self) -> u8 { 2 }
            fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, crate::events::upcaster::UpcasterError> {
                let mut r = payload.clone(); r.as_object_mut().unwrap().insert("stage".to_string(), json!("v2")); Ok(r)
            }
        }

        // GIVEN multiple upcasters for different version transitions
        let mut registry = VersionRegistry::new();
        registry.register(Box::new(V0ToV1));
        registry.register(Box::new(V1ToV2));

        // THEN both can be retrieved
        assert!(registry.get(0, 1).is_some());
        assert!(registry.get(1, 2).is_some());
        assert!(registry.get(0, 2).is_none());
    }
}

#[cfg(test)]
mod bdd_max_supported_schema_version_constant {
    //! Scenario 8: MAX_SUPPORTED_SCHEMA_VERSION matches engine.
    //!
    //! Given the MAX_SUPPORTED_SCHEMA_VERSION constant defined in the engine
    //! When checked against the engine's actual capability
    //! Then the constant accurately reflects the highest schema version the engine can process

    use crate::types::{MAX_SUPPORTED_SCHEMA_VERSION, State};

    #[test]
    fn bdd_max_supported_schema_version_is_positive() {
        // GIVEN the MAX_SUPPORTED_SCHEMA_VERSION constant
        let max = MAX_SUPPORTED_SCHEMA_VERSION;

        // WHEN checked against basic validity
        // THEN it is a positive, non-zero value
        assert!(max > 0);
    }

    #[test]
    fn bdd_max_supported_schema_version_matches_default_state_version() {
        // GIVEN the MAX_SUPPORTED_SCHEMA_VERSION constant
        let max = MAX_SUPPORTED_SCHEMA_VERSION;

        // WHEN a default State is created
        let state = State::default();

        // THEN the state's version equals MAX_SUPPORTED_SCHEMA_VERSION
        assert_eq!(state.version(), max);
    }

    #[test]
    fn bdd_max_supported_schema_version_matches_default_workflow_spec_version() {
        let max = MAX_SUPPORTED_SCHEMA_VERSION;
        let spec = crate::types::WorkflowSpec::default();
        assert_eq!(spec.version(), max);
    }

    #[test]
    fn bdd_max_supported_schema_version_matches_default_snapshot_version() {
        let max = MAX_SUPPORTED_SCHEMA_VERSION;
        let snap = crate::types::Snapshot::default();
        assert_eq!(snap.version(), max);
    }

    #[test]
    fn bdd_event_max_supported_version_is_valid_u8() {
        let max: u8 = crate::events::MAX_SUPPORTED_VERSION;
        assert!(max > 0);
        assert!(max <= u8::MAX);
    }

    #[test]
    fn bdd_schema_version_constants_are_consistent() {
        let event_max: u8 = crate::events::MAX_SUPPORTED_VERSION;
        let durable_max: u16 = MAX_SUPPORTED_SCHEMA_VERSION;
        assert_eq!(u16::from(event_max), durable_max);
    }
}
