//! Spawn supervisor process_cycle implementation.
//!
//! Handles Phase 1 (spawn new processes), Phase 2 (health check existing spawns),
//! and Phase 3 (respawn failed spawns).

use vo_types::InstanceId;

use super::pure::{calculate_backoff_delay, is_zombie_state, should_respawn};
use super::types::{SpawnPhase, SpawnRecord, SpawnSupervisorError};
use super::Actor;

impl Actor {
    /// Detects zombie processes among running spawns and reaps them.
    ///
    /// Scans all running-phase spawn records, checks if the underlying process
    /// is a zombie via `ProcessManager::is_zombie`, and transitions zombie
    /// records to `Terminated` phase. Also detects logical zombie state using
    /// `is_zombie_state` for failed records with high attempt counts.
    ///
    /// Returns the set of instance IDs that were reaped during this sweep.
    pub async fn zombie_detection(&self) -> Vec<InstanceId> {
        let mut reaped = Vec::new();

        // Phase A: Scan running processes for OS-level zombies.
        let running_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::Running, 100)
            .await;

        for record in running_records {
            let pid = match record.spawn_id {
                Some(ref sid) => {
                    // Try to parse the spawn ID as a PID.
                    sid.to_string().parse::<u32>().unwrap_or(0)
                }
                None => 0,
            };

            if pid == 0 {
                continue;
            }

            match self.process_manager.is_zombie(pid).await {
                Ok(true) => {
                    tracing::warn!(
                        instance_id = %record.instance_id,
                        pid,
                        "Zombie process detected, reaping"
                    );
                    self.metrics.zombies_detected.incr();

                    // Terminate the zombie process.
                    if let Err(e) = self.process_manager.terminate(pid).await {
                        tracing::error!(
                            instance_id = %record.instance_id,
                            pid,
                            error = %e,
                            "Failed to terminate zombie process"
                        );
                    }

                    // Transition to Terminated phase.
                    if let Err(e) = self
                        .storage
                        .transition_phase(&record.instance_id, SpawnPhase::Terminated)
                        .await
                    {
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %e,
                            "Failed to transition zombie to Terminated"
                        );
                    }

                    reaped.push(record.instance_id.clone());
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(
                        instance_id = %record.instance_id,
                        pid,
                        error = %e,
                        "Failed to check zombie status"
                    );
                }
            }
        }

        // Phase B: Scan failed records for logical zombie state (high attempts).
        let failed_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::Failed, 100)
            .await;

        for record in failed_records {
            if is_zombie_state(&record) {
                tracing::warn!(
                    instance_id = %record.instance_id,
                    attempts = record.spawn_attempts,
                    "Logical zombie detected (failed with high attempt count), reaping"
                );
                self.metrics.zombies_detected.incr();

                if let Err(e) = self
                    .storage
                    .transition_phase(&record.instance_id, SpawnPhase::Terminated)
                    .await
                {
                    tracing::error!(
                        instance_id = %record.instance_id,
                        error = %e,
                        "Failed to transition logical zombie to Terminated"
                    );
                }

                reaped.push(record.instance_id.clone());
            }
        }

        if !reaped.is_empty() {
            tracing::info!(count = reaped.len(), "Zombie detection sweep completed, reaped instances");
        }

        reaped
    }

    /// Processes one spawn cycle.
    ///
    /// Scans storage for spawns in spawn/health-check phases, spawns/health-checks them,
    /// and transitions to running phase when healthy.
    ///
    /// # Errors
    /// Returns an error if storage or process operations fail.
    pub async fn process_cycle(&self) -> Result<super::types::CycleResult, SpawnSupervisorError> {
        let mut spawns_processed = 0u32;
        let mut health_checks = 0u32;
        let mut errors = 0u32;
        let mut respawns = 0u32;

        // Phase 1: Spawn new processes
        let spawn_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::Spawn, 100)
            .await;

        for record in spawn_records {
            spawns_processed += 1;

            if record.spawn_attempts > self.max_spawn_attempts {
                self.metrics.spawns_failed.incr();
                errors += 1;
                tracing::warn!(
                    instance_id = %record.instance_id,
                    attempts = record.spawn_attempts,
                    max_attempts = self.max_spawn_attempts,
                    "Max spawn attempts exceeded"
                );
                continue;
            }

            let backoff_delay = Self::calc_backoff_delay(self, record.spawn_attempts);

            match self.spawn_process(&record).await {
                Ok(process_handle) => {
                    let mut new_record = record.transition_to_health_check();
                    new_record.last_error = None;

                    if let Err(e) = self.storage.save_spawn_record(&new_record).await {
                        self.metrics.dispatch_errors.incr();
                        errors += 1;
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %e,
                            "Failed to save spawn record"
                        );
                        continue;
                    }

                    match self
                        .perform_health_checks(&record.instance_id, &process_handle)
                        .await
                    {
                        Ok(()) => {
                            let mut running_record = new_record.transition_to_running();
                            running_record.spawn_id =
                                Some(vo_types::SpawnId::new(process_handle.pid.to_string()));

                            if let Err(e) = self.storage.save_spawn_record(&running_record).await {
                                self.metrics.dispatch_errors.incr();
                                errors += 1;
                                tracing::error!(
                                    instance_id = %record.instance_id,
                                    error = %e,
                                    "Failed to save running spawn record"
                                );
                                continue;
                            }

                            self.metrics.spawns_successful.incr();
                        }
                        Err(e) => {
                            self.metrics.health_checks_failed.incr();
                            errors += 1;
                            tracing::error!(
                                instance_id = %record.instance_id,
                                error = %e,
                                "Health check failed"
                            );

                            if record.spawn_attempts < self.max_spawn_attempts {
                                respawns += 1;
                                self.metrics.respawns.incr();

                                tracing::info!(
                                    instance_id = %record.instance_id,
                                    backoff_ms = backoff_delay.as_millis(),
                                    "Scheduling respawn with backoff"
                                );

                                tokio::time::sleep(backoff_delay).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    self.metrics.spawns_failed.incr();
                    errors += 1;

                    let mut updated_record = record.clone();
                    updated_record.last_error = Some(e.clone());

                    if let Err(save_err) = self.storage.save_spawn_record(&updated_record).await {
                        self.metrics.dispatch_errors.incr();
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %save_err,
                            "Failed to save error record"
                        );
                    }
                }
            }
        }

        // Phase 2: Health check existing spawns
        let health_check_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::HealthCheck, 100)
            .await;

        for record in health_check_records {
            spawns_processed += 1;
            health_checks += 1;

            match self
                .perform_health_checks(
                    &record.instance_id,
                    &super::traits::ProcessHandle {
                        pid: 0,
                        executable: record.executable.clone(),
                        args: record.args.clone(),
                    },
                )
                .await
            {
                Ok(()) => {
                    let running_record = record.transition_to_running();

                    if let Err(e) = self.storage.save_spawn_record(&running_record).await {
                        self.metrics.dispatch_errors.incr();
                        errors += 1;
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %e,
                            "Failed to save running spawn record"
                        );
                        continue;
                    }

                    self.metrics.spawns_successful.incr();
                }
                Err(e) => {
                    self.metrics.health_checks_failed.incr();
                    errors += 1;
                    tracing::error!(
                        instance_id = %record.instance_id,
                        error = %e,
                        "Health check failed"
                    );
                }
            }
        }

        // Phase 3: Respawn failed spawns
        let failed_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::Failed, 100)
            .await;

        for record in failed_records {
            spawns_processed += 1;

            if should_respawn(&record, self.max_spawn_attempts) {
                let backoff_delay = Self::calc_backoff_delay(self, record.spawn_attempts);
                tracing::info!(
                    instance_id = %record.instance_id,
                    backoff_ms = backoff_delay.as_millis(),
                    "Respawning failed spawn with backoff"
                );

                tokio::time::sleep(backoff_delay).await;

                let new_record = record.respawn(None);

                if let Err(e) = self.storage.save_spawn_record(&new_record).await {
                    self.metrics.dispatch_errors.incr();
                    errors += 1;
                    tracing::error!(
                        instance_id = %record.instance_id,
                        error = %e,
                        "Failed to save respawn record"
                    );
                    continue;
                }

                if let Err(e) = self
                    .work_queue
                    .enqueue_spawn(
                        record.instance_id.clone(),
                        record.executable.clone(),
                        record.args.clone(),
                    )
                    .await
                {
                    self.metrics.dispatch_errors.incr();
                    errors += 1;
                    tracing::error!(
                        instance_id = %record.instance_id,
                        error = %e,
                        "Failed to enqueue respawn"
                    );
                    continue;
                }

                respawns += 1;
                self.metrics.respawns.incr();
            }
        }

        Ok(super::types::CycleResult {
            spawns_processed,
            health_checks,
            errors,
            respawns,
        })
    }

    /// Spawns a process for a spawn record.
    async fn spawn_process(
        &self,
        record: &SpawnRecord,
    ) -> Result<super::traits::ProcessHandle, SpawnSupervisorError> {
        self.process_manager
            .spawn_process(&record.executable, &record.args)
            .await
    }

    /// Calculates backoff delay using exponential backoff.
    ///
    /// Formula: `initial_backoff * backoff_multiplier^(attempt - 1)`
    fn calc_backoff_delay(&self, attempt: u32) -> std::time::Duration {
        let delay_ms = calculate_backoff_delay(
            self.initial_backoff.as_millis() as u64,
            self.backoff_multiplier,
            attempt,
        );
        std::time::Duration::from_millis(delay_ms)
    }
}
