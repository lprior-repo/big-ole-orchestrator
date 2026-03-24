//! wtf-worker — Activity worker SDK and timer firing loop.
//!
//! # Modules
//! - [`queue`]: `ActivityTask` + `WorkQueueConsumer` (pull consumer on `wtf-work`).
//! - [`activity`]: `complete_activity` / `fail_activity` — result reporting via JetStream.
//! - [`timer`]: `TimerRecord` + `run_timer_loop` — fires expired timers from KV.
//! - [`worker`]: `Worker` — high-level handler dispatch loop.
//! - [`builtin`]: Built-in activity handlers (`"echo"`, `"sleep"`).

pub mod activity;
pub mod builtin;
pub mod queue;
pub mod timer;
pub mod worker;

pub use activity::{
    calculate_backoff_delay, complete_activity, fail_activity, retries_exhausted, send_heartbeat,
    HeartbeatSender,
};
pub use builtin::register_defaults;
pub use queue::{enqueue_activity, ActivityTask, WorkQueueConsumer, WORK_STREAM_NAME};
pub use timer::{
    delete_timer, fire_timer, run_timer_loop, store_timer, TimerRecord, TIMER_POLL_INTERVAL,
};
pub use worker::Worker;
