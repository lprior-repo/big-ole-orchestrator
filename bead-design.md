## ARCH-DRIFT Audit: vo-actor message routing state machine

### FINDING: Missing 'draining' state

**Task description claims:** Route states = active, inactive, draining
**Actual implementation:** Only active/inactive (boolean `is_active` flag)

### Code Analysis

**routing.rs (LineageRouter):**
- No routing state machine with active/inactive/draining states
- Uses LineageQuery to ResolvedRoute resolution
- LineageStatus = Active | Tombstoned only

**message_router.rs (MessageRouter):**
- RoutingDestination.is_active: bool - binary state only
- activate() / deactivate() - simple boolean toggle, no transition validation
- No draining state anywhere in codebase

**lifecycle.rs (ActorLifecycleState):**
- States: Pending, Running, Stopping, Stopped, Failed
- Proper compute_next_state() transition validation
- But this is actor lifecycle, NOT routing state machine

### Drift Summary

| State     | Task Claims | Code Has                    |
|-----------|-------------|-----------------------------|
| active    | YES         | YES (is_active=true)        |
| inactive  | YES         | YES (is_active=false)       |
| draining  | YES         | NO - DOES NOT EXIST         |

### No Transition Validation in Routing

Unlike lifecycle.rs which has compute_next_state() and is_valid_transition(), the routing code has no state machine transition validation. deactivate_channel() can be called on any channel regardless of current state.

### Action Items

1. If draining state is needed: implement 3-state RouteState enum
2. If draining is not needed: update task description to match implementation
3. Consider adding transition validation like lifecycle.rs has

This is an ARCHITECTURAL DRIFT - specification and implementation diverged.