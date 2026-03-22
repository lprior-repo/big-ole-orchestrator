# Red Queen Report: Bead wtf-l0rm

## Adversarial Testing Analysis

### Attack Vectors Considered

1. **Timer created with past fire_at**
   - Scenario: Instance creates timer with `fire_at = now - 1 hour`
   - Attack: Timer is immediately due when created
   - Defense: `is_due(now)` check fires timer immediately in both sync and watch paths
   - Status: ✅ Defended

2. **Rapid create/delete cycles**
   - Scenario: Timer created, then deleted before watch processes it
   - Attack: Could cause duplicate processing or missed deletes
   - Defense: Delete operations are no-ops in `process_watch_entry`; get returns None if deleted
   - Status: ✅ Defended

3. **Watch stream disconnection**
   - Scenario: Watch stream closes unexpectedly
   - Attack: Could miss timer events during disconnection
   - Defense: Loop detects `None` from watch stream and exits; restart will trigger initial sync
   - Status: ✅ Defended

4. **Malformed timer records**
   - Scenario: KV contains invalid msgpack data
   - Attack: Could cause deserialization panic
   - Defense: `from_msgpack()` returns Error, logged and skipped
   - Status: ✅ Defended

5. **Timer fires but delete fails**
   - Scenario: JetStream append succeeds, KV delete fails
   - Attack: Timer re-fires on next sync
   - Defense: Instance actors handle duplicate `TimerFired` idempotently via applied_seq check
   - Status: ✅ Defended (by design, not prevented)

6. **Watch delivers out-of-order entries**
   - Scenario: Put arrives before Initial snapshot for same key
   - Attack: Timer might be processed twice
   - Defense: `fire_timer()` is idempotent; applied_seq check prevents duplicate processing
   - Status: ✅ Defended

7. **Very long timer list on initial sync**
   - Scenario: Thousands of timers exist at startup
   - Attack: Initial sync blocks watch stream establishment
   - Defense: `sync_and_fire_due` is async and cooperative; watch stream setup happens after
   - Status: ✅ Defended

8. **Timer scheduled for exactly now**
   - Scenario: `fire_at = Utc::now()`
   - Attack: Boundary condition - should fire immediately
   - Defense: `is_due()` uses `<=` comparison, so `fire_at <= now` is true
   - Status: ✅ Defended

### Edge Cases Tested

| Edge Case | Analysis | Status |
|-----------|----------|--------|
| Empty KV bucket | Initial sync finds no timers, watch waits | ✅ OK |
| Timer with 0 TTL | Immediate expiration | ✅ OK |
| Timer key collision | Key is timer_id, no collision possible | ✅ OK |
| Rapid successive watches | Stream handles rapid entries | ✅ OK |
| Shutdown during initial sync | Graceful exit after current timer | ✅ OK |

### Red Queen Decision

✅ **ALL DEFENSES HOLD** - No critical vulnerabilities found.

The implementation correctly handles all adversarial scenarios through:
- Idempotent event processing via applied_seq
- Graceful error handling with loop continuation
- Proper boundary conditions in is_due()
- No unsafe code paths

## Proceed to STATE 5.5 (Black Hat Review)
