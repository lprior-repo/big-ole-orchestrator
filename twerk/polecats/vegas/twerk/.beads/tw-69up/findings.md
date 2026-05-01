# Findings: tw-69up - twerk-cli config file loading tests

## Task
Write tests for config file loading with CLI flag overrides in `crates/twerk-cli/src/config.rs`

## Implementation
Created `crates/twerk-cli/src/config.rs` with:
- `Config` struct with `timeout_secs` and `workers` fields
- `Config::from_file()` - loads from TOML config file with proper error handling (path and line number on parse errors)
- `Config::apply_cli_overrides()` - CLI flags override file values
- Default values: timeout=30s, workers=2

## Tests Implemented
1. `test_cli_overrides_file_config` - CLI --timeout=60 overrides file timeout=30, workers preserved from file
2. `test_file_overrides_defaults` - File values override defaults when no CLI flags
3. `test_no_config_file_all_defaults` - Config::default() returns timeout=30, workers=2
4. `test_invalid_config_file_error` - Parse errors include path and line/column info
5. `test_valid_config_file_loading` - Valid TOML file loads correctly
6. `test_cli_overrides_both_file_values` - Both CLI flags override both file values
7. `test_cli_overrides_none_leaves_file_values` - None CLI flags leaves file values unchanged

## Notes
- Worktree at `/home/lewis/gt/twerk/polecats/vegas/twerk/` appears to be misconfigured (not a proper git worktree)
- Files created there are actually tracked in the parent `/home/lewis/gt/` veloxide repo
- Added tests use tempfile for creating temporary TOML config files
