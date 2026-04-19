use vo_sdk_macros::task_macro as task;

#[task(retries = 3)]
fn my_task() {}

fn main() {}
