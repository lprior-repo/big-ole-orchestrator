# Test Plan: vel-u0e (CLI Scaffold)

## Summary
- Behaviors identified: 21
- Trophy allocation: 72 unit / 35 integration / 5 e2e / 15 proptest (Total 127 tests)
- Proptest invariants: 15
- Fuzz targets: 2
- Kani harnesses: 1
- Target Mutation Kill Rate: ≥90%

## 1. Behavior Inventory

1.  `interpret_cli_from` parses valid arguments into `Cli` struct when given valid command line args.
2.  `interpret_cli_from` returns `clap::Error` when given invalid arguments.
3.  `interpret_cli_from` returns `clap::Error` when missing required arguments.
4.  `interpret_cli_from` handles `--help` flag by returning a specific `clap::Error` variant (DisplayHelp).
5.  `interpret_cli_from` handles `--version` flag by returning a specific `clap::Error` variant (DisplayVersion).
6.  `dispatch` routes to the correct handler based on the parsed `Cli` subcommand.
7.  `dispatch` returns `CliError::Dispatch` when a subcommand handler fails.
8.  `dispatch` returns `Ok(())` when a subcommand handler succeeds.
9.  `map_error_to_exit_code` returns `2` (or appropriate code) for `CliError::Clap` errors indicating bad usage.
10. `map_error_to_exit_code` returns `0` for `CliError::Clap` errors indicating help or version output.
11. `map_error_to_exit_code` returns specific non-zero codes for domain errors (`InvalidNumeric`, `InvalidNatsUrl`, `Dispatch`).
12. `map_error_to_exit_code` exhaustively handles every `clap::ErrorKind` deterministically.
13. `parse_strict_numeric` returns `u64` when given a valid positive integer string.
14. `parse_strict_numeric` returns `CliError::InvalidNumeric` when given a string with a leading `+` sign.
15. `parse_strict_numeric` returns `CliError::InvalidNumeric` when given a string with a leading `-` sign.
16. `parse_strict_numeric` returns `CliError::InvalidNumeric` when given a non-numeric string.
17. `parse_nats_url` returns `NatsUrl` when given a valid hostname and port.
18. `parse_nats_url` returns `NatsUrl` when given a valid hostname without a port.
19. `parse_nats_url` returns `CliError::InvalidNatsUrl` when given an empty host.
20. `parse_nats_url` returns `CliError::InvalidNatsUrl` when given a port out of bounds (0).
21. `parse_nats_url` returns `CliError::InvalidNatsUrl` when given a port out of bounds (65536).

## 2. Trophy Allocation

*   **Unit Tests (72)**: Cover all pure functions (`map_error_to_exit_code`, `parse_strict_numeric`, `parse_nats_url`). This includes exhaustive coverage of every `fmt::Display` match arm for all error variants, ensuring formatting never panics and produces the expected string.
*   **Integration Tests (35)**: Cover `interpret_cli_from` and `dispatch` using real `clap` parsing and routing logic, verifying the boundary between the CLI framework and the application.
*   **Proptest (15)**: Property-based testing for the parsers (`parse_strict_numeric`, `parse_nats_url`) to ensure they handle arbitrary strings correctly without panicking and uphold invariants.
*   **E2E Tests (5)**: A small number of tests invoking the compiled binary to verify `main.rs` wiring (exit codes, stdout/stderr output for help/version).

## 3. BDD Scenarios

### Behavior: CLI Parsing Success
Given: A list of valid command-line argument strings `["veloxide", "serve", "--port", "8080"]`
When: `interpret_cli_from` is called
Then: Returns `Ok(Cli { command: Command::Serve { port: 8080 } })`

### Behavior: CLI Parsing Failure (Invalid Argument)
Given: A list of command-line argument strings `["veloxide", "--unknown-flag"]`
When: `interpret_cli_from` is called
Then: Returns `Err(clap::Error)` with kind `clap::error::ErrorKind::UnknownArgument`.

### Behavior: CLI Parsing Failure (Missing Required)
Given: A list of command-line argument strings `["veloxide", "serve"]` (assuming port is required)
When: `interpret_cli_from` is called
Then: Returns `Err(clap::Error)` with kind `clap::error::ErrorKind::MissingRequiredArgument`.

### Behavior: CLI Parsing Help Flag
Given: A list of command-line argument strings `["veloxide", "--help"]`
When: `interpret_cli_from` is called
Then: Returns `Err(clap::Error)` with kind `clap::error::ErrorKind::DisplayHelp`.

### Behavior: CLI Parsing Version Flag
Given: A list of command-line argument strings `["veloxide", "--version"]`
When: `interpret_cli_from` is called
Then: Returns `Err(clap::Error)` with kind `clap::error::ErrorKind::DisplayVersion`.

### Behavior: Dispatch Routing Success
Given: A parsed `Cli { command: Command::Serve { port: 8080 } }`
When: `dispatch` is called
Then: Returns `Ok(())`.

### Behavior: Dispatch Routing Failure
Given: A parsed `Cli` that triggers a failing subcommand
When: `dispatch` is called
Then: Returns `Err(CliError::Dispatch("subcommand failed".to_string()))`.

### Behavior: Error Mapping - Clap DisplayHelp
Given: A `CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayHelp))`
When: `map_error_to_exit_code` is called
Then: Returns `0`.

### Behavior: Error Mapping - Clap DisplayVersion
Given: A `CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayVersion))`
When: `map_error_to_exit_code` is called
Then: Returns `0`.

### Behavior: Error Mapping - Clap Usage Error
Given: A `CliError::Clap(clap::Error::new(clap::error::ErrorKind::InvalidValue))`
When: `map_error_to_exit_code` is called
Then: Returns `2`.

### Behavior: Error Mapping - Invalid Numeric
Given: A `CliError::InvalidNumeric("invalid".to_string())`
When: `map_error_to_exit_code` is called
Then: Returns `1`.

### Behavior: Error Mapping - Invalid NATS URL
Given: A `CliError::InvalidNatsUrl("invalid".to_string())`
When: `map_error_to_exit_code` is called
Then: Returns `1`.

### Behavior: Error Mapping - Dispatch Failure
Given: A `CliError::Dispatch("failed".to_string())`
When: `map_error_to_exit_code` is called
Then: Returns `1`.

### Behavior: Strict Numeric Parsing Success (Minimum)
Given: The string `"0"`
When: `parse_strict_numeric` is called
Then: Returns `Ok(0)`.

### Behavior: Strict Numeric Parsing Success (One)
Given: The string `"1"`
When: `parse_strict_numeric` is called
Then: Returns `Ok(1)`.

### Behavior: Strict Numeric Parsing Success (Maximum)
Given: The string `"18446744073709551615"` (u64::MAX)
When: `parse_strict_numeric` is called
Then: Returns `Ok(18446744073709551615)`.

### Behavior: Strict Numeric Parsing Failure (Overflow)
Given: The string `"18446744073709551616"` (u64::MAX + 1)
When: `parse_strict_numeric` is called
Then: Returns `Err(CliError::InvalidNumeric("Value exceeds u64 maximum".to_string()))`.

### Behavior: Strict Numeric Parsing Failure (Leading Plus)
Given: The string `"+42"`
When: `parse_strict_numeric` is called
Then: Returns `Err(CliError::InvalidNumeric("Leading '+' is not allowed".to_string()))`.

### Behavior: Strict Numeric Parsing Failure (Leading Minus)
Given: The string `"-42"`
When: `parse_strict_numeric` is called
Then: Returns `Err(CliError::InvalidNumeric("Leading '-' is not allowed".to_string()))`.

### Behavior: Strict Numeric Parsing Failure (Non-numeric)
Given: The string `"42abc"`
When: `parse_strict_numeric` is called
Then: Returns `Err(CliError::InvalidNumeric("Invalid numeric format".to_string()))`.

### Behavior: NATS URL Parsing Success (Host Only)
Given: The string `"localhost"`
When: `parse_nats_url` is called
Then: Returns `Ok(NatsUrl { host: "localhost".to_string(), port: None })`.

### Behavior: NATS URL Parsing Success (Host and Port)
Given: The string `"localhost:4222"`
When: `parse_nats_url` is called
Then: Returns `Ok(NatsUrl { host: "localhost".to_string(), port: Some(4222) })`.

### Behavior: NATS URL Parsing Success (Minimum Port)
Given: The string `"localhost:1"`
When: `parse_nats_url` is called
Then: Returns `Ok(NatsUrl { host: "localhost".to_string(), port: Some(1) })`.

### Behavior: NATS URL Parsing Success (Maximum Port)
Given: The string `"localhost:65535"`
When: `parse_nats_url` is called
Then: Returns `Ok(NatsUrl { host: "localhost".to_string(), port: Some(65535) })`.

### Behavior: NATS URL Parsing Failure (Empty Host)
Given: The string `":4222"`
When: `parse_nats_url` is called
Then: Returns `Err(CliError::InvalidNatsUrl("Host cannot be empty".to_string()))`.

### Behavior: NATS URL Parsing Failure (Port 0)
Given: The string `"localhost:0"`
When: `parse_nats_url` is called
Then: Returns `Err(CliError::InvalidNatsUrl("Port must be between 1 and 65535".to_string()))`.

### Behavior: NATS URL Parsing Failure (Port 65536)
Given: The string `"localhost:65536"`
When: `parse_nats_url` is called
Then: Returns `Err(CliError::InvalidNatsUrl("Port must be between 1 and 65535".to_string()))`.

### Behavior: Error Display - InvalidNumeric
Given: A `CliError::InvalidNumeric("bad number".to_string())`
When: `to_string()` is called
Then: Returns exactly `"Invalid numeric value: bad number"`.

### Behavior: Error Display - InvalidNatsUrl
Given: A `CliError::InvalidNatsUrl("bad url".to_string())`
When: `to_string()` is called
Then: Returns exactly `"Invalid NATS URL: bad url"`.

### Behavior: Error Display - Dispatch
Given: A `CliError::Dispatch("dispatch failed".to_string())`
When: `to_string()` is called
Then: Returns exactly `"Command dispatch failed: dispatch failed"`.

### Behavior: Error Display - Clap
Given: A `CliError::Clap(clap_err)`
When: `to_string()` is called
Then: Returns exactly the output of `clap_err.to_string()`.

## 4. Proptest Invariants

### Proptest: `parse_strict_numeric`
Invariant: Never panics on any valid UTF-8 string.
Strategy: Arbitrary `String`.
Anti-invariant: Any string containing `+` or `-` at index 0 must return `Err`.

### Proptest: `parse_nats_url`
Invariant: Never panics on any valid UTF-8 string.
Strategy: Arbitrary `String`.
Anti-invariant: Any string ending in `:0` or `:65536` must return `Err`.

## 5. Fuzz Targets

### Fuzz Target: CLI Argument Parsing
Input type: `&[&str]` (converted to `OsString`)
Risk: Panic or OOM on extremely long or deeply nested CLI arguments.
Corpus seeds: Standard valid commands, deeply nested subcommands, huge strings.

### Fuzz Target: NATS URL Parsing
Input type: `&str`
Risk: Panic on malformed URLs, excessive memory allocation.
Corpus seeds: `"localhost:4222"`, `"nats://..."`, `""`, a string of 1MB of `':'`.

## 6. Kani Harnesses

### Kani Harness: Error to Exit Code Mapping
Property: `map_error_to_exit_code` never panics for any possible constructed `CliError` or `clap::Error`.
Bound: Depth 3.
Rationale: Ensures the top-level error handler is bulletproof and will reliably terminate the process with a valid exit code rather than aborting.

## 7. Mutation Checkpoints

Critical mutations to survive:
- Changing `port < 1` to `port <= 1` in NATS URL parsing must be caught by `parse_nats_url_rejects_port_0`.
- Changing `port > 65535` to `port >= 65535` must be caught by `parse_nats_url_rejects_port_65536`.
- Removing the `starts_with('+')` check in strict numeric parsing must be caught by `parse_strict_numeric_rejects_leading_plus`.
- Modifying the exit code returned for `DisplayHelp` from `0` to `1` must be caught by the corresponding mapping test.
- Returning a default `Cli` struct in `interpret_cli_from` must be caught by `CLI Parsing Success` asserting the exact `Cli` values.

Threshold: 90% mutation kill rate minimum.
Coverage: 90% line coverage minimum.

## 8. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| Numeric Happy Path | "123" | `Ok(123)` | Unit |
| Numeric Happy Min | "0" | `Ok(0)` | Unit |
| Numeric Happy Max | "18446744073709551615" | `Ok(18446744073709551615)` | Unit |
| Numeric Error: Plus | "+1" | `Err(InvalidNumeric("..."))` | Unit |
| Numeric Error: Minus | "-1" | `Err(InvalidNumeric("..."))` | Unit |
| Numeric Error: Alpha | "abc" | `Err(InvalidNumeric("..."))` | Unit |
| Numeric Error: Overflow | "18446744073709551616" | `Err(InvalidNumeric("..."))` | Unit |
| NATS Happy Path | "host:123" | `Ok(NatsUrl { host: "host", port: 123 })` | Unit |
| NATS Happy Path No Port | "host" | `Ok(NatsUrl { host: "host", port: None })` | Unit |
| NATS Happy Min Port | "host:1" | `Ok(NatsUrl { host: "host", port: 1 })` | Unit |
| NATS Happy Max Port | "host:65535" | `Ok(NatsUrl { host: "host", port: 65535 })` | Unit |
| NATS Error: Port 0 | "host:0" | `Err(InvalidNatsUrl("..."))` | Unit |
| NATS Error: Port 65536 | "host:65536" | `Err(InvalidNatsUrl("..."))` | Unit |
| NATS Error: Empty Host | ":123" | `Err(InvalidNatsUrl("..."))` | Unit |
| Error Formatting: Dispatch | `CliError::Dispatch("err")` | Exact String Match | Unit |
| Error Formatting: InvalidNum | `CliError::InvalidNumeric("err")` | Exact String Match | Unit |
| Error Formatting: InvalidNats | `CliError::InvalidNatsUrl("err")` | Exact String Match | Unit |
| Error Formatting: Clap | `CliError::Clap(err)` | Exact String Match | Unit |
| CLI Help Flag | `["cmd", "--help"]` | `Err(DisplayHelp)` | Integration|
| Arbitrary Strings | Any string | No Panic | Proptest |
