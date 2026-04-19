use vo_sdk_macros::task_macro as task;

#[task]
union Sneaky { x: i32, y: u32 }

fn main() {}
