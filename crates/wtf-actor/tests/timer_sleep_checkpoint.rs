//! Bug regression: TimerFired must create a checkpoint so ctx.sleep() can replay.
//!
//! BUG: apply_event ignores TimerScheduled (doesn't track op_id) and TimerFired
//! (doesn't create a checkpoint). After restart, ctx.sleep() finds no checkpoint
//! for the sleep operation and re-schedules a new timer — double-firing.
//!
//! Fix:
//!   - TimerScheduled: increment operation_counter, store timer_id → op_id mapping.
//!   - TimerFired: look up op_id from mapping, create checkpoint_map[op_id].

use wtf_actor::procedural::state::apply_event;
use wtf_actor::procedural::ProceduralActorState;
use wtf_common::WorkflowEvent;

/// After applying TimerScheduled + TimerFired, checkpoint_map must contain op 0.
#[test]
fn timer_fired_creates_checkpoint_so_sleep_can_replay() {
    let s0 = ProceduralActorState::new();

    let scheduled = WorkflowEvent::TimerScheduled {
        timer_id: "inst-01:t:0".into(),
        fire_at: chrono::Utc::now(),
    };
    let (s1, _) = apply_event(&s0, &scheduled, 1).expect("TimerScheduled");
    assert_eq!(
        s1.operation_counter, 1,
        "TimerScheduled must increment operation_counter"
    );

    let fired = WorkflowEvent::TimerFired {
        timer_id: "inst-01:t:0".into(),
    };
    let (s2, _) = apply_event(&s1, &fired, 2).expect("TimerFired");

    assert!(
        s2.checkpoint_map.contains_key(&0),
        "TimerFired must create checkpoint[0] so ctx.sleep() can replay without re-scheduling"
    );
}
