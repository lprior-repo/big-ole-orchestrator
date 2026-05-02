//! Task Subscriber with Atomic Claim Semantics
//!
//! Provides atomic task claiming to prevent race conditions when multiple workers
//! attempt to claim the same task. Exactly one worker will succeed; others receive
//! AlreadyClaimed.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimResult {
    Claimed,
    AlreadyClaimed,
}

#[derive(Debug, Clone)]
struct TaskClaim {
    task_id: String,
    owner_id: String,
}

pub struct TaskSubscriber {
    claims: Arc<RwLock<HashMap<String, TaskClaim>>>,
}

impl TaskSubscriber {
    pub fn new() -> Self {
        Self {
            claims: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn claim(&self, task_id: &str, worker_id: &str) -> ClaimResult {
        let mut claims = self.claims.write().await;
        match claims.get(task_id) {
            Some(existing) if existing.owner_id == worker_id => ClaimResult::Claimed,
            Some(_) => ClaimResult::AlreadyClaimed,
            None => {
                claims.insert(
                    task_id.to_string(),
                    TaskClaim {
                        task_id: task_id.to_string(),
                        owner_id: worker_id.to_string(),
                    },
                );
                ClaimResult::Claimed
            }
        }
    }

    pub async fn release(&self, task_id: &str, worker_id: &str) -> bool {
        let mut claims = self.claims.write().await;
        if let Some(existing) = claims.get(task_id) {
            if existing.owner_id == worker_id {
                claims.remove(task_id);
                return true;
            }
        }
        false
    }

    #[cfg(test)]
    pub async fn get_claim(&self, task_id: &str) -> Option<String> {
        let claims = self.claims.read().await;
        claims.get(task_id).map(|c| c.owner_id.clone())
    }
}

impl Default for TaskSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::task::JoinHandle;

    fn spawn_worker(
        subscriber: Arc<TaskSubscriber>,
        task_id: String,
        worker_id: String,
    ) -> JoinHandle<ClaimResult> {
        tokio::spawn(async move { subscriber.claim(&task_id, &worker_id).await })
    }

    #[tokio::test]
    async fn test_concurrent_claim_race_condition() {
        let subscriber = Arc::new(TaskSubscriber::new());
        let task_id = "task-123".to_string();

        let worker1 = spawn_worker(subscriber.clone(), task_id.clone(), "worker-1".to_string());
        let worker2 = spawn_worker(subscriber.clone(), task_id.clone(), "worker-2".to_string());

        let (result1, result2) = tokio::join!(worker1, worker2);

        let results = [result1.unwrap(), result2.unwrap()];
        let claimed_count = results.iter().filter(|r| *r == ClaimResult::Claimed).count();
        let already_claimed_count = results
            .iter()
            .filter(|r| *r == ClaimResult::AlreadyClaimed)
            .count();

        assert_eq!(
            claimed_count, 1,
            "Exactly one worker should get Claimed, got {}",
            claimed_count
        );
        assert_eq!(
            already_claimed_count, 1,
            "Exactly one worker should get AlreadyClaimed, got {}",
            already_claimed_count
        );
    }

    #[tokio::test]
    async fn test_same_worker_claiming_twice_gets_claimed() {
        let subscriber = Arc::new(TaskSubscriber::new());
        let task_id = "task-456".to_string();
        let worker_id = "worker-1".to_string();

        let result1 = subscriber.claim(&task_id, &worker_id).await;
        let result2 = subscriber.claim(&task_id, &worker_id).await;

        assert_eq!(result1, ClaimResult::Claimed);
        assert_eq!(result2, ClaimResult::Claimed);
    }

    #[tokio::test]
    async fn test_release_allows_reclaim() {
        let subscriber = Arc::new(TaskSubscriber::new());
        let task_id = "task-789".to_string();

        let result1 = subscriber.claim(&task_id, "worker-1").await;
        assert_eq!(result1, ClaimResult::Claimed);

        let released = subscriber.release(&task_id, "worker-1").await;
        assert!(released);

        let result2 = subscriber.claim(&task_id, "worker-2").await;
        assert_eq!(result2, ClaimResult::Claimed);
    }

    #[tokio::test]
    async fn test_release_wrong_owner_fails() {
        let subscriber = Arc::new(TaskSubscriber::new());
        let task_id = "task-999".to_string();

        subscriber.claim(&task_id, "worker-1").await;

        let released = subscriber.release(&task_id, "worker-2").await;
        assert!(!released);

        let owner = subscriber.get_claim(&task_id).await;
        assert_eq!(owner, Some("worker-1".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_tasks_independent() {
        let subscriber = Arc::new(TaskSubscriber::new());

        let t1_w1 = spawn_worker(subscriber.clone(), "task-1".to_string(), "worker-1".to_string());
        let t2_w1 = spawn_worker(subscriber.clone(), "task-2".to_string(), "worker-1".to_string());
        let t1_w2 = spawn_worker(subscriber.clone(), "task-1".to_string(), "worker-2".to_string());
        let t2_w2 = spawn_worker(subscriber.clone(), "task-2".to_string(), "worker-2".to_string());

        let (r1, r2, r3, r4) = tokio::join!(t1_w1, t2_w1, t1_w2, t2_w2);
        let results = [r1.unwrap(), r2.unwrap(), r3.unwrap(), r4.unwrap()];

        assert_eq!(results[0], ClaimResult::Claimed);
        assert_eq!(results[1], ClaimResult::Claimed);
        assert_eq!(results[2], ClaimResult::AlreadyClaimed);
        assert_eq!(results[3], ClaimResult::AlreadyClaimed);
    }

    #[tokio::test]
    async fn test_many_workers_race_for_same_task() {
        let subscriber = Arc::new(TaskSubscriber::new());
        let task_id = "task-many".to_string();
        let worker_count = 10;

        let mut handles: Vec<JoinHandle<ClaimResult>> = Vec::new();
        for i in 0..worker_count {
            let sub = subscriber.clone();
            let tid = task_id.clone();
            handles.push(tokio::spawn(async move {
                sub.claim(&tid, &format!("worker-{}", i)).await
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        let claimed_count = results.iter().filter(|r| *r == ClaimResult::Claimed).count();
        let already_claimed_count = results
            .iter()
            .filter(|r| *r == ClaimResult::AlreadyClaimed)
            .count();

        assert_eq!(
            claimed_count, 1,
            "Exactly one worker should get Claimed out of {}, got {}",
            worker_count, claimed_count
        );
        assert_eq!(
            already_claimed_count,
            (worker_count - 1) as usize,
            "All other workers should get AlreadyClaimed"
        );
    }
}
