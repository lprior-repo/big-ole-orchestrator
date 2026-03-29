# Kani Justification: vo-3cv7

- **bead_id**: vo-3cv7
- **phase**: STATE-5.7
- **updated_at**: 2026-03-23T19:42:00Z

## Critical State Machines
The wait_for_signal mechanism is a state machine with two states:
1. WAITING (signal name registered in pending_signal_calls, waiting for RpcReplyPort)
2. DELIVERED (signal received, reply sent through port)

Transitions are triggered by handle_signal (deliver) and handle_inject_event_msg (replay). The Ractor actor model serializes all mutations — no concurrent access possible.

## Why Kani Adds Nothing
1. Ractor guarantees single-threaded message processing — no data races
2. The two-state machine has exactly one transition (WAITING → DELIVERED)
3. HashMap entry removal is atomic within the actor's serialized context
4. No unsafe code, no arithmetic, no index manipulation

## Known Defect (P2 — deferred)
Buffer-remove-before-publish in handle_wait_for_signal creates a tiny replay divergence window. This is not a Kani-detectable issue — it's a logic ordering defect that requires integration testing, not model checking.

## Conclusion
Kani would not find anything the actor model + tests don't already guarantee.
