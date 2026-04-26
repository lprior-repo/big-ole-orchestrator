use super::write_class::WriteClass;

pub(crate) fn emit_rejection(class: WriteClass, reason: &str) {
    let label = match class {
        WriteClass::CriticalControlPlane => "critical_control_plane",
        WriteClass::OperatorProjection => "operator_projection",
        WriteClass::BulkBlob => "bulk_blob",
    };
    metrics::counter!("vo_storage.write_rejected_total", "class" => label, "reason" => reason.to_string())
        .increment(1);
}

pub(crate) fn emit_queue_depth(class: WriteClass, depth: usize) {
    let label = match class {
        WriteClass::CriticalControlPlane => "critical_control_plane",
        WriteClass::OperatorProjection => "projection",
        WriteClass::BulkBlob => "bulk_blob",
    };
    metrics::gauge!("vo_storage.queue_depth", "class" => label).set(depth as f64);
}
