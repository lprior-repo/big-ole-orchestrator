import os
import re
import glob

workspace = "/home/lewis/src/vo-k6u"

def fix_let_underscore(filepath):
    with open(filepath, 'r') as f:
        content = f.read()
    
    # Replace `let _ = expr;` with `expr.unwrap();` if it seems like an error suppression, or just `expr;` if we want to handle.
    # The mandate says: "Explicitly unwrap and assert, or return the Result."
    # Let's replace `let _ = ` with `// let _ =` temporarily? No, unwrap it.
    
    # For UI and engine
    def replacer(match):
        expr = match.group(1)
        # Avoid unwrapping if we don't know it's a Result, but the mandate says "explicitly unwrap and assert"
        # We will use `.expect("suppressed error");` or similar
        return f"{expr}.expect(\"previously suppressed error\");"

    # Wait, `let _ = tx.send(res);` -> `tx.send(res).unwrap();`
    # `let _ = write!(...);` -> `write!(...).unwrap();`
    # Let's do a simple replacement for `let _ = ` -> `.unwrap();`
    # A bit dangerous, let's be careful.
    
    new_content = []
    for line in content.split('\n'):
        if 'let _ = ' in line and not 'catch_unwind' in line and not 'payload' in line and not 'Url::revoke_object_url' in line:
            # Let's just append `.unwrap()` for send and write and store and workflow
            if 'tx.send' in line or 'write!' in line or 'store.' in line or 'workflow.' in line or 'r1.register' in line or 'r2.register' in line or 'discover_graph' in line or 'NonEmptyVec::new_unchecked' in line or 'results.insert' in line or 'visited.insert' in line or 'collapsed.try_write' in line or 'expanded_runs.try_write' in line or 'shutdown_tx' in line:
                line = re.sub(r'let _ = (.*?);', r'\1.unwrap();', line)
            elif 'clipboard().write_text' in line:
                line = re.sub(r'let _ = (.*?);', r'\1.unwrap();', line)
            elif 'write_text_fn.call1' in line:
                line = re.sub(r'let _ = (.*?);', r'\1.unwrap();', line)
        new_content.append(line)
        
    with open(filepath, 'w') as f:
        f.write('\n'.join(new_content))

for root, _, files in os.walk(workspace):
    for file in files:
        if file.endswith('.rs'):
            fix_let_underscore(os.path.join(root, file))

