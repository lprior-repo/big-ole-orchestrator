//! Red Queen coevolutionary adversarial tests for lock contention in vo-worker.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use vo_worker::{
    LockError, LockId, LockManager, LockMode, LockPromote, LockPromoteResponse, LockQuery,
    LockQueryResponse, LockRelease, LockRequest, LockResponse, OwnerId, WaitEdge, WaitForGraph,
};
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
fn guard() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
type Locks = HashMap<(String, String), (String, LockMode)>;

struct CLM {
    locks: Mutex<Locks>,
    contention: AtomicUsize,
    cap: usize,
}
impl CLM {
    fn new(cap: usize) -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
            contention: AtomicUsize::new(0),
            cap,
        }
    }
    fn ct(&self) -> usize {
        self.contention.load(Ordering::SeqCst)
    }
    fn shr(&self, k: &str, l: &Locks) -> usize {
        l.iter()
            .filter(|((lk, _), (_, m))| lk == k && *m == LockMode::Shared)
            .count()
    }
    fn excl(&self, k: &str, l: &Locks) -> bool {
        l.iter()
            .any(|((lk, _), (_, m))| lk == k && *m == LockMode::Exclusive)
    }
    fn ok_resp(r: LockRequest, t: String) -> LockResponse {
        LockResponse {
            request_id: r.request_id,
            lock_id: r.lock_id,
            owner: r.owner,
            granted: true,
            hold_token: Some(t),
            expires_at: None,
            error: None,
        }
    }
    fn fail_resp(r: LockRequest) -> LockResponse {
        LockResponse {
            request_id: r.request_id,
            lock_id: r.lock_id,
            owner: r.owner,
            granted: false,
            hold_token: None,
            expires_at: None,
            error: Some("contention".into()),
        }
    }
}

#[async_trait]
impl LockManager for CLM {
    async fn acquire(&self, r: LockRequest) -> LockResponse {
        let k = r.lock_id.as_str().to_string();
        let o = r.owner.to_string();
        let mut l = self.locks.lock().unwrap();
        if l.contains_key(&(k.clone(), o.clone())) {
            return Self::ok_resp(r, format!("t-{}", o));
        }
        match r.mode {
            LockMode::Exclusive if self.excl(&k, &l) || self.shr(&k, &l) > 0 => {
                self.contention.fetch_add(1, Ordering::SeqCst);
                Self::fail_resp(r)
            }
            LockMode::Shared if self.excl(&k, &l) || self.shr(&k, &l) >= self.cap => {
                self.contention.fetch_add(1, Ordering::SeqCst);
                Self::fail_resp(r)
            }
            LockMode::Exclusive => {
                let t = format!("t-{}-{}", o, k);
                l.insert((k, o), (t.clone(), LockMode::Exclusive));
                Self::ok_resp(r, t)
            }
            LockMode::Shared => {
                let t = format!("s-{}-{}", o, k);
                l.insert((k, o), (t.clone(), LockMode::Shared));
                Self::ok_resp(r, t)
            }
        }
    }
    async fn release(&self, r: LockRelease) -> Result<(), LockError> {
        if self
            .locks
            .lock()
            .unwrap()
            .remove(&(r.lock_id.as_str().to_string(), r.owner.to_string()))
            .is_none()
        {
            Err(LockError::NotFound(r.lock_id))
        } else {
            Ok(())
        }
    }
    async fn query(&self, _: LockQuery) -> LockQueryResponse {
        LockQueryResponse { locks: vec![] }
    }
    async fn promote(&self, p: LockPromote) -> LockPromoteResponse {
        let k = p.lock_id.as_str().to_string();
        let o = p.owner.to_string();
        let mut l = self.locks.lock().unwrap();
        if self.shr(&k, &l) > 1 {
            return LockPromoteResponse {
                request_id: String::new(),
                lock_id: p.lock_id,
                granted: false,
                new_mode: None,
                error: Some("multi".into()),
            };
        }
        if let Some((t, m)) = l.get_mut(&(k.clone(), o)) {
            *m = LockMode::Exclusive;
            return LockPromoteResponse {
                request_id: String::new(),
                lock_id: p.lock_id,
                granted: true,
                new_mode: Some(LockMode::Exclusive),
                error: None,
            };
        }
        LockPromoteResponse {
            request_id: String::new(),
            lock_id: p.lock_id,
            granted: false,
            new_mode: None,
            error: Some("no".into()),
        }
    }
    async fn demote(&self, id: LockId, _: OwnerId, _: String) -> Result<LockMode, LockError> {
        Err(LockError::NotFound(id))
    }
    async fn extend_ttl(
        &self,
        id: LockId,
        _: OwnerId,
        _: String,
        _: u64,
    ) -> Result<chrono::DateTime<chrono::Utc>, LockError> {
        Err(LockError::NotFound(id))
    }
    async fn is_locked(&self, id: &LockId) -> bool {
        let k = id.as_str();
        self.locks.lock().unwrap().keys().any(|(lk, _)| lk == k)
    }
    async fn get_holder(&self, id: &LockId) -> Option<(OwnerId, LockMode)> {
        self.locks
            .lock()
            .unwrap()
            .iter()
            .find(|((lk, _), _)| lk == id.as_str())
            .map(|((_, o), (_, m))| (OwnerId::new(o.clone()), *m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(lock: &str, owner: &str, mode: LockMode, rid: &str) -> LockRequest {
        LockRequest {
            lock_id: LockId::new(lock),
            owner: OwnerId::new(owner.into()),
            mode,
            ttl_ms: 1000,
            request_id: rid.into(),
        }
    }

    #[tokio::test]
    async fn exclusive_owner_blocks_all_competitors() {
        let _g = guard();
        let m = Arc::new(CLM::new(4));
        assert!(
            m.acquire(req("rx", "dom", LockMode::Exclusive, "r1"))
                .await
                .granted
        );
        for i in 0..5 {
            let m = m.clone();
            assert!(
                !m.acquire(req(
                    "rx",
                    &format!("rv-{}", i),
                    LockMode::Shared,
                    &format!("s{}", i)
                ))
                .await
                .granted,
                "rv-{}",
                i
            );
        }
        assert_eq!(m.ct(), 5);
    }

    #[tokio::test]
    async fn shared_lock_cap_triggers_contention() {
        let _g = guard();
        let m = Arc::new(CLM::new(2));
        for i in 0..2 {
            assert!(
                m.acquire(req(
                    "c",
                    &format!("r{}", i),
                    LockMode::Shared,
                    &format!("s{}", i)
                ))
                .await
                .granted
            );
        }
        assert!(
            !m.acquire(req("c", "ov", LockMode::Shared, "o"))
                .await
                .granted
        );
        assert_eq!(m.ct(), 1);
    }

    #[tokio::test]
    async fn promote_fails_under_multi_shared_contention() {
        let _g = guard();
        let m = Arc::new(CLM::new(4));
        for i in 0..3 {
            assert!(
                m.acquire(req(
                    "p",
                    &format!("h{}", i),
                    LockMode::Shared,
                    &format!("s{}", i)
                ))
                .await
                .granted
            );
        }
        assert!(
            !m.promote(LockPromote {
                lock_id: LockId::new("p"),
                owner: OwnerId::new("h0".into()),
                hold_token: "t".into(),
                new_mode: LockMode::Exclusive
            })
            .await
            .granted
        );
    }

    #[tokio::test]
    async fn wait_for_graph_detects_3way_cycle() {
        let _g = guard();
        let mut g = WaitForGraph::default();
        let a = OwnerId::new("a".into());
        let b = OwnerId::new("b".into());
        let c = OwnerId::new("c".into());
        let lx = LockId::new("lx");
        let ly = LockId::new("ly");
        let lz = LockId::new("lz");
        g.set_lock_holder(lx.clone(), a.clone());
        g.set_lock_holder(ly.clone(), b.clone());
        g.set_lock_holder(lz.clone(), c.clone());
        g.add_edge(WaitEdge {
            waiter: a,
            lock_id: ly,
            requested_mode: LockMode::Exclusive,
        });
        g.add_edge(WaitEdge {
            waiter: b,
            lock_id: lz,
            requested_mode: LockMode::Exclusive,
        });
        g.add_edge(WaitEdge {
            waiter: c,
            lock_id: lx,
            requested_mode: LockMode::Exclusive,
        });
        let cy = g.detect_cycle();
        assert!(cy.is_some());
        assert_eq!(cy.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn sequential_access_zero_contention() {
        let _g = guard();
        let m = Arc::new(CLM::new(1));
        let mut ok = 0usize;
        for o in ["a", "b", "c", "d"] {
            let r = m
                .acquire(req("h", o, LockMode::Exclusive, &format!("e{}", o)))
                .await;
            if r.granted {
                ok += 1;
                let _ = m
                    .release(LockRelease {
                        lock_id: LockId::new("h"),
                        owner: OwnerId::new(o.into()),
                        hold_token: r.hold_token.unwrap(),
                    })
                    .await;
            }
        }
        assert_eq!(ok, 4);
        assert_eq!(m.ct(), 0);
    }
}
