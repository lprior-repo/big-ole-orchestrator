use vo_sdk_macros::task_macro as task;

#[task]
async fn complex<'a, T>() where T: Send + 'a {}
