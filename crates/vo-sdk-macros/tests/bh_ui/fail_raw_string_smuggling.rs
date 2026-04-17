use vo_sdk_macros::task_macro as task;

#[task]
fn x() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    unimplemented!()
}
