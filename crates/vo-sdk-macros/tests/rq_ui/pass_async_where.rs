use vo_sdk_macros::task_macro as task;

#[task]
async fn async_with_where<T>()
where
    T: Send,
{
}
