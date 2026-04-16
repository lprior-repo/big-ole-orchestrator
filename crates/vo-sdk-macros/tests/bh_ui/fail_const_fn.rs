use vo_sdk_macros::task_macro as task;

#[task]
const fn compile_time() -> i32 { 42 }

fn main() {}
