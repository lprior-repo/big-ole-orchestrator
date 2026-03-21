use super::model::{no_results, visible_indices, Category, NODE_TEMPLATES};

// ── Category metadata ─────────────────────────────────────────────────────────

#[test]
fn given_entry_category_when_getting_dot_then_it_uses_entry_colour() {
    assert_eq!(Category::Entry.dot_class(), "bg-blue-400");
}

#[test]
fn given_all_categories_when_getting_labels_then_none_are_empty() {
    for cat in Category::ORDER {
        assert!(!cat.label().is_empty(), "{cat:?} has an empty label");
    }
}

#[test]
fn given_all_categories_when_getting_icon_badge_classes_then_none_are_empty() {
    for cat in Category::ORDER {
        assert!(
            !cat.icon_badge_class().is_empty(),
            "{cat:?} has an empty icon_badge_class"
        );
    }
}

// ── Template matching ─────────────────────────────────────────────────────────

#[test]
fn given_schedule_query_when_matching_templates_then_cron_trigger_is_found() {
    let template = NODE_TEMPLATES
        .iter()
        .find(|t| t.node_type == "cron-trigger");

    assert!(
        template.is_some(),
        "cron-trigger must exist in NODE_TEMPLATES"
    );
    assert!(
        template.is_some_and(|t| t.matches_query("schedule")),
        "cron-trigger should match 'schedule'"
    );
}

#[test]
fn given_friendly_phrase_when_matching_templates_then_send_message_matches() {
    let template = NODE_TEMPLATES
        .iter()
        .find(|t| t.node_type == "send-message");

    assert!(
        template.is_some(),
        "send-message must exist in NODE_TEMPLATES"
    );
    assert!(
        template.is_some_and(|t| t.matches_query("without waiting")),
        "send-message should match 'without waiting'"
    );
}

#[test]
fn given_unrelated_query_when_matching_http_handler_then_no_match() {
    let template = NODE_TEMPLATES
        .iter()
        .find(|t| t.node_type == "http-handler");

    assert!(
        template.is_some(),
        "http-handler must exist in NODE_TEMPLATES"
    );
    assert!(
        template.is_some_and(|t| !t.matches_query("totally-unrelated-query")),
        "http-handler should not match an unrelated query"
    );
}

#[test]
fn given_empty_query_when_matching_any_template_then_always_matches() {
    for template in &NODE_TEMPLATES {
        assert!(
            template.matches_query(""),
            "{} should match an empty query",
            template.node_type
        );
    }
}

// ── visible_indices ───────────────────────────────────────────────────────────

#[test]
fn given_entry_category_with_empty_query_when_getting_indices_then_all_entry_templates_returned() {
    let indices = visible_indices(Category::Entry, "");
    let expected_count = NODE_TEMPLATES
        .iter()
        .filter(|t| t.category == Category::Entry)
        .count();

    assert_eq!(indices.len(), expected_count);
}

#[test]
fn given_impossible_query_when_getting_indices_then_empty_vec_returned() {
    let indices = visible_indices(Category::Durable, "xyzzy-no-match");

    assert!(indices.is_empty());
}

#[test]
fn given_impossible_query_when_checking_no_results_then_returns_true() {
    assert!(no_results("xyzzy-no-match"));
}

#[test]
fn given_empty_query_when_checking_no_results_then_returns_false() {
    assert!(!no_results(""));
}

// ── Catalogue completeness ────────────────────────────────────────────────────

#[test]
fn given_node_templates_when_counting_then_exactly_24_exist() {
    assert_eq!(NODE_TEMPLATES.len(), 24);
}

#[test]
fn given_node_templates_when_checking_types_then_all_node_types_are_non_empty() {
    for t in &NODE_TEMPLATES {
        assert!(!t.node_type.is_empty(), "node_type must not be empty");
        assert!(
            !t.label.is_empty(),
            "label must not be empty for {}",
            t.node_type
        );
        assert!(
            !t.icon.is_empty(),
            "icon must not be empty for {}",
            t.node_type
        );
    }
}

#[test]
fn given_category_order_when_iterating_then_all_six_categories_are_represented() {
    let represented: std::collections::HashSet<Category> =
        NODE_TEMPLATES.iter().map(|t| t.category).collect();

    for cat in Category::ORDER {
        assert!(
            represented.contains(&cat),
            "No template found for category {cat:?}"
        );
    }
}
