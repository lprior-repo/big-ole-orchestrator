#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

// ── Data ─────────────────────────────────────────────────────────────────────
// Inert, `Copy`, comparable.  No logic lives here — only the shape of things.

/// All node categories in canonical display order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Category {
    Entry,
    Durable,
    State,
    Flow,
    Timing,
    Signal,
}

impl Category {
    /// Ordered slice used by the sidebar to iterate categories top→bottom.
    pub(super) const ORDER: [Self; 6] = [
        Self::Entry,
        Self::Durable,
        Self::State,
        Self::Flow,
        Self::Timing,
        Self::Signal,
    ];

    /// Human-readable section header.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Entry => "Entry Points",
            Self::Durable => "Actions",
            Self::State => "Memory",
            Self::Flow => "Logic",
            Self::Timing => "Timing",
            Self::Signal => "Signals",
        }
    }

    /// Tailwind dot colour for the category header.
    pub(super) const fn dot_class(self) -> &'static str {
        match self {
            Self::Entry => "bg-blue-400",
            Self::Durable => "bg-green-400",
            Self::State => "bg-cyan-400",
            Self::Flow => "bg-pink-400",
            Self::Timing => "bg-purple-400",
            Self::Signal => "bg-amber-400",
        }
    }

    /// Tailwind icon badge colours for node buttons.
    pub(super) const fn icon_badge_class(self) -> &'static str {
        match self {
            Self::Entry => "bg-blue-100 text-blue-700 border-blue-200",
            Self::Durable => "bg-green-100 text-green-700 border-green-200",
            Self::State => "bg-cyan-100 text-cyan-700 border-cyan-200",
            Self::Flow => "bg-pink-100 text-pink-700 border-pink-200",
            Self::Timing => "bg-purple-100 text-purple-700 border-purple-200",
            Self::Signal => "bg-amber-100 text-amber-700 border-amber-200",
        }
    }
}

/// A static node template — lives in `const` memory, `Copy`able at zero cost.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct NodeTemplate {
    pub(super) node_type: &'static str,
    pub(super) label: &'static str,
    pub(super) description: &'static str,
    pub(super) friendly: &'static str,
    pub(super) tooltip: &'static str,
    pub(super) icon: &'static str,
    pub(super) category: Category,
    pub(super) doc_link: Option<&'static str>,
}

// ── Calculations ─────────────────────────────────────────────────────────────
// Pure functions: time-independent, no side effects.

impl NodeTemplate {
    /// Returns `true` when `query` (already lower-cased by caller) matches any
    /// searchable field on this template.  Empty query always matches.
    pub(super) fn matches_query(self, query: &str) -> bool {
        query.is_empty()
            || self.label.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
            || self.friendly.to_lowercase().contains(query)
    }
}

// ── Static catalogue ─────────────────────────────────────────────────────────

pub(super) const NODE_TEMPLATES: [NodeTemplate; 24] = [
    NodeTemplate {
        node_type: "http-handler",
        label: "HTTP Trigger",
        description: "Handle HTTP/gRPC invocation",
        friendly: "Starts when someone calls this URL",
        tooltip: "This workflow runs when an HTTP request comes in. Use this as the starting point for webhook-based workflows.",
        icon: "globe",
        category: Category::Entry,
        doc_link: Some("#http-trigger"),
    },
    NodeTemplate {
        node_type: "kafka-handler",
        label: "Kafka Trigger",
        description: "Consume events from Kafka topic",
        friendly: "Starts when a message arrives in Kafka",
        tooltip: "This workflow runs when a message is published to a Kafka topic. Great for event-driven architectures.",
        icon: "kafka",
        category: Category::Entry,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "cron-trigger",
        label: "Schedule",
        description: "Schedule periodic execution",
        friendly: "Runs automatically on a schedule",
        tooltip: "This workflow runs on a schedule you define using cron expressions. Use for nightly jobs, reminders, or periodic sync tasks.",
        icon: "clock",
        category: Category::Entry,
        doc_link: Some("#schedule-trigger"),
    },
    NodeTemplate {
        node_type: "workflow-submit",
        label: "Submit Workflow",
        description: "Submit workflow with key",
        friendly: "Start a workflow manually",
        tooltip: "Submit a new workflow execution with a specific key. Useful for triggering long-running processes from other services.",
        icon: "play-circle",
        category: Category::Entry,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "run",
        label: "Run Code",
        description: "ctx.run() - persisted side effect",
        friendly: "Execute custom code (saved for retries)",
        tooltip: "Run custom code that gets saved in the journal. If the workflow retries, the saved result is used instead of re-running the code. Perfect for API calls or any non-deterministic operation.",
        icon: "shield",
        category: Category::Durable,
        doc_link: Some("#contextsideeffects-journaling-results"),
    },
    NodeTemplate {
        node_type: "service-call",
        label: "Call Service",
        description: "Request-response to service",
        friendly: "Call another service and wait for result",
        tooltip: "Call another service and wait for the response. Use this for synchronous operations where you need a result back.",
        icon: "arrow-right",
        category: Category::Durable,
        doc_link: Some("#contextclient-service-communication"),
    },
    NodeTemplate {
        node_type: "object-call",
        label: "Object Call",
        description: "Call virtual object handler",
        friendly: "Read or update data in an object",
        tooltip: "Call a Virtual Object - a stateful service with a key. Objects process one call at a time per key, making them great for counters, carts, or user profiles.",
        icon: "box",
        category: Category::Durable,
        doc_link: Some("#service-types"),
    },
    NodeTemplate {
        node_type: "workflow-call",
        label: "Workflow Call",
        description: "Submit or attach to workflow",
        friendly: "Start or continue a workflow",
        tooltip: "Start a new workflow or attach to an existing one with the same key. Workflows can run for hours or days, waiting for external events.",
        icon: "workflow",
        category: Category::Durable,
        doc_link: Some("#service-types"),
    },
    NodeTemplate {
        node_type: "send-message",
        label: "Send Message",
        description: "Fire-and-forget one-way call",
        friendly: "Send without waiting for response",
        tooltip: "Send a message to a service and continue without waiting. Great for notifications, logging, or triggering background tasks where you don't need a response.",
        icon: "send",
        category: Category::Durable,
        doc_link: Some("#one-way-calls-fire-and-forget"),
    },
    NodeTemplate {
        node_type: "delayed-send",
        label: "Delayed Message",
        description: "Schedule future handler call",
        friendly: "Send after a delay",
        tooltip: "Schedule a message to be sent after a specific delay. Perfect for reminders, timeouts, or delayed processing.",
        icon: "clock-send",
        category: Category::Durable,
        doc_link: Some("#delayed-calls-scheduled"),
    },
    NodeTemplate {
        node_type: "get-state",
        label: "Get State",
        description: "ctx.get() - read persisted state",
        friendly: "Load saved data",
        tooltip: "Load data that was previously saved in this workflow/object. Returns the saved value or a default if nothing was saved.",
        icon: "download",
        category: Category::State,
        doc_link: Some("#contextreadstate-reading-state"),
    },
    NodeTemplate {
        node_type: "set-state",
        label: "Save to Memory",
        description: "ctx.set() - write persisted state",
        friendly: "Save data for later",
        tooltip: "Save data that persists across retries and can be loaded by other steps. Think of it like saving a variable that survives restarts.",
        icon: "upload",
        category: Category::State,
        doc_link: Some("#contextwritestate-writing-state"),
    },
    NodeTemplate {
        node_type: "clear-state",
        label: "Clear Memory",
        description: "ctx.clear() / clearAll()",
        friendly: "Delete saved data",
        tooltip: "Remove saved data. Use 'Clear' to delete one item or 'Clear All' to wipe everything.",
        icon: "eraser",
        category: Category::State,
        doc_link: Some("#contextwritestate-writing-state"),
    },
    NodeTemplate {
        node_type: "condition",
        label: "If / Else",
        description: "Conditional branching",
        friendly: "Go different ways based on a condition",
        tooltip: "Check a condition and go down different paths. The 'true' output runs if the condition is met, 'false' runs otherwise.",
        icon: "git-branch",
        category: Category::Flow,
        doc_link: Some("#common-patterns"),
    },
    NodeTemplate {
        node_type: "switch",
        label: "Switch",
        description: "Multi-path routing",
        friendly: "Go to one of many paths",
        tooltip: "Match a value against multiple cases and go to the matching path. Like a switch statement in code.",
        icon: "git-fork",
        category: Category::Flow,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "loop",
        label: "Loop",
        description: "Iterate over collection",
        friendly: "Repeat for each item in a list",
        tooltip: "Run a set of steps repeatedly for each item in a collection. Great for batch processing or sending multiple notifications.",
        icon: "repeat",
        category: Category::Flow,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "parallel",
        label: "Parallel",
        description: "Promise.all() concurrent steps",
        friendly: "Run multiple steps at once",
        tooltip: "Run multiple steps at the same time and wait for all to complete. Use when you have independent tasks that can run concurrently.",
        icon: "layers",
        category: Category::Flow,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "compensate",
        label: "Compensate",
        description: "Saga compensation / rollback",
        friendly: "Undo operations if something fails",
        tooltip: "Define rollback logic that runs if a later step fails. Part of the Saga pattern for distributed transactions.",
        icon: "undo",
        category: Category::Flow,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "sleep",
        label: "Delay",
        description: "ctx.sleep() - durable pause",
        friendly: "Wait for a period of time",
        tooltip: "Pause the workflow for a specified time. The pause is durable - if the service restarts, the timer continues. Great for timeouts or delayed processing.\n\nNote: Virtual Objects are blocked during sleep.",
        icon: "timer",
        category: Category::Timing,
        doc_link: Some("#contexttimers-scheduling--timers"),
    },
    NodeTemplate {
        node_type: "timeout",
        label: "Timeout",
        description: "orTimeout() - deadline guard",
        friendly: "Give up if something takes too long",
        tooltip: "Set a deadline for an operation. If it doesn't complete in time, fail with a timeout error.",
        icon: "alarm",
        category: Category::Timing,
        doc_link: None,
    },
    NodeTemplate {
        node_type: "durable-promise",
        label: "Wait for Signal",
        description: "ctx.promise() - await external",
        friendly: "Wait for another workflow to send a signal",
        tooltip: "Pause this workflow until another workflow or handler sends a signal. The signal can include data. Great for coordinating between workflows.",
        icon: "sparkles",
        category: Category::Signal,
        doc_link: Some("#contextpromises-durable-promises"),
    },
    NodeTemplate {
        node_type: "awakeable",
        label: "Wait for Webhook",
        description: "Pause for external completion",
        friendly: "Pause until an external service calls back",
        tooltip: "Pause the workflow and generate a webhook URL. When the external service calls the webhook with data, the workflow continues with that data.\n\nPerfect for: Payment callbacks, approval workflows, external processing.",
        icon: "bell",
        category: Category::Signal,
        doc_link: Some("#contextawakeables-callback-pattern"),
    },
    NodeTemplate {
        node_type: "resolve-promise",
        label: "Send Signal",
        description: "Resolve a durable promise",
        friendly: "Signal another workflow to continue",
        tooltip: "Send a signal to a waiting workflow. Can include data that the waiting workflow receives. Use to coordinate between workflows.",
        icon: "check-circle",
        category: Category::Signal,
        doc_link: Some("#contextpromises-durable-promises"),
    },
    NodeTemplate {
        node_type: "signal-handler",
        label: "Signal Handler",
        description: "Shared handler for signals",
        friendly: "Handle incoming signals",
        tooltip: "A special handler that receives signals from other workflows. Define what happens when a signal arrives.",
        icon: "radio",
        category: Category::Signal,
        doc_link: None,
    },
];

// ── Calculations ─────────────────────────────────────────────────────────────

/// Returns the indices into `NODE_TEMPLATES` that belong to `category` and
/// match the lower-cased `query`.  Pure — no allocation beyond the Vec.
pub(super) fn visible_indices(category: Category, query: &str) -> Vec<usize> {
    NODE_TEMPLATES
        .iter()
        .enumerate()
        .filter(|(_, t)| t.category == category && t.matches_query(query))
        .map(|(i, _)| i)
        .collect()
}

/// Returns `true` when no template across the whole catalogue matches `query`.
pub(super) fn no_results(query: &str) -> bool {
    NODE_TEMPLATES.iter().all(|t| !t.matches_query(query))
}
