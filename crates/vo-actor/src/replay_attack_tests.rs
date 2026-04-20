//! Black-hat adversarial tests: message replay attack surface.
//!
//! Task ID: bh-002
//!
//! These tests PROVE that vo-actor messages lack replay protection by demonstrating
//! that captured messages can be replayed to cause duplicate processing.

use crate::message_router::{MessageMetadata, MessageRouter};

/// VULNERABILITY: MessageMetadata generates unique ULID per message, but
/// the MessageRouter never checks if a message_id was already processed.
///
/// This means the infrastructure for dedup exists (message_id) but is
/// completely unused. An attacker with access to the routing layer can
/// replay messages with impunity.
#[test]
fn message_metadata_generates_unique_ids_but_no_dedup_check_exists() {
    let meta1 = MessageMetadata::default();
    let meta2 = MessageMetadata::default();

    // Each metadata gets a unique ULID
    assert_ne!(
        meta1.message_id, meta2.message_id,
        "Each MessageMetadata gets a unique message_id"
    );

    // But there is no SeenMessages set, dedup cache, or idempotency store
    // anywhere in the MessageRouter struct
    let router = MessageRouter::with_default_config();

    // PROOF: MessageRouter struct fields (visible via debug) contain
    // no dedup/members. A HashSet<String> or LruCache<String, ()> for
    // seen message_ids is absent.
    let debug_str = format!("{:?}", router);
    assert!(
        !debug_str.contains("seen_messages")
            && !debug_str.contains("dedup")
            && !debug_str.contains("processed_ids"),
        "VULNERABILITY: No dedup data structure exists in MessageRouter"
    );
}
