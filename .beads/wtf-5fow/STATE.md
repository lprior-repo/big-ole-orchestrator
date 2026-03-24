STATE 4

- Implementation landed for heartbeat-expiry recovery coverage.
- Recovery path now has observable FSM `current_state` in status snapshots.
- Metadata lookup bug in `wtf-storage` was fixed so recovery can find persisted instance metadata.
- Targeted checks and integration tests pass.
- Strict clippy remains blocked by broader pre-existing `wtf-storage` warnings outside this bead.
