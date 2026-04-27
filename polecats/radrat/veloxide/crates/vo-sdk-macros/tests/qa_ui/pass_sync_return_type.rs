use vo_sdk_macros::task_macro as task;

#[task]
fn validate() -> Result<(), std::io::Error> {
    Ok(())
}
