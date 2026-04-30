//! Causation chain truncation, depth enforcement, and archival (ADR-051).
//!
//! This module manages the bounded depth of causation chains introduced by ADR-036.
//! It provides:
//! - Configurable maximum chain depth (default: 128)
//! - Chain depth tracking on each command
//! - Collapse strategy when depth exceeds the maximum
//! - Broken chain detection (causation_id references non-existent event)

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum causation chain depth before truncation is triggered.
///
/// This bound covers:
/// - Normal workflow execution (typically 5-20 links)
/// - Nested AI agent loops (typically 10-50 links)
/// - Retry cascades with compensation (typically 10-100 links)
/// - Headroom for future complexity
///
/// When a chain would exceed this depth, the oldest segment is collapsed
/// into an archival blob (see `CausationArchival`).
pub const DEFAULT_MAX_CAUSATION_DEPTH: u32 = 128;

/// Minimum acceptable chain depth. Chains shorter than this are considered
/// degenerate (e.g., a single-link chain with no meaningful lineage).
pub const MIN_CAUSATION_DEPTH: u32 = 1;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Tracks the current depth of a causation chain.
///
/// Depth is the number of links from the root causation anchor to the
/// current command. A root command has depth 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CausationDepth(pub u32);

impl CausationDepth {
    /// Create a new causation depth.
    ///
    /// # Panics
    ///
    /// Panics if `depth` is zero or exceeds `DEFAULT_MAX_CAUSATION_DEPTH`.
    pub fn new(depth: u32) -> Self {
        assert!(depth >= MIN_CAUSATION_DEPTH, "depth must be >= 1");
        assert!(
            depth <= DEFAULT_MAX_CAUSATION_DEPTH,
            "depth must be <= {DEFAULT_MAX_CAUSATION_DEPTH}"
        );
        Self(depth)
    }

    /// Create without validation (caller guarantees validity).
    #[must_use]
    pub const fn unchecked(depth: u32) -> Self {
        Self(depth)
    }

    /// Returns true if this depth equals the configured maximum.
    #[must_use]
    pub fn is_at_max(&self) -> bool {
        self.0 >= DEFAULT_MAX_CAUSATION_DEPTH
    }

    /// Returns true if incrementing this depth would exceed the maximum.
    #[must_use]
    pub fn would_exceed_max(&self) -> bool {
        self.0 + 1 > DEFAULT_MAX_CAUSATION_DEPTH
    }

    /// Increment the depth, returning `None` if it would exceed the maximum.
    #[must_use]
    pub fn increment(&self) -> Option<Self> {
        let next = self.0.checked_add(1)?;
        if next > DEFAULT_MAX_CAUSATION_DEPTH {
            None
        } else {
            Some(Self(next))
        }
    }

    /// Get the raw depth value.
    #[must_use]
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Default for CausationDepth {
    fn default() -> Self {
        Self(1)
    }
}

/// Represents a broken causation chain link.
///
/// A broken link occurs when an event's `causation_id` references an event
/// that cannot be found in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenChainLink {
    /// The command that has the broken reference.
    pub referencing_command: String,
    /// The causation_id that could not be resolved.
    pub broken_reference: String,
    /// The instance this event belongs to.
    pub instance_id: String,
    /// Whether the archival lookup also failed for this reference.
    pub archival_lookup_failed: bool,
}

impl BrokenChainLink {
    #[must_use]
    pub fn new(
        referencing_command: &str,
        broken_reference: &str,
        instance_id: &str,
    ) -> Self {
        Self {
            referencing_command: referencing_command.to_string(),
            broken_reference: broken_reference.to_string(),
            instance_id: instance_id.to_string(),
            archival_lookup_failed: false,
        }
    }
}

/// An archival blob containing a collapsed causation chain segment.
///
/// When a chain exceeds `DEFAULT_MAX_CAUSATION_DEPTH`, the oldest segment
/// is replaced by a reference to this archival blob, preserving the full
/// original chain for forensic audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausationArchival {
    /// Unique identifier for this archival segment.
    pub segment_id: String,
    /// Total chain depth before collapse.
    pub original_depth: u32,
    /// The collapsed links (oldest events in the chain).
    pub collapsed_links: Vec<CollapsedLink>,
    /// The causation_id of the link that remains in the active chain
    /// after collapse (points to the deepest link preserved).
    pub preserved_anchor: String,
}

/// A single collapsed link within a `CausationArchival` blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollapsedLink {
    /// The command_id of this link.
    pub command_id: String,
    /// The causation_id of this link.
    pub causation_id: String,
    /// Timestamp when this command was issued.
    pub issued_at_ms: u64,
}

/// Result of attempting to advance a causation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainAdvanceResult {
    /// Chain advanced normally. Contains the new depth.
    Advanced(CausationDepth),
    /// Chain is at max depth. Contains the archival blob to create.
    CollapseRequired {
        /// The new depth after collapse (resets to near-max).
        new_depth: CausationDepth,
        /// The archival blob containing the collapsed segment.
        archival: CausationArchival,
    },
}

/// Errors that can occur during causation chain operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausationChainError {
    /// Chain depth exceeds the configured maximum.
    DepthExceeded { current: u32, max: u32 },
    /// A causation reference was not found.
    ReferenceNotFound { reference: String },
    /// Archival lookup failed for a collapsed segment.
    ArchivalNotFound { segment_id: String },
    /// Invalid chain state (e.g., depth of zero).
    InvalidState(String),
}

impl std::fmt::Display for CausationChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CausationChainError::DepthExceeded { current, max } => {
                write!(f, "chain depth {current} exceeds maximum {max}")
            }
            CausationChainError::ReferenceNotFound { reference } => {
                write!(f, "causation reference not found: {reference}")
            }
            CausationChainError::ArchivalNotFound { segment_id } => {
                write!(f, "archival segment not found: {segment_id}")
            }
            CausationChainError::InvalidState(msg) => {
                write!(f, "invalid chain state: {msg}")
            }
        }
    }
}

impl std::error::Error for CausationChainError {}

// ---------------------------------------------------------------------------
// Chain Depth Management
// ---------------------------------------------------------------------------

/// Attempts to advance a causation chain by one link.
///
/// If the new depth would exceed `DEFAULT_MAX_CAUSATION_DEPTH`, returns
/// a `CollapseRequired` result with an archival blob containing the
/// collapsed segment.
///
/// # Arguments
///
/// * `current_depth` - The current chain depth
/// * `new_command_id` - The command_id of the new chain link
/// * `old_causation_id` - The causation_id being replaced
/// * `segment_id` - A unique identifier for the archival segment
/// * `all_collapsed_links` - All links in the collapsed segment
pub fn advance_chain(
    current_depth: CausationDepth,
    new_command_id: &str,
    old_causation_id: &str,
    segment_id: &str,
    all_collapsed_links: Vec<CollapsedLink>,
) -> Result<ChainAdvanceResult, CausationChainError> {
    if current_depth.would_exceed_max() {
        // Collapse: the new link gets depth = MAX_DEPTH - 1, keeping
        // the oldest surviving link at depth 1
        let new_depth = CausationDepth::unchecked(DEFAULT_MAX_CAUSATION_DEPTH - 1);

        let archival = CausationArchival {
            segment_id: segment_id.to_string(),
            original_depth: current_depth.0,
            collapsed_links: all_collapsed_links,
            preserved_anchor: old_causation_id.to_string(),
        };

        Ok(ChainAdvanceResult::CollapseRequired {
            new_depth,
            archival,
        })
    } else {
        let new_depth = current_depth
            .increment()
            .ok_or_else(|| CausationChainError::DepthExceeded {
                current: current_depth.0,
                max: DEFAULT_MAX_CAUSATION_DEPTH,
            })?;
        Ok(ChainAdvanceResult::Advanced(new_depth))
    }
}

/// Validates a causation chain by checking that all links have valid depth.
///
/// Returns `Ok(())` if the chain is valid, or an error describing the
/// first invalid link found.
pub fn validate_chain_depths(depths: &[CausationDepth]) -> Result<(), CausationChainError> {
    for (i, depth) in depths.iter().enumerate() {
        if depth.0 == 0 {
            return Err(CausationChainError::InvalidState(format!(
                "link {i} has zero depth"
            )));
        }
        if depth.0 > DEFAULT_MAX_CAUSATION_DEPTH {
            return Err(CausationChainError::DepthExceeded {
                current: depth.0,
                max: DEFAULT_MAX_CAUSATION_DEPTH,
            });
        }
    }
    Ok(())
}

/// Checks if a causation reference is a placeholder for a broken link.
///
/// Broken references use the format:
/// - `"archived:<segment-id>"` for collapsed segments
/// - `"unknown:<original-id>"` for unrecoverable references
#[must_use]
pub fn is_broken_chain_reference(reference: &str) -> bool {
    reference.starts_with("archived:") || reference.starts_with("unknown:")
}

/// Extracts the original reference from a broken chain placeholder.
#[must_use]
pub fn extract_broken_chain_original(reference: &str) -> Option<&str> {
    if let Some(rest) = reference.strip_prefix("archived:") {
        Some(rest)
    } else if let Some(rest) = reference.strip_prefix("unknown:") {
        Some(rest)
    } else {
        None
    }
}

/// Validates a causation reference and creates a BrokenChainLink if the reference is broken.
///
/// A broken reference occurs when:
/// - The causation_id uses the `unknown:` prefix (unrecoverable reference)
/// - The causation_id uses the `archived:` prefix (references a collapsed archival segment)
///
/// For references that are not placeholders (i.e., normal causation_ids),
/// this function returns `Ok(())` - the caller is responsible for validating
/// that the referenced command actually exists in the event store.
///
/// # Arguments
///
/// * `causation_id` - The causation_id string to validate
/// * `referencing_command` - The command_id of the command that has this causation_id
/// * `instance_id` - The instance this command belongs to
///
/// # Returns
///
/// * `Ok(BrokenChainLink)` if the reference is a known broken placeholder
/// * `Err(CausationChainError::ReferenceNotFound)` if the reference format indicates a broken chain
pub fn validate_causation_reference(
    causation_id: &str,
    referencing_command: &str,
    instance_id: &str,
) -> Result<BrokenChainLink, CausationChainError> {
    if is_broken_chain_reference(causation_id) {
        let broken = BrokenChainLink::new(referencing_command, causation_id, instance_id);
        if causation_id.starts_with("unknown:") {
            Ok(broken)
        } else if causation_id.starts_with("archived:") {
            Ok(broken)
        } else {
            Err(CausationChainError::ReferenceNotFound {
                reference: causation_id.to_string(),
            })
        }
    } else {
        Ok(BrokenChainLink::new(referencing_command, causation_id, instance_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_default_is_one() {
        let depth = CausationDepth::default();
        assert_eq!(depth.0, 1);
    }

    #[test]
    fn depth_new_validates_minimum() {
        let result = std::panic::catch_unwind(|| CausationDepth::new(0));
        assert!(result.is_err(), "depth 0 should panic");
    }

    #[test]
    fn depth_new_validates_maximum() {
        let result = std::panic::catch_unwind(|| CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH + 1));
        assert!(result.is_err(), "depth > MAX should panic");
    }

    #[test]
    fn depth_increment_works() {
        let depth = CausationDepth::new(1);
        let next = depth.increment().unwrap();
        assert_eq!(next.0, 2);
    }

    #[test]
    fn depth_increment_returns_none_at_max() {
        let depth = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH);
        assert!(depth.increment().is_none());
    }

    #[test]
    fn depth_would_exceed_max_one_before_cap() {
        let depth = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH - 1);
        assert!(!depth.would_exceed_max());

        let depth2 = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH);
        assert!(depth2.would_exceed_max());
    }

    #[test]
    fn depth_is_at_max() {
        assert!(CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH).is_at_max());
        assert!(!CausationDepth::new(1).is_at_max());
    }

    #[test]
    fn advance_chain_normal_path() {
        let depth = CausationDepth::new(5);
        let result = advance_chain(
            depth,
            "cmd-new",
            "cmd-parent",
            "seg-1",
            vec![],
        );
        assert!(matches!(result, Ok(ChainAdvanceResult::Advanced(_))));
        if let Ok(ChainAdvanceResult::Advanced(d)) = result {
            assert_eq!(d.0, 6);
        }
    }

    #[test]
    fn advance_chain_collapses_at_max() {
        let depth = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH);
        let collapsed = vec![CollapsedLink {
            command_id: "cmd-old-1".to_string(),
            causation_id: "cmd-old-2".to_string(),
            issued_at_ms: 1000,
        }];
        let result = advance_chain(depth, "cmd-new", "cmd-parent", "seg-1", collapsed.clone());

        if let Ok(ChainAdvanceResult::CollapseRequired { new_depth, archival }) = result {
            assert_eq!(new_depth.0, DEFAULT_MAX_CAUSATION_DEPTH - 1);
            assert_eq!(archival.segment_id, "seg-1");
            assert_eq!(archival.original_depth, DEFAULT_MAX_CAUSATION_DEPTH);
            assert_eq!(archival.collapsed_links, collapsed);
            assert_eq!(archival.preserved_anchor, "cmd-parent");
        } else {
            panic!("expected CollapseRequired");
        }
    }

    #[test]
    fn validate_chain_depths_all_valid() {
        let depths = vec![
            CausationDepth::new(1),
            CausationDepth::new(2),
            CausationDepth::new(3),
        ];
        assert!(validate_chain_depths(&depths).is_ok());
    }

    #[test]
    fn validate_chain_depths_zero_depth_fails() {
        let depths = vec![
            CausationDepth::new(1),
            CausationDepth(0),
            CausationDepth::new(3),
        ];
        let err = validate_chain_depths(&depths).unwrap_err();
        assert!(matches!(err, CausationChainError::InvalidState(_)));
    }

    #[test]
    fn is_broken_chain_reference_archived() {
        assert!(is_broken_chain_reference("archived:seg-123"));
    }

    #[test]
    fn is_broken_chain_reference_unknown() {
        assert!(is_broken_chain_reference("unknown:cmd-abc"));
    }

    #[test]
    fn is_broken_chain_reference_normal() {
        assert!(!is_broken_chain_reference("cmd-abc"));
    }

    #[test]
    fn extract_broken_chain_original_archived() {
        assert_eq!(
            extract_broken_chain_original("archived:seg-123"),
            Some("seg-123")
        );
    }

    #[test]
    fn extract_broken_chain_original_unknown() {
        assert_eq!(
            extract_broken_chain_original("unknown:cmd-abc"),
            Some("cmd-abc")
        );
    }

    #[test]
    fn extract_broken_chain_original_normal_returns_none() {
        assert_eq!(
            extract_broken_chain_original("cmd-abc"),
            None
        );
    }

    #[test]
    fn validate_causation_reference_unknown_prefix_returns_broken_link() {
        let result = validate_causation_reference(
            "unknown:cmd-missing",
            "cmd-current",
            "inst-1",
        );
        assert!(result.is_ok());
        let broken = result.unwrap();
        assert_eq!(broken.referencing_command, "cmd-current");
        assert_eq!(broken.broken_reference, "unknown:cmd-missing");
        assert_eq!(broken.instance_id, "inst-1");
    }

    #[test]
    fn validate_causation_reference_archived_prefix_returns_broken_link() {
        let result = validate_causation_reference(
            "archived:seg-abc123",
            "cmd-current",
            "inst-2",
        );
        assert!(result.is_ok());
        let broken = result.unwrap();
        assert_eq!(broken.referencing_command, "cmd-current");
        assert_eq!(broken.broken_reference, "archived:seg-abc123");
        assert_eq!(broken.instance_id, "inst-2");
    }

    #[test]
    fn validate_causation_reference_normal_reference_returns_ok() {
        let result = validate_causation_reference(
            "cmd-parent-event",
            "cmd-current",
            "inst-3",
        );
        assert!(result.is_ok());
        let broken = result.unwrap();
        assert_eq!(broken.referencing_command, "cmd-current");
        assert_eq!(broken.broken_reference, "cmd-parent-event");
        assert_eq!(broken.instance_id, "inst-3");
    }

    #[test]
    fn validate_causation_reference_external_root_returns_ok() {
        let result = validate_causation_reference(
            "external-root",
            "cmd-trigger",
            "inst-4",
        );
        assert!(result.is_ok());
        let broken = result.unwrap();
        assert_eq!(broken.referencing_command, "cmd-trigger");
        assert_eq!(broken.broken_reference, "external-root");
        assert_eq!(broken.instance_id, "inst-4");
    }
}
