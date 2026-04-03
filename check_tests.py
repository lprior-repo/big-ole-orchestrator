import re

with open('.beads/vel-kpm/test-plan.md', 'r') as f:
    plan = f.read()

with open('crates/vo-types/src/schema_version_tests.rs', 'r') as f:
    tests = f.read()

plan_funcs = re.findall(r'Test function: `fn ([a-zA-Z0-9_]+)\(\)`', plan)
test_funcs = re.findall(r'fn ([a-zA-Z0-9_]+)\(\)', tests)

missing = set(plan_funcs) - set(test_funcs)
extra = set(test_funcs) - set(plan_funcs)

print("Missing:", missing)
print("Extra:", extra)
print(f"Total plan: {len(plan_funcs)}, Total implemented: {len(test_funcs)}")
