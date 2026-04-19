use vo_sdk_macros::task_macro as task;

#[task]
async fn fetch_data() -> Result<(), std::io::Error> {
    Ok(())
}
