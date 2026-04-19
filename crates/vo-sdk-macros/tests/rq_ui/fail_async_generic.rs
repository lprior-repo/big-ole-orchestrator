use vo_sdk_macros::task_macro as task;

#[task]
async fn my_task<'a, T>() where T: Send + 'a {}
