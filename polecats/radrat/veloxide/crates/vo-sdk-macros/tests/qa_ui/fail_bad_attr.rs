use vo_sdk_macros::task_macro as task;

#[task(timeout = 30)]
fn with_bad_attr() {}

fn main() {}
