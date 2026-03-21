#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "get" => Self::Get,
            "put" => Self::Put,
            "delete" => Self::Delete,
            "patch" => Self::Patch,
            _ => Self::Post,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[Self::Get, Self::Post, Self::Put, Self::Delete, Self::Patch]
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvocationStatus {
    Queued,
    Running,
    Suspended,
    Completed,
    Failed,
    Skipped,
    Retrying,
}

impl InvocationStatus {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "suspended" => Some(Self::Suspended),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "skipped" => Some(Self::Skipped),
            "retrying" => Some(Self::Retrying),
            "" => None,
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Suspended => "suspended",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Retrying => "retrying",
        }
    }

    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Suspended => "Suspended",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Skipped => "Skipped",
            Self::Retrying => "Retrying",
        }
    }

    #[must_use]
    pub const fn icon_name(self) -> &'static str {
        match self {
            Self::Queued => "clock",
            Self::Running => "loader",
            Self::Suspended => "pause",
            Self::Completed => "check-circle",
            Self::Failed => "alert-circle",
            Self::Skipped => "x",
            Self::Retrying => "refresh",
        }
    }

    #[must_use]
    pub const fn is_spinning(self) -> bool {
        matches!(self, Self::Running | Self::Retrying)
    }
}

impl fmt::Display for InvocationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RunOutcome {
    #[default]
    Success,
    Failure,
}

impl RunOutcome {
    #[must_use]
    pub const fn from_success(success: bool) -> Self {
        if success {
            Self::Success
        } else {
            Self::Failure
        }
    }

    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Failure => "Failed",
        }
    }
}

impl From<bool> for RunOutcome {
    fn from(success: bool) -> Self {
        Self::from_success(success)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputOrigin {
    LiveOutput,
    PinnedSample,
    NoOutput,
}

impl OutputOrigin {
    #[must_use]
    pub const fn from_flags(has_live: bool, has_pinned: bool) -> Self {
        if has_live {
            Self::LiveOutput
        } else if has_pinned {
            Self::PinnedSample
        } else {
            Self::NoOutput
        }
    }

    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::LiveOutput => "Live output",
            Self::PinnedSample => "Pinned sample",
            Self::NoOutput => "No output",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadShape {
    Object { field_count: usize },
    Array { element_count: usize },
    String { char_count: usize },
    Number,
    Boolean,
    Null,
}

impl PayloadShape {
    #[must_use]
    pub fn from_value(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Object(map) => Self::Object {
                field_count: map.len(),
            },
            serde_json::Value::Array(arr) => Self::Array {
                element_count: arr.len(),
            },
            serde_json::Value::String(s) => Self::String {
                char_count: s.len(),
            },
            serde_json::Value::Number(_) => Self::Number,
            serde_json::Value::Bool(_) => Self::Boolean,
            serde_json::Value::Null => Self::Null,
        }
    }

    #[must_use]
    pub fn to_display(&self) -> String {
        match self {
            Self::Object { field_count } => format!("object ({field_count})"),
            Self::Array { element_count } => format!("array ({element_count})"),
            Self::String { char_count } => format!("string ({char_count})"),
            Self::Number => "number".to_string(),
            Self::Boolean => "boolean".to_string(),
            Self::Null => "null".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusBadgeStyle {
    pub bg: &'static str,
    pub text: &'static str,
    pub border: &'static str,
}

impl StatusBadgeStyle {
    #[must_use]
    pub const fn new(bg: &'static str, text: &'static str, border: &'static str) -> Self {
        Self { bg, text, border }
    }

    #[must_use]
    pub fn to_classes(self) -> String {
        format!("{} {} {}", self.bg, self.text, self.border)
    }
}

#[must_use]
pub const fn invocation_badge_style(status: InvocationStatus) -> StatusBadgeStyle {
    match status {
        InvocationStatus::Queued => StatusBadgeStyle::new(
            "bg-cyan-500/15",
            "text-cyan-400",
            "border-cyan-500/30",
        ),
        InvocationStatus::Running => StatusBadgeStyle::new(
            "bg-indigo-500/15",
            "text-indigo-400",
            "border-indigo-500/30",
        ),
        InvocationStatus::Suspended => StatusBadgeStyle::new(
            "bg-pink-500/15",
            "text-pink-400",
            "border-pink-500/30",
        ),
        InvocationStatus::Completed => StatusBadgeStyle::new(
            "bg-emerald-500/15",
            "text-emerald-400",
            "border-emerald-500/30",
        ),
        InvocationStatus::Failed => StatusBadgeStyle::new(
            "bg-red-500/15",
            "text-red-400",
            "border-red-500/30",
        ),
        InvocationStatus::Skipped => StatusBadgeStyle::new(
            "bg-slate-500/15",
            "text-slate-300",
            "border-slate-500/30",
        ),
        InvocationStatus::Retrying => StatusBadgeStyle::new(
            "bg-amber-500/15",
            "text-amber-400",
            "border-amber-500/30",
        ),
    }
}

#[must_use]
pub const fn outcome_badge_style(outcome: RunOutcome) -> StatusBadgeStyle {
    match outcome {
        RunOutcome::Success => StatusBadgeStyle::new(
            "bg-emerald-50",
            "text-emerald-700",
            "border-emerald-200",
        ),
        RunOutcome::Failure => StatusBadgeStyle::new(
            "bg-red-50",
            "text-red-700",
            "border-red-200",
        ),
    }
}

#[must_use]
pub const fn outcome_icon_class(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Success => "h-3 w-3 text-emerald-500",
        RunOutcome::Failure => "h-3 w-3 text-red-500",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionEventCategory {
    Status,
    Journal,
    Retry,
}

impl ExecutionEventCategory {
    #[must_use]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Journal => "Journal",
            Self::Retry => "Retry",
        }
    }

    #[must_use]
    pub const fn dot_class(self) -> &'static str {
        match self {
            Self::Status => "bg-indigo-500",
            Self::Journal => "bg-emerald-500",
            Self::Retry => "bg-amber-500",
        }
    }

    #[must_use]
    pub const fn pill_class(self) -> &'static str {
        match self {
            Self::Status => "bg-indigo-500/15 text-indigo-300",
            Self::Journal => "bg-emerald-500/15 text-emerald-300",
            Self::Retry => "bg-amber-500/15 text-amber-300",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationResultCategory {
    HasErrors,
    HasWarningsOnly,
    Valid,
}

impl ValidationResultCategory {
    #[must_use]
    pub const fn from_counts(error_count: usize, warning_count: usize) -> Self {
        if error_count > 0 {
            Self::HasErrors
        } else if warning_count > 0 {
            Self::HasWarningsOnly
        } else {
            Self::Valid
        }
    }

    #[must_use]
    pub const fn header_bg_class(self) -> &'static str {
        match self {
            Self::HasErrors => "bg-red-50 border-red-200",
            Self::HasWarningsOnly => "bg-amber-50 border-amber-200",
            Self::Valid => "bg-emerald-50 border-emerald-200",
        }
    }

    #[must_use]
    pub const fn status_text_class(self) -> &'static str {
        match self {
            Self::HasErrors => "text-[12px] font-medium text-red-700",
            Self::HasWarningsOnly => "text-[12px] font-medium text-amber-700",
            Self::Valid => "text-[12px] font-medium text-emerald-700",
        }
    }

    #[must_use]
    pub const fn badge_class(self) -> &'static str {
        match self {
            Self::HasErrors => "bg-red-100 text-red-700",
            _ => "bg-amber-100 text-amber-700",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseState {
    Collapsed,
    Expanded,
}

impl CollapseState {
    #[must_use]
    pub const fn from_bool(collapsed: bool) -> Self {
        if collapsed {
            Self::Collapsed
        } else {
            Self::Expanded
        }
    }

    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        matches!(self, Self::Collapsed)
    }

    pub fn toggle(&mut self) {
        *self = match self {
            Self::Collapsed => Self::Expanded,
            Self::Expanded => Self::Collapsed,
        };
    }
}

#[must_use]
pub const fn panel_height_class(state: CollapseState) -> &'static str {
    match state {
        CollapseState::Collapsed => "h-10",
        CollapseState::Expanded => "h-[280px]",
    }
}

#[must_use]
pub const fn chevron_rotation_class(state: CollapseState) -> &'static str {
    match state {
        CollapseState::Collapsed => "-rotate-90",
        CollapseState::Expanded => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn http_method_parse_is_case_insensitive() {
        assert_eq!(HttpMethod::parse("GET"), HttpMethod::Get);
        assert_eq!(HttpMethod::parse("get"), HttpMethod::Get);
        assert_eq!(HttpMethod::parse("Post"), HttpMethod::Post);
        assert_eq!(HttpMethod::parse("UNKNOWN"), HttpMethod::Post);
    }

    #[test]
    fn http_method_as_str_returns_uppercase() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
    }

    #[test]
    fn invocation_status_parse_handles_variants() {
        assert_eq!(InvocationStatus::parse("queued"), Some(InvocationStatus::Queued));
        assert_eq!(InvocationStatus::parse("RUNNING"), Some(InvocationStatus::Running));
        assert_eq!(InvocationStatus::parse(""), None);
        assert_eq!(InvocationStatus::parse("unknown"), None);
    }

    #[test]
    fn invocation_status_spinning_states() {
        assert!(InvocationStatus::Running.is_spinning());
        assert!(InvocationStatus::Retrying.is_spinning());
        assert!(!InvocationStatus::Completed.is_spinning());
    }

    #[test]
    fn run_outcome_from_bool() {
        assert_eq!(RunOutcome::from(true), RunOutcome::Success);
        assert_eq!(RunOutcome::from(false), RunOutcome::Failure);
    }

    #[test]
    fn output_origin_from_flags() {
        assert_eq!(OutputOrigin::from_flags(true, true), OutputOrigin::LiveOutput);
        assert_eq!(OutputOrigin::from_flags(false, true), OutputOrigin::PinnedSample);
        assert_eq!(OutputOrigin::from_flags(false, false), OutputOrigin::NoOutput);
    }

    #[test]
    fn payload_shape_from_json() {
        let obj = json!({"a": 1, "b": 2});
        let shape = PayloadShape::from_value(&obj);
        assert!(matches!(shape, PayloadShape::Object { field_count: 2 }));

        let arr = json!([1, 2, 3]);
        let shape = PayloadShape::from_value(&arr);
        assert!(matches!(shape, PayloadShape::Array { element_count: 3 }));

        let s = json!("hello");
        let shape = PayloadShape::from_value(&s);
        assert!(matches!(shape, PayloadShape::String { char_count: 5 }));
    }

    #[test]
    fn collapse_state_toggle() {
        let mut state = CollapseState::Collapsed;
        state.toggle();
        assert_eq!(state, CollapseState::Expanded);
        state.toggle();
        assert_eq!(state, CollapseState::Collapsed);
    }

    #[test]
    fn validation_result_category_from_counts() {
        assert_eq!(
            ValidationResultCategory::from_counts(1, 0),
            ValidationResultCategory::HasErrors
        );
        assert_eq!(
            ValidationResultCategory::from_counts(0, 2),
            ValidationResultCategory::HasWarningsOnly
        );
        assert_eq!(
            ValidationResultCategory::from_counts(0, 0),
            ValidationResultCategory::Valid
        );
    }
}
