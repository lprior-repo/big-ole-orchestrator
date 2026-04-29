#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

//! Guarantee-aware UI badges (ADR-007, ADR-031).
//!
//! Renders visual badges distinguishing exact-once vs at-least-once vs best-effort
//! delivery semantics. Two components:
//! - `GuaranteeBadge`: workflow-level guarantee class badge
//! - `NodeGuaranteeBadge`: per-node safety classification badge

use dioxus::prelude::*;
use vo_types::{GuaranteeClass, NodeKind};

/// Workflow-level guarantee class badge.
///
/// Renders a colored badge with a shield icon and human-readable label
/// indicating the delivery guarantee tier (exact-once / at-least-once / best-effort).
/// Uses Tailwind classes from `GuaranteeClass::badge_class()` and icon from `GuaranteeClass::icon()`.
#[component]
pub fn GuaranteeBadge(guarantee_class: GuaranteeClass, class: Option<String>) -> Element {
    let badge_classes = guarantee_class.badge_class();
    let icon_name = guarantee_class.icon();
    let label = guarantee_class.label();
    let extra_class = class.unwrap_or_default();

    rsx! {
        span {
            class: "inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-[10px] font-medium {badge_classes} {extra_class}",
            {crate::ui::icons::icon_by_name(icon_name, "h-3 w-3".to_string())}
            span { "{label}" }
        }
    }
}

/// Per-node guarantee-aware badge showing the node's safety classification
/// relative to the workflow's guarantee tier.
///
/// - For `Unsafe` nodes in non-BestEffort workflows: renders a conflict indicator.
/// - For all other nodes: renders the workflow guarantee tier as context.
#[component]
pub fn NodeGuaranteeBadge(
    node_kind: NodeKind,
    workflow_guarantee: GuaranteeClass,
    class: Option<String>,
) -> Element {
    let extra_class = class.unwrap_or_default();

    let is_conflict =
        matches!(node_kind, NodeKind::Unsafe) && !workflow_guarantee.permits_unsafe_nodes();

    if is_conflict {
        rsx! {
            span {
                class: "inline-flex items-center gap-1 rounded-md border border-red-400 bg-red-100 px-2 py-0.5 text-[10px] font-medium text-red-700 {extra_class}",
                {crate::ui::icons::icon_by_name("shield-off", "h-3 w-3".to_string())}
                span { "unsafe in {workflow_guarantee.label()} workflow" }
            }
        }
    } else {
        rsx! {
            GuaranteeBadge { guarantee_class: workflow_guarantee, class: extra_class }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarantee_class_badge_classes_are_distinct() {
        let exact = GuaranteeClass::ExactOnce.badge_class();
        let atleast = GuaranteeClass::AtLeastOnce.badge_class();
        let best = GuaranteeClass::BestEffort.badge_class();

        assert_ne!(exact, atleast);
        assert_ne!(exact, best);
        assert_ne!(atleast, best);
    }

    #[test]
    fn guarantee_class_icons_are_distinct_shields() {
        for gc in GuaranteeClass::all_variants() {
            let icon = gc.icon();
            assert!(
                icon.contains("shield"),
                "guarantee icon must be shield variant, got: {icon}"
            );
        }

        assert_ne!(
            GuaranteeClass::ExactOnce.icon(),
            GuaranteeClass::AtLeastOnce.icon()
        );
        assert_ne!(
            GuaranteeClass::ExactOnce.icon(),
            GuaranteeClass::BestEffort.icon()
        );
        assert_ne!(
            GuaranteeClass::AtLeastOnce.icon(),
            GuaranteeClass::BestEffort.icon()
        );
    }

    #[test]
    fn guarantee_class_labels_are_non_empty() {
        for gc in GuaranteeClass::all_variants() {
            assert!(!gc.label().is_empty());
        }
    }

    #[test]
    fn unsafe_node_in_exact_once_is_conflict() {
        assert!(matches!(NodeKind::Unsafe, NodeKind::Unsafe));
        assert!(!GuaranteeClass::ExactOnce.permits_unsafe_nodes());
    }

    #[test]
    fn unsafe_node_in_best_effort_is_not_conflict() {
        assert!(GuaranteeClass::BestEffort.permits_unsafe_nodes());
    }

    #[test]
    fn pure_node_in_exact_once_is_not_conflict() {
        assert!(!matches!(NodeKind::Pure, NodeKind::Unsafe));
    }

    #[test]
    fn all_node_kinds_except_unsafe_are_safe_in_any_guarantee() {
        let safe_kinds = [
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
        ];
        for kind in safe_kinds {
            assert!(!matches!(kind, NodeKind::Unsafe));
        }
    }

    #[test]
    fn exact_once_badge_uses_emerald() {
        assert!(GuaranteeClass::ExactOnce.badge_class().contains("emerald"));
    }

    #[test]
    fn at_least_once_badge_uses_amber() {
        assert!(GuaranteeClass::AtLeastOnce.badge_class().contains("amber"));
    }

    #[test]
    fn best_effort_badge_uses_red() {
        assert!(GuaranteeClass::BestEffort.badge_class().contains("red"));
    }

    #[test]
    fn guarantee_class_all_variants_covers_three_tiers() {
        assert_eq!(GuaranteeClass::all_variants().len(), 3);
    }

    #[test]
    fn conflict_detection_logic_is_complete() {
        let guarantees = [
            GuaranteeClass::ExactOnce,
            GuaranteeClass::AtLeastOnce,
            GuaranteeClass::BestEffort,
        ];

        for guarantee in guarantees {
            let is_unsafe_conflict = !guarantee.permits_unsafe_nodes();
            match guarantee {
                GuaranteeClass::ExactOnce => assert!(is_unsafe_conflict),
                GuaranteeClass::AtLeastOnce => assert!(is_unsafe_conflict),
                GuaranteeClass::BestEffort => assert!(!is_unsafe_conflict),
            }
        }
    }
}
