use crate::calculations::proportional_rig_quota;
use crate::data::{BeadId, BeadJson, FleetEntry, PolecatName, PolecatStatus, Rig, RigKind};
use std::collections::{HashMap, HashSet};

const MAX_PER_RIG_QUOTA: usize = 12;

#[derive(Debug)]
pub struct RigCycle {
    pub rig: &'static Rig,
    pub fleet: Vec<FleetEntry>,
    pub statuses: Vec<PolecatStatus>,
}

#[derive(Debug)]
pub struct ReadyPool {
    rig: &'static Rig,
    beads: Vec<BeadJson>,
    claimed: HashSet<String>,
}

impl ReadyPool {
    pub fn new(rig: &'static Rig, beads: Vec<BeadJson>) -> Self {
        Self {
            rig,
            beads,
            claimed: HashSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct SelectedBead {
    pub rig: &'static Rig,
    pub bead: BeadId,
}

fn unassigned_ready_count(pool: &ReadyPool) -> usize {
    pool.beads
        .iter()
        .filter(|bead| bead.assignee.is_none() && !pool.claimed.contains(&bead.id))
        .count()
}

fn take_bead_from_pool(pool: &mut ReadyPool) -> Option<SelectedBead> {
    let bead = pool
        .beads
        .iter()
        .find(|bead| bead.assignee.is_none() && !pool.claimed.contains(&bead.id))
        .map(|bead| BeadId(bead.id.clone()))?;

    pool.claimed.insert(bead.as_str().to_string());
    Some(SelectedBead {
        rig: pool.rig,
        bead,
    })
}

fn source_polecat_can_take(cycles: &[RigCycle], source: &Rig, name: &PolecatName) -> bool {
    cycles
        .iter()
        .find(|cycle| cycle.rig.kind == source.kind)
        .and_then(|cycle| {
            cycle
                .fleet
                .iter()
                .position(|entry| entry.name.as_str() == name.as_str())
                .and_then(|index| cycle.statuses.get(index).copied())
        })
        .is_some_and(|status| matches!(status, PolecatStatus::Dead | PolecatStatus::Idle))
}

pub fn select_bead_for_polecat(
    target_rig: &Rig,
    name: &PolecatName,
    pools: &mut [ReadyPool],
    cycles: &[RigCycle],
) -> Option<SelectedBead> {
    let local_index = pools
        .iter()
        .position(|pool| pool.rig.kind == target_rig.kind);
    if let Some(index) = local_index
        && let Some(selected) = take_bead_from_pool(&mut pools[index])
    {
        return Some(selected);
    }

    let borrow_index = pools
        .iter()
        .enumerate()
        .filter(|(_, pool)| pool.rig.kind != target_rig.kind)
        .filter(|(_, pool)| source_polecat_can_take(cycles, pool.rig, name))
        .max_by_key(|(_, pool)| unassigned_ready_count(pool))
        .map(|(index, _)| index);

    borrow_index.and_then(|index| take_bead_from_pool(&mut pools[index]))
}

pub fn quotas_for_pools(pools: &[ReadyPool], remaining: usize) -> HashMap<RigKind, usize> {
    let total_ready = pools.iter().map(unassigned_ready_count).sum::<usize>();
    pools
        .iter()
        .map(|pool| {
            (
                pool.rig.kind,
                proportional_rig_quota(
                    unassigned_ready_count(pool),
                    total_ready,
                    remaining,
                    MAX_PER_RIG_QUOTA,
                ),
            )
        })
        .collect()
}
