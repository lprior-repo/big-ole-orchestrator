use vo_sdk_macros::task_macro as task;

trait MyTrait {
    #[task]
    fn my_fn();
}

fn main() {}
