use vo_sdk_macros::task_macro as task;

#[task(foo)]
#[task(bar)]
fn multi_attr() {}

fn main() {}
