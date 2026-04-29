use vo_sdk_macros::task_macro as task;

#[task(unknown_key = 99)]
fn with_bad_attr() {}

fn main() {}
