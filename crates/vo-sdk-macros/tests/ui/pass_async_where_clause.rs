use vo_sdk_macros::task_macro as task;

#[task]
async fn my_task() where i32: Send {}