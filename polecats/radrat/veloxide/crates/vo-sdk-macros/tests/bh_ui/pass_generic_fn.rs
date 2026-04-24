use vo_sdk_macros::task_macro as task;

#[task]
fn generic_task<T: Default>() -> T {
    T::default()
}
