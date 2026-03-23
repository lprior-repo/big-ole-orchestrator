pub mod heartbeat;
pub mod list;
pub mod signal;
pub mod start;
pub mod status;
pub mod terminate;

use std::time::Duration;

/// Timeout for RPC calls to individual WorkflowInstance actors.
pub const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub use self::heartbeat::handle_heartbeat_expired;
pub use self::list::handle_list_active;
pub use self::signal::handle_signal;
pub use self::start::{handle_start_workflow, StartWorkflowParams};
pub use self::status::handle_get_status;
pub use self::terminate::handle_terminate;
