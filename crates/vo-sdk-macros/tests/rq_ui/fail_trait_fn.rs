use vo_sdk_macros::task_macro as task;

#[task]
enum NotAFn {
    A,
    B,
}

fn main() {}
