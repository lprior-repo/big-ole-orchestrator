use vo_sdk_macros::task_macro as task;

#[task]
async fn async_where() where (): Sized {}
