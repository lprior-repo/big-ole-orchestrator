#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeUiState {
    #[default]
    Default,
    Selected,
    Executing,
    Skipped,
}

impl NodeUiState {
    #[must_use]
    pub const fn is_selected(self) -> bool {
        matches!(self, Self::Selected)
    }

    #[must_use]
    pub const fn is_executing(self) -> bool {
        matches!(self, Self::Executing)
    }

    #[must_use]
    pub const fn is_skipped(self) -> bool {
        matches!(self, Self::Skipped)
    }
}

impl fmt::Display for NodeUiState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Selected => write!(f, "selected"),
            Self::Executing => write!(f, "executing"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyString(String);

impl NonEmptyString {
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        Some(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = EmptyStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(EmptyStringError)
    }
}

impl From<NonEmptyString> for String {
    fn from(value: NonEmptyString) -> Self {
        value.0
    }
}

impl std::ops::Deref for NonEmptyString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyStringError;

impl fmt::Display for EmptyStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "string cannot be empty")
    }
}

impl std::error::Error for EmptyStringError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeLabel(NonEmptyString);

impl NodeLabel {
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        NonEmptyString::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for NodeLabel {
    type Error = EmptyStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        NonEmptyString::try_from(value).map(Self)
    }
}

impl From<NodeLabel> for String {
    fn from(value: NodeLabel) -> Self {
        value.0.into_inner()
    }
}

impl fmt::Display for NodeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeIcon {
    Globe,
    Kafka,
    Clock,
    Play,
    Code,
    Call,
    Box,
    GitBranch,
    GitMerge,
    Send,
    Database,
    Save,
    Trash,
    Repeat,
    Layers,
    Undo,
    Moon,
    AlertTriangle,
    Target,
    Radio,
    CheckCircle,
    Bell,
    HelpCircle,
    Shield,
    Download,
    Upload,
    Eraser,
    Timer,
    Alarm,
    ClockSend,
    Sparkles,
    PlayCircle,
    ArrowRight,
    Workflow,
}

impl NodeIcon {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Globe => "globe",
            Self::Kafka => "kafka",
            Self::Clock => "clock",
            Self::Play => "play",
            Self::Code => "code",
            Self::Call => "call",
            Self::Box => "box",
            Self::GitBranch => "git-branch",
            Self::GitMerge => "git-merge",
            Self::Send => "send",
            Self::Database => "database",
            Self::Save => "save",
            Self::Trash => "trash",
            Self::Repeat => "repeat",
            Self::Layers => "layers",
            Self::Undo => "undo",
            Self::Moon => "moon",
            Self::AlertTriangle => "alert-triangle",
            Self::Target => "target",
            Self::Radio => "radio",
            Self::CheckCircle => "check-circle",
            Self::Bell => "bell",
            Self::HelpCircle => "help-circle",
            Self::Shield => "shield",
            Self::Download => "download",
            Self::Upload => "upload",
            Self::Eraser => "eraser",
            Self::Timer => "timer",
            Self::Alarm => "alarm",
            Self::ClockSend => "clock-send",
            Self::Sparkles => "sparkles",
            Self::PlayCircle => "play-circle",
            Self::ArrowRight => "arrow-right",
            Self::Workflow => "workflow",
        }
    }
}

impl fmt::Display for NodeIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for NodeIcon {
    type Err = UnknownIconError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "globe" => Ok(Self::Globe),
            "kafka" => Ok(Self::Kafka),
            "clock" => Ok(Self::Clock),
            "play" => Ok(Self::Play),
            "code" => Ok(Self::Code),
            "call" => Ok(Self::Call),
            "box" => Ok(Self::Box),
            "git-branch" => Ok(Self::GitBranch),
            "git-merge" => Ok(Self::GitMerge),
            "git-fork" => Ok(Self::GitMerge),
            "send" => Ok(Self::Send),
            "database" => Ok(Self::Database),
            "save" => Ok(Self::Save),
            "trash" => Ok(Self::Trash),
            "repeat" => Ok(Self::Repeat),
            "layers" => Ok(Self::Layers),
            "undo" => Ok(Self::Undo),
            "moon" => Ok(Self::Moon),
            "alert-triangle" => Ok(Self::AlertTriangle),
            "target" => Ok(Self::Target),
            "radio" => Ok(Self::Radio),
            "check-circle" => Ok(Self::CheckCircle),
            "bell" => Ok(Self::Bell),
            "help-circle" => Ok(Self::HelpCircle),
            "shield" => Ok(Self::Shield),
            "download" => Ok(Self::Download),
            "upload" => Ok(Self::Upload),
            "eraser" => Ok(Self::Eraser),
            "timer" => Ok(Self::Timer),
            "alarm" => Ok(Self::Alarm),
            "clock-send" => Ok(Self::ClockSend),
            "sparkles" => Ok(Self::Sparkles),
            "play-circle" => Ok(Self::PlayCircle),
            "arrow-right" => Ok(Self::ArrowRight),
            "workflow" => Ok(Self::Workflow),
            _ => Err(UnknownIconError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownIconError(pub String);

impl fmt::Display for UnknownIconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown icon: {}", self.0)
    }
}

impl std::error::Error for UnknownIconError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunOutcome {
    Success,
    Failure,
}

impl RunOutcome {
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServiceName(NonEmptyString);

impl ServiceName {
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        NonEmptyString::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for ServiceName {
    type Error = EmptyStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        NonEmptyString::try_from(value).map(Self)
    }
}

impl From<ServiceName> for String {
    fn from(value: ServiceName) -> Self {
        value.0.into_inner()
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StateKey(NonEmptyString);

impl StateKey {
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        NonEmptyString::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for StateKey {
    type Error = EmptyStringError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        NonEmptyString::try_from(value).map(Self)
    }
}

impl From<StateKey> for String {
    fn from(value: StateKey) -> Self {
        value.0.into_inner()
    }
}

impl fmt::Display for StateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PositiveDuration {
    pub millis: u64,
}

impl PositiveDuration {
    #[must_use]
    pub const fn from_millis(millis: u64) -> Option<Self> {
        if millis == 0 {
            return None;
        }
        Some(Self { millis })
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.millis
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub category: super::NodeCategory,
    pub label: &'static str,
    pub icon: NodeIcon,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod node_ui_state {
        use super::*;

        #[test]
        fn default_is_default() {
            assert_eq!(NodeUiState::default(), NodeUiState::Default);
        }

        #[test]
        fn selected_is_selected() {
            assert!(NodeUiState::Selected.is_selected());
            assert!(!NodeUiState::Default.is_selected());
        }

        #[test]
        fn executing_is_executing() {
            assert!(NodeUiState::Executing.is_executing());
            assert!(!NodeUiState::Default.is_executing());
        }

        #[test]
        fn skipped_is_skipped() {
            assert!(NodeUiState::Skipped.is_skipped());
            assert!(!NodeUiState::Default.is_skipped());
        }
    }

    mod non_empty_string {
        use super::*;

        #[test]
        fn empty_string_is_none() {
            assert!(NonEmptyString::new(String::new()).is_none());
        }

        #[test]
        fn non_empty_string_is_some() {
            let s = NonEmptyString::new("hello".to_string());
            assert!(s.is_some());
            assert_eq!(s.unwrap().as_str(), "hello");
        }

        #[test]
        fn try_from_empty_errors() {
            assert!(NonEmptyString::try_from(String::new()).is_err());
        }

        #[test]
        fn into_inner_returns_original() {
            let s = NonEmptyString::new("test".to_string()).unwrap();
            assert_eq!(s.into_inner(), "test");
        }
    }

    mod node_icon {
        use super::*;

        #[test]
        fn from_str_known_icons() {
            assert_eq!(NodeIcon::from_str("globe"), Ok(NodeIcon::Globe));
            assert_eq!(NodeIcon::from_str("clock"), Ok(NodeIcon::Clock));
            assert_eq!(NodeIcon::from_str("git-branch"), Ok(NodeIcon::GitBranch));
        }

        #[test]
        fn from_str_unknown_errors() {
            assert!(NodeIcon::from_str("unknown").is_err());
        }

        #[test]
        fn display_matches_as_str() {
            assert_eq!(NodeIcon::Globe.to_string(), "globe");
            assert_eq!(NodeIcon::GitBranch.to_string(), "git-branch");
        }
    }

    mod positive_duration {
        use super::*;

        #[test]
        fn zero_is_none() {
            assert!(PositiveDuration::from_millis(0).is_none());
        }

        #[test]
        fn positive_is_some() {
            let d = PositiveDuration::from_millis(100);
            assert!(d.is_some());
            assert_eq!(d.unwrap().as_millis(), 100);
        }
    }

    mod service_name {
        use super::*;

        #[test]
        fn empty_is_none() {
            assert!(ServiceName::new(String::new()).is_none());
        }

        #[test]
        fn non_empty_works() {
            let name = ServiceName::new("my-service".to_string()).unwrap();
            assert_eq!(name.as_str(), "my-service");
        }
    }

    mod state_key {
        use super::*;

        #[test]
        fn empty_is_none() {
            assert!(StateKey::new(String::new()).is_none());
        }

        #[test]
        fn non_empty_works() {
            let key = StateKey::new("user:123".to_string()).unwrap();
            assert_eq!(key.as_str(), "user:123");
        }
    }
}
