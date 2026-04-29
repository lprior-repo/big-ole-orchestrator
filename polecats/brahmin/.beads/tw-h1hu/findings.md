# Findings: Circuit Breaker for Subprocess Execution (tw-h1hu)

## Implementation Summary

Implemented a circuit breaker pattern for subprocess execution in `vo-executor` crate to prevent wasting resources on broken binaries.

## Location

- **Module**: `crates/vo-executor/src/circuit_breaker.rs`
- **Exports**: `crates/vo-executor/src/lib.rs`

## Key Components

### `SubprocessCircuitBreaker`
Main struct managing circuit breakers per binary path using `DashMap` for thread-safe access.

### `CircuitState` Enum
- `Closed`: Normal operation, all executions allowed
- `Open`: Circuit tripped, executions rejected for 30 seconds
- `HalfOpen`: Testing state, allows one execution

### Constants
- `WINDOW_SIZE`: 10 - number of recent executions to track
- `FAILURE_THRESHOLD`: 0.5 (50%) - failure rate threshold
- `OPEN_DURATION`: 30 seconds

### Key Methods
- `is_available(binary_path)`: Check if circuit allows execution
- `execute(binary_path, f)`: Execute with circuit breaker protection
- `get_state(binary_path)`: Get current circuit state and recent results

### `CircuitBreakerExecutionError<E>`
- `CircuitOpen { binary }`: Execution rejected due to open circuit
- `SubprocessError { source }`: Wraps subprocess error with circuit breaker context

## Behavior

1. Tracks last 10 execution results per binary
2. When failure rate exceeds 50% in the window, opens circuit
3. Logs error-level alert when circuit opens
4. Circuit stays open for 30 seconds
5. After timeout, transitions to half-open
6. In half-open: allows one test execution
7. Success in half-open → closes circuit
8. Failure in half-open → reopens circuit

## Testing

Five unit tests implemented:
- `test_circuit_starts_closed`
- `test_circuit_opens_after_50_percent_failure`
- `test_circuit_half_open_after_timeout`
- `test_circuit_closes_after_successful_half_open_request`
- `test_success_resets_failure_count`

## Usage Example

```rust
let cb = SubprocessCircuitBreaker::new();

let result = cb.execute("/bin/my-binary", || async {
    run_subprocess(config).await
}).await;

match result {
    Ok(output) => { /* handle success */ }
    Err(CircuitBreakerExecutionError::CircuitOpen { binary }) => {
        // Circuit is open, handle appropriately
    }
    Err(CircuitBreakerExecutionError::SubprocessError { source }) => {
        // Subprocess error, handle appropriately
    }
}
```

## Notes

- Uses `dashmap` for concurrent access to circuit breaker state per binary
- Uses `tokio::sync::RwLock` for async-safe state management
- Thread-safe and async-runtime compatible
- Non-blocking async operations throughout