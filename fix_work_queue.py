with open("crates/vo-actor/src/timer_supervisor_tests.rs", "r") as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "let _work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());" in line:
        lines[i] = line.replace("let _work_queue", "let work_queue")

# Now just fix line 604
# Wait, the lines might have shifted if we added Default impl earlier in the file.
# The warning was in `timer_delete_before_dispatch_returns_ok_on_success` or something similar, where `work_queue` is unused.
# Let's find the function where `work_queue` is unused.
# The unused warning said:
# 604 |         let work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());
# Let's just look for lines where `work_queue` is NOT followed by `work_queue` in the next 10 lines.

with open("crates/vo-actor/src/timer_supervisor_tests.rs", "w") as f:
    f.writelines(lines)
