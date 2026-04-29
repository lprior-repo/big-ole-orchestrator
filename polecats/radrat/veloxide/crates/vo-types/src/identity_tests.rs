use crate::identity::{CausationId, CommandId, CorrelationId};
use uuid::Uuid;

#[test]
fn test_command_id_instantiates_correctly() {
    let cmd = CommandId::new();
    assert!(!cmd.to_uuid().is_nil());
}

#[test]
fn test_correlation_id_instantiates_correctly() {
    let corr = CorrelationId::new();
    assert!(!corr.to_uuid().is_nil());
}

#[test]
fn test_causation_id_instantiates_correctly() {
    let caus = CausationId::new();
    assert!(!caus.to_uuid().is_nil());
}

#[test]
fn test_command_id_parse_valid() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let cmd = CommandId::parse(uuid_str).unwrap();
    assert_eq!(cmd.to_uuid().to_string(), uuid_str);
}

#[test]
fn test_correlation_id_parse_valid() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440001";
    let corr = CorrelationId::parse(uuid_str).unwrap();
    assert_eq!(corr.to_uuid().to_string(), uuid_str);
}

#[test]
fn test_causation_id_parse_valid() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440002";
    let caus = CausationId::parse(uuid_str).unwrap();
    assert_eq!(caus.to_uuid().to_string(), uuid_str);
}

#[test]
fn test_command_id_try_from_string_valid() {
    let uuid_str = String::from("550e8400-e29b-41d4-a716-446655440000");
    let cmd = CommandId::try_from(uuid_str).unwrap();
    assert!(!cmd.to_uuid().is_nil());
}

#[test]
fn test_command_id_display_format() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let cmd = CommandId::parse(uuid_str).unwrap();
    assert_eq!(format!("{}", cmd), uuid_str);
}

#[test]
fn test_correlation_id_display_format() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440001";
    let corr = CorrelationId::parse(uuid_str).unwrap();
    assert_eq!(format!("{}", corr), uuid_str);
}

#[test]
fn test_causation_id_display_format() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440002";
    let caus = CausationId::parse(uuid_str).unwrap();
    assert_eq!(format!("{}", caus), uuid_str);
}

#[test]
fn test_identity_types_are_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<CommandId>();
    assert_copy::<CorrelationId>();
    assert_copy::<CausationId>();
}

#[test]
fn test_identity_types_are_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<CommandId>();
    assert_clone::<CorrelationId>();
    assert_clone::<CausationId>();
}

#[test]
fn test_identity_types_debug() {
    let cmd = CommandId::new();
    let debug_str = format!("{:?}", cmd);
    assert!(debug_str.starts_with("CommandId"));
}

#[test]
fn test_identity_types_partial_eq() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let cmd1 = CommandId::parse(uuid_str).unwrap();
    let cmd2 = CommandId::parse(uuid_str).unwrap();
    assert_eq!(cmd1, cmd2);
}

#[test]
fn test_identity_types_hash() {
    use std::collections::HashSet;
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let cmd1 = CommandId::parse(uuid_str).unwrap();
    let cmd2 = CommandId::parse(uuid_str).unwrap();
    let mut set = HashSet::new();
    set.insert(cmd1);
    set.insert(cmd2);
    assert_eq!(set.len(), 1);
}

#[test]
fn test_command_id_serialize_deserialize() {
    let cmd = CommandId::new();
    let serialized = serde_json::to_string(&cmd).unwrap();
    let deserialized: CommandId = serde_json::from_str(&serialized).unwrap();
    assert_eq!(cmd, deserialized);
}

#[test]
fn test_correlation_id_serialize_deserialize() {
    let corr = CorrelationId::new();
    let serialized = serde_json::to_string(&corr).unwrap();
    let deserialized: CorrelationId = serde_json::from_str(&serialized).unwrap();
    assert_eq!(corr, deserialized);
}

#[test]
fn test_causation_id_serialize_deserialize() {
    let caus = CausationId::new();
    let serialized = serde_json::to_string(&caus).unwrap();
    let deserialized: CausationId = serde_json::from_str(&serialized).unwrap();
    assert_eq!(caus, deserialized);
}

#[test]
fn test_command_id_from_uuid() {
    let uuid = Uuid::new_v4();
    let cmd = CommandId::from_uuid(uuid);
    assert_eq!(cmd.to_uuid(), uuid);
}

#[test]
fn test_correlation_id_from_uuid() {
    let uuid = Uuid::new_v4();
    let corr = CorrelationId::from_uuid(uuid);
    assert_eq!(corr.to_uuid(), uuid);
}

#[test]
fn test_causation_id_from_uuid() {
    let uuid = Uuid::new_v4();
    let caus = CausationId::from_uuid(uuid);
    assert_eq!(caus.to_uuid(), uuid);
}

#[test]
fn test_identity_types_not_equal_to_each_other() {
    let cmd_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let corr_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let caus_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440002").unwrap();

    let cmd = CommandId::from_uuid(cmd_uuid);
    let corr = CorrelationId::from_uuid(corr_uuid);
    let caus = CausationId::from_uuid(caus_uuid);

    assert_ne!(cmd.to_uuid(), corr.to_uuid());
    assert_ne!(cmd.to_uuid(), caus.to_uuid());
    assert_ne!(corr.to_uuid(), caus.to_uuid());
}

#[test]
fn test_identity_types_from_bytes() {
    let bytes: [u8; 16] = *b"1234567890abcdef";
    let uuid = Uuid::from_bytes(bytes);
    let cmd = CommandId::from_uuid(uuid);
    assert_eq!(cmd.to_uuid().into_bytes().as_slice(), &bytes[..]);
}

#[test]
fn test_command_id_try_from_string_invalid() {
    let invalid_str = String::from("not-a-uuid");
    let result = CommandId::try_from(invalid_str);
    assert!(result.is_err());
}

#[test]
fn test_correlation_id_try_from_string_invalid() {
    let invalid_str = String::from("not-a-uuid");
    let result = CorrelationId::try_from(invalid_str);
    assert!(result.is_err());
}

#[test]
fn test_causation_id_try_from_string_invalid() {
    let invalid_str = String::from("not-a-uuid");
    let result = CausationId::try_from(invalid_str);
    assert!(result.is_err());
}
