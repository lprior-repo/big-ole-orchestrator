# Findings: tw-9fxj - Replace LazyLock+expect in TaskStore with fallible init

## Issue
`task_store.rs:121` crashes process on corrupt JSON file. The `LazyLock` with `.expect()` panics instead of returning an error.

## Root Cause
Location: `hardline/crates/cli/src/commands/task_store.rs:119-128`

```rust
static TASK_STORE: LazyLock<Arc<TaskStore>> =
    LazyLock::new(|| {
        Arc::new(TaskStore::load().expect(
            "Fatal: failed to initialize task store — check file permissions and disk state",
        ))
    });

pub fn get_task_store() -> Arc<TaskStore> {
    TASK_STORE.clone()
}
```

`TaskStore::load()` returns `CoreResult<Self>` (fallible), but `LazyLock` with `.expect()` converts this into a panic on any error (corrupt JSON, permissions issues, etc.).

## Fix Applied

### 1. Changed static storage from LazyLock to OnceCell
```rust
// Before
use std::sync::LazyLock;
static TASK_STORE: LazyLock<Arc<TaskStore>> = LazyLock::new(|| { ... });

// After
use once_cell::sync::OnceCell;
static TASK_STORE: OnceCell<Arc<TaskStore>> = OnceCell::new();
```

### 2. Changed get_task_store() to return CoreResult<Arc<TaskStore>>
```rust
// Before
pub fn get_task_store() -> Arc<TaskStore> {
    TASK_STORE.clone()
}

// After
pub fn get_task_store() -> CoreResult<Arc<TaskStore>> {
    TASK_STORE
        .get_or_try_init(|| TaskStore::load().map(Arc::new))
        .cloned()
}
```

### 3. Updated all call sites to handle Result
All 6 call sites in `actions.rs` changed from:
```rust
let store = get_task_store();
```
to:
```rust
let store = get_task_store()?;
```

## Files Modified
- `hardline/polecats/dust/hardline/crates/cli/src/commands/task_store.rs` - Changed LazyLock to OnceCell, made get_task_store() fallible
- `hardline/polecats/dust/hardline/crates/cli/src/commands/handlers/task/actions.rs` - Updated 6 call sites with `?`

## Verification
`cargo check -p scp-cli` completed successfully.

## Impact
- Process no longer crashes on corrupt JSON file
- Errors are propagated properly to CLI user
- Backward compatible: existing error handling already returns `CoreResult<()>`
