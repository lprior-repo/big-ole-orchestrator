with open("crates/vo-actor/src/timer_supervisor_tests.rs", "r") as f:
    content = f.read()

# Fix unused work_queue at exactly the place it happens in timer_delete_before_dispatch_returns_ok_on_success
lines = content.split('\n')
for i, line in enumerate(lines):
    if "let work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());" in line:
        # Check if work_queue is used in the next 10 lines
        used = False
        for j in range(1, 10):
            if i + j < len(lines) and "work_queue" in lines[i+j]:
                used = True
                break
        if not used:
            lines[i] = line.replace("let work_queue", "let _work_queue")

content = '\n'.join(lines)

# Fix assert_eq!
content = content.replace("assert_eq!(result, true);", "assert!(result);")
content = content.replace("assert_eq!(result, false);", "assert!(!result);")
content = content.replace('assert_eq!(\n            result, true,\n            "verify_dual_clock should return true at boundary fire_at = now"\n        );', 'assert!(\n            result,\n            "verify_dual_clock should return true at boundary fire_at = now"\n        );')
content = content.replace('assert_eq!(\n            result, true,\n            "verify_dual_clock should return true at boundary elapsed = now"\n        );', 'assert!(\n            result,\n            "verify_dual_clock should return true at boundary elapsed = now"\n        );')
content = content.replace('assert_eq!(\n            result, true,\n            "is_overdue should return true when one ms past boundary"\n        );', 'assert!(\n            result,\n            "is_overdue should return true when one ms past boundary"\n        );')

with open("crates/vo-actor/src/timer_supervisor_tests.rs", "w") as f:
    f.write(content)

with open("crates/vo-actor/src/reanimator.rs", "r") as f:
    rcontent = f.read()

rcontent = rcontent.replace("    impl MockWorkQueue {\n        pub fn new() -> Self {", "    impl Default for MockWorkQueue {\n        fn default() -> Self {\n            Self::new()\n        }\n    }\n\n    impl MockWorkQueue {\n        pub fn new() -> Self {")

with open("crates/vo-actor/src/reanimator.rs", "w") as f:
    f.write(rcontent)

