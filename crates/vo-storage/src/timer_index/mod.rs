pub mod key;
pub mod value;
pub mod record;
pub mod storage;
pub mod api;

pub use key::TimerKey;
pub use value::TimerValue;
pub use record::TimerRecord;
pub use storage::Storage;
pub use api::{timer_set, timer_delete, scan_due_timers, poll_expired_timers, scan_all_timers_for_instance};

pub(crate) type ScanResult = Vec<(Vec<u8>, Vec<u8>)>;
