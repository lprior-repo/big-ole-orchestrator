---
name: no-tdd-guard-config
enabled: true
event: file
action: block
conditions:
  - field: file_path
    operator: regex_match
    pattern: settings(\.local)?\.json$
  - field: new_text
    operator: contains
    pattern: tdd-guard
---

🚫 **tdd-guard configuration is off-limits.**

You are not allowed to add, modify, or remove any `tdd-guard` or `tdd-guard-rust` entries in settings files. The user manages this configuration themselves.

Do not touch it.
