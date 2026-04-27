//! BLACK-HAT adversarial tests for vo-worker lock manager.

use vo_worker::{LockId, LockMode, OwnerId, WaitEdge, WaitForGraph};

#[test]
fn bh_deadlock_graph_chaining_attack() {
    let mut graph = WaitForGraph::default();
    let n = 20;
    for i in 0..n {
        let o = OwnerId::new(format!("attacker-{i}"));
        let l = LockId::new(format!("lock-{i}"));
        graph.set_lock_holder(l.clone(), o.clone());
        let next_i = (i + 1) % n;
        graph.add_edge(WaitEdge {
            waiter: o,
            lock_id: LockId::new(format!("lock-{next_i}")),
            requested_mode: LockMode::Exclusive,
        });
    }
    let cycle = graph.detect_cycle();
    assert!(cycle.is_some(), "20-node cycle must be detected");
    assert_eq!(cycle.unwrap().len(), n);
}
