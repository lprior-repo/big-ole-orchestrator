#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

// Rule stubs — each rule will be implemented in its own bead.
// WTF-L001: non-deterministic-time
// WTF-L002: non-deterministic-random
// WTF-L003: direct-async-io
// WTF-L004: ctx-in-closure
// WTF-L005: tokio-spawn-in-workflow
// WTF-L006: std-thread-spawn-in-workflow
