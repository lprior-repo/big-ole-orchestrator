# Implementation Summary: wtf-0iu2

## Files Changed
- `crates/wtf-frontend/src/graph/workflow_node.rs`

## Changes Made

### New Config Structs Added
1. `CtxActivityConfig` - Activity node with name, activity_type, input_schema
2. `CtxSleepConfig` - Sleep node with duration_ms (u64)
3. `CtxWaitSignalConfig` - Wait signal node with signal_name (String)
4. `CtxNowConfig` - Unit struct for timestamp capture

### New Enum Variants Added to WorkflowNode
- `CtxActivity(CtxActivityConfig)`
- `CtxSleep(CtxSleepConfig)`
- `CtxWaitSignal(CtxWaitSignalConfig)`
- `CtxNow(CtxNowConfig)`

### Updated Impl Blocks
- `category()`: CtxActivity -> Durable, CtxSleep -> Timing, CtxWaitSignal -> Signal, CtxNow -> Timing
- `icon()`: CtxActivity -> "zap", CtxSleep -> "moon", CtxWaitSignal -> "bell", CtxNow -> "clock"
- `description()`: Added descriptions for all new variants
- `output_port_type()`: CtxWaitSignal -> Signal port type
- `Display::fmt()`: Added kebab-case strings (ctx-activity, ctx-sleep, ctx-wait-signal, ctx-now)
- `FromStr::from_str()`: Added parsing for all new type strings

### Updated Tests
- `all_node_types()`: Added 4 new type strings
- `given_all_variants_when_counting_then_is_24`: Renamed to `given_all_variants_when_counting_then_is_28` and updated assertion
- `given_all_24_node_types_when_parsing`: Renamed to `given_all_28_node_types_when_parsing`

## Moon Gate Results
- cargo check: PASSED
- cargo clippy: PASSED (only pre-existing warning)
- cargo fmt: PASSED

## Quality Gates
| Gate | Status |
|------|--------|
| Contract | PASSED |
| Test Plan | PASSED |
| Implementation | PASSED |
| Moon | PASSED |
