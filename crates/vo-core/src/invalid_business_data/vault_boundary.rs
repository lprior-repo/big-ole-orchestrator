mod vault_boundary {
    use super::*;
    use crate::vault::{
        rotation::{RotationStateError, RotationStateMachine},
        CredentialError,
    };

    #[test]
    fn rotation_start_from_waiting_for_overlap_rejected() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().unwrap();
        machine.enter_overlap();

        let result = machine.start_rotation();
        assert!(matches!(result, Err(RotationStateError::AlreadyRotating)));
    }

    #[test]
    fn rotation_complete_from_idle_resets_to_idle() {
        let mut machine = RotationStateMachine::new();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );

        machine.complete_rotation(None);
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_fail_from_idle_transitions_to_failed() {
        let mut machine = RotationStateMachine::new();
        machine.fail_rotation("unexpected call".to_string());
        assert!(matches!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Failed(ref s) if s == "unexpected call"
        ));
        assert_eq!(machine.state().consecutive_failures(), 1);
    }

    #[test]
    fn rotation_enter_overlap_from_idle_succeeds() {
        let mut machine = RotationStateMachine::new();
        machine.enter_overlap();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::WaitingForOverlap
        );
    }

    #[test]
    fn rotation_acknowledge_failure_from_idle_resets() {
        let mut machine = RotationStateMachine::new();
        machine.acknowledge_failure();
        assert_eq!(
            machine.state().state(),
            vo_types::credentials::RotationStatus::Idle
        );
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_double_failure_accumulates_counter() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().unwrap();
        machine.fail_rotation("err1".to_string());
        assert_eq!(machine.state().consecutive_failures(), 1);

        machine.start_rotation().unwrap();
        machine.fail_rotation("err2".to_string());
        assert_eq!(machine.state().consecutive_failures(), 2);
    }

    #[test]
    fn rotation_state_error_debug_format_contains_variant_name() {
        let err = RotationStateError::AlreadyRotating;
        let msg = format!("{err:?}");
        assert!(msg.contains("AlreadyRotating"));
    }

    #[test]
    fn credential_error_all_variants_have_display() {
        let id =
            vo_types::credentials::CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let errs: Vec<CredentialError> = vec![
            CredentialError::CredentialNotFound(id.clone()),
            CredentialError::CredentialAlreadyExists(id.clone()),
            CredentialError::VersionNotFound {
                credential_id: id.clone(),
                version_id: vo_types::credentials::CredentialVersionId::parse(
                    "01H5JYV4XHGSR2F8KZ9BWNRFMB",
                )
                .unwrap(),
            },
            CredentialError::InvalidCredentialState {
                credential_id: id.clone(),
                current_status: vo_types::credentials::CredentialStatus::Active,
                required_status: vec![vo_types::credentials::CredentialStatus::Active],
                operation: "rotate".to_string(),
            },
            CredentialError::MasterKeyNotFound(1),
            CredentialError::MasterKeyRevoked(1),
            CredentialError::VaultStorageError("disk full".to_string()),
        ];

        for err in &errs {
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "error display should not be empty: {:?}",
                err
            );
        }
    }
}