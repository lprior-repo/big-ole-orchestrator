use dioxus::prelude::*;

use super::set_a::{
    ClockIcon, CodeIcon, DatabaseIcon, GitBranchIcon, GitForkIcon, GlobeIcon, MailIcon, MergeIcon,
    MessageSquareIcon, RepeatIcon, ShuffleIcon, SparklesIcon, WebhookIcon,
};
use super::set_b::{CheckCircleIcon, FileOutputIcon, LoaderIcon, SendIcon};
use super::set_c::{ServerIcon, XIcon, ZapIcon};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    Server,
    Zap,
    Webhook,
    Clock,
    Mail,
    Globe,
    Database,
    Shuffle,
    Code,
    Sparkles,
    GitBranch,
    GitFork,
    Repeat,
    Merge,
    MessageSquare,
    Send,
    FileOutput,
    Loader,
    CheckCircle,
    X,
    Target,
    Radio,
    Bell,
    Alarm,
    Timer,
    Play,
    Kafka,
    Undo,
    Info,
}

impl IconId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Zap => "zap",
            Self::Webhook => "webhook",
            Self::Clock => "clock",
            Self::Mail => "mail",
            Self::Globe => "globe",
            Self::Database => "database",
            Self::Shuffle => "shuffle",
            Self::Code => "code",
            Self::Sparkles => "sparkles",
            Self::GitBranch => "git-branch",
            Self::GitFork => "git-fork",
            Self::Repeat => "repeat",
            Self::Merge => "merge",
            Self::MessageSquare => "message-square",
            Self::Send => "send",
            Self::FileOutput => "file-output",
            Self::Loader => "loader",
            Self::CheckCircle => "check-circle",
            Self::X => "x",
            Self::Target => "target",
            Self::Radio => "radio",
            Self::Bell => "bell",
            Self::Alarm => "alarm",
            Self::Timer => "timer",
            Self::Play => "play",
            Self::Kafka => "kafka",
            Self::Undo => "undo",
            Self::Info => "info",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "server" => Some(Self::Server),
            "zap" => Some(Self::Zap),
            "webhook" => Some(Self::Webhook),
            "clock" => Some(Self::Clock),
            "mail" => Some(Self::Mail),
            "globe" => Some(Self::Globe),
            "database" => Some(Self::Database),
            "shuffle" => Some(Self::Shuffle),
            "code" => Some(Self::Code),
            "sparkles" => Some(Self::Sparkles),
            "git-branch" => Some(Self::GitBranch),
            "git-fork" => Some(Self::GitFork),
            "repeat" => Some(Self::Repeat),
            "merge" => Some(Self::Merge),
            "message-square" => Some(Self::MessageSquare),
            "send" => Some(Self::Send),
            "file-output" => Some(Self::FileOutput),
            "loader" => Some(Self::Loader),
            "check-circle" => Some(Self::CheckCircle),
            "x" => Some(Self::X),
            "target" => Some(Self::Target),
            "radio" => Some(Self::Radio),
            "bell" => Some(Self::Bell),
            "alarm" => Some(Self::Alarm),
            "timer" => Some(Self::Timer),
            "play" => Some(Self::Play),
            "kafka" => Some(Self::Kafka),
            "undo" => Some(Self::Undo),
            "info" => Some(Self::Info),
            _ => None,
        }
    }
}

impl std::fmt::Display for IconId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for IconId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s).ok_or_else(|| format!("Unknown icon: {s}"))
    }
}

pub fn icon(icon_id: IconId, class: String) -> Element {
    match icon_id {
        IconId::Server => rsx! { ServerIcon { class } },
        IconId::Zap => rsx! { ZapIcon { class } },
        IconId::Webhook => rsx! { WebhookIcon { class } },
        IconId::Clock => rsx! { ClockIcon { class } },
        IconId::Mail => rsx! { MailIcon { class } },
        IconId::Globe => rsx! { GlobeIcon { class } },
        IconId::Database => rsx! { DatabaseIcon { class } },
        IconId::Shuffle => rsx! { ShuffleIcon { class } },
        IconId::Code => rsx! { CodeIcon { class } },
        IconId::Sparkles => rsx! { SparklesIcon { class } },
        IconId::GitBranch => rsx! { GitBranchIcon { class } },
        IconId::GitFork => rsx! { GitForkIcon { class } },
        IconId::Repeat => rsx! { RepeatIcon { class } },
        IconId::Merge => rsx! { MergeIcon { class } },
        IconId::MessageSquare => rsx! { MessageSquareIcon { class } },
        IconId::Send => rsx! { SendIcon { class } },
        IconId::FileOutput => rsx! { FileOutputIcon { class } },
        IconId::Loader => rsx! { LoaderIcon { class } },
        IconId::CheckCircle => rsx! { CheckCircleIcon { class } },
        IconId::X => rsx! { XIcon { class } },
        IconId::Target | IconId::Radio | IconId::Bell | IconId::Alarm | IconId::Timer 
        | IconId::Play | IconId::Kafka | IconId::Undo | IconId::Info => rsx! {
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "{class}",
                circle { cx: "12", cy: "12", r: "10" }
            }
        },
    }
}

pub fn icon_by_name(name: &str, class: String) -> Element {
    match IconId::from_str(name) {
        Some(id) => icon(id, class),
        None => rsx! {
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "{class}",
                circle { cx: "12", cy: "12", r: "10" }
            }
        },
    }
}
