use vo_sdk_macros::task_macro as task;

#[task]
fn x(a: i32, b: i32, c: i32) -> Result<Vec<u8>, Box<dyn std::error::Error>> { unimplemented!() }

fn main() {}
