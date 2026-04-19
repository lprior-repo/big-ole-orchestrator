use vo_types::{DekId, InstanceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOperation {
    Create,
    Access,
    Use,
    Destroy,
    Rotate,
}

impl KeyOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "key_create",
            Self::Access => "key_access",
            Self::Use => "key_use",
            Self::Destroy => "key_destroy",
            Self::Rotate => "key_rotate",
        }
    }
}

pub struct KeyAuditEntry {
    pub operation: KeyOperation,
    pub instance_id: InstanceId,
    pub dek_id: Option<DekId>,
    pub success: bool,
    pub timestamp_ms: u64,
}

impl KeyAuditEntry {
    pub fn new(
        operation: KeyOperation,
        instance_id: InstanceId,
        dek_id: Option<DekId>,
        success: bool,
    ) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            operation,
            instance_id,
            dek_id,
            success,
            timestamp_ms,
        }
    }

    #[must_use]
    pub fn to_trace_message(&self) -> String {
        format!(
            "key_audit operation={} instance_id={} dek_id={:?} success={} timestamp_ms={}",
            self.operation.as_str(),
            self.instance_id,
            self.dek_id.as_ref().map(|d| d.as_str()),
            self.success,
            self.timestamp_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_instance_id() -> InstanceId {
        InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
    }

    fn sample_dek_id() -> DekId {
        DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    #[test]
    fn key_operation_as_str() {
        assert_eq!(KeyOperation::Create.as_str(), "key_create");
        assert_eq!(KeyOperation::Access.as_str(), "key_access");
        assert_eq!(KeyOperation::Use.as_str(), "key_use");
        assert_eq!(KeyOperation::Destroy.as_str(), "key_destroy");
        assert_eq!(KeyOperation::Rotate.as_str(), "key_rotate");
    }

    #[test]
    fn key_audit_entry_create() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Create, instance_id.clone(), Some(dek_id.clone()), true);

        assert_eq!(entry.operation, KeyOperation::Create);
        assert_eq!(entry.instance_id, instance_id);
        assert_eq!(entry.dek_id.as_ref().unwrap(), &dek_id);
        assert!(entry.success);
        assert!(entry.timestamp_ms > 0);
    }

    #[test]
    fn key_audit_entry_to_trace_message() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Create, instance_id.clone(), Some(dek_id.clone()), true);

        let msg = entry.to_trace_message();
        assert!(msg.contains("key_audit"));
        assert!(msg.contains("key_create"));
        assert!(msg.contains(instance_id.as_str()));
        assert!(msg.contains(dek_id.as_str()));
        assert!(msg.contains("success=true"));
    }

    #[test]
    fn key_audit_entry_failed_operation() {
        let instance_id = sample_instance_id();
        let entry = KeyAuditEntry::new(KeyOperation::Access, instance_id.clone(), None, false);

        assert_eq!(entry.operation, KeyOperation::Access);
        assert!(entry.dek_id.is_none());
        assert!(!entry.success);
    }

    #[test]
    fn key_audit_entry_use_operation() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Use, instance_id.clone(), Some(dek_id.clone()), true);

        assert_eq!(entry.operation, KeyOperation::Use);
        assert_eq!(entry.instance_id, instance_id);
        assert_eq!(entry.dek_id.as_ref().unwrap(), &dek_id);
        assert!(entry.success);
        assert!(entry.timestamp_ms > 0);
    }

    #[test]
    fn key_audit_entry_use_operation_trace_message() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Use, instance_id.clone(), Some(dek_id.clone()), true);

        let msg = entry.to_trace_message();
        assert!(msg.contains("key_audit"));
        assert!(msg.contains("key_use"));
        assert!(msg.contains(instance_id.as_str()));
        assert!(msg.contains(dek_id.as_str()));
        assert!(msg.contains("success=true"));
    }

    #[test]
    fn key_audit_entry_use_operation_failed() {
        let instance_id = sample_instance_id();
        let entry = KeyAuditEntry::new(KeyOperation::Use, instance_id.clone(), None, false);

        assert_eq!(entry.operation, KeyOperation::Use);
        assert!(entry.dek_id.is_none());
        assert!(!entry.success);
    }

    #[test]
    fn key_audit_entry_destroy_operation() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Destroy, instance_id.clone(), Some(dek_id.clone()), true);

        assert_eq!(entry.operation, KeyOperation::Destroy);
        assert_eq!(entry.instance_id, instance_id);
        assert_eq!(entry.dek_id.as_ref().unwrap(), &dek_id);
        assert!(entry.success);
        assert!(entry.timestamp_ms > 0);
    }

    #[test]
    fn key_audit_entry_destroy_operation_trace_message() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Destroy, instance_id.clone(), Some(dek_id.clone()), true);

        let msg = entry.to_trace_message();
        assert!(msg.contains("key_audit"));
        assert!(msg.contains("key_destroy"));
        assert!(msg.contains(instance_id.as_str()));
        assert!(msg.contains(dek_id.as_str()));
        assert!(msg.contains("success=true"));
    }

    #[test]
    fn key_audit_entry_destroy_operation_failed() {
        let instance_id = sample_instance_id();
        let entry = KeyAuditEntry::new(KeyOperation::Destroy, instance_id.clone(), None, false);

        assert_eq!(entry.operation, KeyOperation::Destroy);
        assert!(entry.dek_id.is_none());
        assert!(!entry.success);
    }

    #[test]
    fn key_audit_entry_rotate_operation() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Rotate, instance_id.clone(), Some(dek_id.clone()), true);

        assert_eq!(entry.operation, KeyOperation::Rotate);
        assert_eq!(entry.instance_id, instance_id);
        assert_eq!(entry.dek_id.as_ref().unwrap(), &dek_id);
        assert!(entry.success);
        assert!(entry.timestamp_ms > 0);
    }

    #[test]
    fn key_audit_entry_rotate_operation_trace_message() {
        let instance_id = sample_instance_id();
        let dek_id = sample_dek_id();
        let entry = KeyAuditEntry::new(KeyOperation::Rotate, instance_id.clone(), Some(dek_id.clone()), true);

        let msg = entry.to_trace_message();
        assert!(msg.contains("key_audit"));
        assert!(msg.contains("key_rotate"));
        assert!(msg.contains(instance_id.as_str()));
        assert!(msg.contains(dek_id.as_str()));
        assert!(msg.contains("success=true"));
    }
}