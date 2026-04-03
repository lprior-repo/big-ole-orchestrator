with open("crates/vo-actor/src/lib.rs", "r") as f:
    content = f.read()

replacements = [
    ("resume_on_failed_instance_emits_instanceresumed_and_actor_re_enters_decision", "01H5JYV4XHGSR2F8KZ9BWNRFMA", "01H5JYV4XHGSR2F8KZ9B00F000"),
    ("resume_on_failed_instance_emits_instanceresumed_with_hash_state", "01H5JYV4XHGSR2F8KZ9BWNRFMA", "01H5JYV4XHGSR2F8KZ9B00F000"),
    ("resume_on_failed_instance_transitions_lifecycle_from_failed_to_running", "01H5JYV4XHGSR2F8KZ9BWNRFMA", "01H5JYV4XHGSR2F8KZ9B00F000")
]

for test_name, old_id, new_id in replacements:
    idx = content.find(f"async fn {test_name}() {{")
    if idx == -1:
        print(f"Could not find test {test_name}")
        continue
    end_idx = content.find("async fn", idx + 10)
    if end_idx == -1:
        end_idx = len(content)
    
    test_body = content[idx:end_idx]
    new_test_body = test_body.replace(f'"{old_id}"', f'"{new_id}"')
    content = content[:idx] + new_test_body + content[end_idx:]

with open("crates/vo-actor/src/lib.rs", "w") as f:
    f.write(content)
