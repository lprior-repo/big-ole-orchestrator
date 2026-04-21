use vo_sdk_macros::task_macro as task;

#[task]
trait Foo {
    fn bar();
}

fn main() {}
