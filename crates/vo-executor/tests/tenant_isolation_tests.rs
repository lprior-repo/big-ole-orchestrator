//! ADR: Tenant Isolation and Quota Enforcement tests
//!
//! Tests for vel-knzg — defining per-tenant resource quotas and isolation boundaries.
//!
//! Contract (per ADR-006 + ADR-033 + vel-knzg):
//! 1. Per-tenant permit budgets — each tenant has a scoped semaphore
//! 2. Global cap — total across all tenants bounded
//! 3. Tenant A bulk work does not affect Tenant B critical latency
//! 4. Webhook flood isolated to offending tenant
//! 5. Over-quota tenant receives 429 (not 503)
//! 6. No tenant can consume all global resources
//! 7. ExactCritical jobs cannot be starved by Bulk tenants from other tenants

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Tenant identifier type.
///
/// Derived from API key, explicit request field, or issuer in CommandEnvelope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for TenantId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for TenantId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

/// Per-tenant quota configuration.
///
/// Defines the permit budget for a single tenant.
#[derive(Debug, Clone)]
pub struct TenantQuota {
    /// Maximum concurrent permits this tenant may hold.
    pub max_permits: usize,
    /// Maximum queued waiters for this tenant.
    pub max_queue: usize,
}

impl TenantQuota {
    pub fn new(max_permits: usize, max_queue: usize) -> Self {
        Self {
            max_permits,
            max_queue,
        }
    }

    pub fn default_tenant() -> Self {
        Self::new(10, 100)
    }
}

/// Tenant-scoped semaphore registry.
///
/// Replaces the global PrioritySemaphore with per-tenant semaphores
/// bounded by a global semaphore. Each tenant gets their own semaphore
/// with configurable permit budget. The global semaphore caps total usage.
#[derive(Debug)]
pub struct TenantSemaphoreRegistry {
    /// Global semaphore bounding total permits across all tenants.
    global: Arc<Semaphore>,
    /// Per-tenant semaphores.
    tenants: dashmap::DashMap<TenantId, Arc<Semaphore>>,
    /// Quota configuration per tenant.
    quotas: dashmap::DashMap<TenantId, TenantQuota>,
}

impl TenantSemaphoreRegistry {
    /// Create a new registry with a global permit cap.
    pub fn new(global_max_permits: usize) -> Self {
        Self {
            global: Arc::new(Semaphore::new(global_max_permits)),
            tenants: DashMap::new(),
            quotas: DashMap::new(),
        }
    }

    /// Register a tenant with a specific quota.
    pub fn register(&self, tenant: TenantId, quota: TenantQuota) {
        let permits = std::cmp::min(quota.max_permits, self.global_available());
        let sem = Arc::new(Semaphore::new(permits));
        self.tenants.insert(tenant.clone(), sem);
        self.quotas.insert(tenant, quota);
    }

    /// Get or register a tenant with default quota.
    pub fn get_or_default(&self, tenant: &TenantId) -> Arc<Semaphore> {
        if let Some(entry) = self.tenants.get(tenant) {
            return entry.value().clone();
        }
        let quota = TenantQuota::default_tenant();
        self.register(tenant.clone(), quota.clone());
        self.tenants.get(tenant).unwrap().value().clone()
    }

    /// Try to acquire a permit for a tenant.
    ///
    /// Returns None if the tenant's quota or the global cap is exhausted.
    pub fn try_acquire(&self, tenant: &TenantId) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let sem = self.get_or_default(tenant);
        let permit = sem.try_acquire_owned().ok()?;

        // Also try to acquire a global permit.
        // If global is exhausted, release the tenant permit and return None.
        match self.global.clone().try_acquire_owned() {
            Ok(_global) => Some(permit),
            Err(_) => {
                // Release tenant permit — global is full
                drop(permit);
                None
            }
        }
    }

    /// Check if a tenant has available permits.
    pub fn tenant_available(&self, tenant: &TenantId) -> usize {
        self.tenants
            .get(tenant)
            .map(|entry| entry.value().available_permits())
            .unwrap_or(0)
    }

    /// Check global available permits.
    pub fn global_available(&self) -> usize {
        self.global.available_permits()
    }

    /// Get total permits across all tenants.
    pub fn total_permits(&self) -> usize {
        self.tenants
            .iter()
            .map(|entry| entry.value().max_permits())
            .sum()
    }

    /// Get the number of registered tenants.
    pub fn tenant_count(&self) -> usize {
        self.tenants.len()
    }
}

use dashmap::DashMap;

// ===========================================================================
// TESTS
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_id_creation_and_display() {
        let id = TenantId::new("acme-corp");
        assert_eq!(id.as_str(), "acme-corp");
        assert_eq!(format!("{}", id), "acme-corp");
    }

    #[test]
    fn tenant_id_default_is_empty() {
        let id = TenantId::default();
        assert_eq!(id.as_str(), "");
    }

    #[test]
    fn tenant_quota_default() {
        let quota = TenantQuota::default_tenant();
        assert_eq!(quota.max_permits, 10);
        assert_eq!(quota.max_queue, 100);
    }

    #[test]
    fn registry_basic_acquire_releases() {
        let registry = TenantSemaphoreRegistry::new(50);
        let tenant = TenantId::new("tenant-a");
        registry.register(tenant.clone(), TenantQuota::new(10, 100));

        let permit = registry.try_acquire(&tenant);
        assert!(permit.is_some(), "Should acquire permit");

        // Acquire remaining permits for this tenant
        for _ in 1..10 {
            assert!(registry.try_acquire(&tenant).is_some());
        }

        // 11th should fail (quota=10)
        assert!(
            registry.try_acquire(&tenant).is_none(),
            "Tenant quota exhausted at 10 permits"
        );
    }

    #[test]
    fn tenant_a_permits_do_not_affect_tenant_b() {
        let registry = TenantSemaphoreRegistry::new(50);
        let tenant_a = TenantId::new("tenant-a");
        let tenant_b = TenantId::new("tenant-b");

        registry.register(
            tenant_a.clone(),
            TenantQuota::new(3, 50),
        );
        registry.register(
            tenant_b.clone(),
            TenantQuota::new(3, 50),
        );

        // Tenant A uses all 3 permits
        for _ in 0..3 {
            assert!(registry.try_acquire(&tenant_a).is_some());
        }
        assert!(registry.try_acquire(&tenant_a).is_none());

        // Tenant B should still have all 3 permits available
        let b1 = registry.try_acquire(&tenant_b);
        let b2 = registry.try_acquire(&tenant_b);
        let b3 = registry.try_acquire(&tenant_b);
        assert!(
            b1.is_some() && b2.is_some() && b3.is_some(),
            "Tenant B should have full quota available despite Tenant A being exhausted"
        );
    }

    #[test]
    fn global_cap_prevents_all_tenants_from_exhausting_resources() {
        let registry = TenantSemaphoreRegistry::new(10);
        let t1 = TenantId::new("t1");
        let t2 = TenantId::new("t2");
        let t3 = TenantId::new("t3");

        // Each tenant has a quota larger than the global cap
        registry.register(t1.clone(), TenantQuota::new(20, 200));
        registry.register(t2.clone(), TenantQuota::new(20, 200));
        registry.register(t3.clone(), TenantQuota::new(20, 200));

        // Collect permits across all tenants
        let mut all_permits = Vec::new();
        for _ in 0..10 {
            match registry.try_acquire(&t1) {
                Some(p) => all_permits.push(p),
                None => break,
            }
        }
        let t1_count = all_permits.len();

        // Try t2
        let mut t2_permits = Vec::new();
        for _ in 0..10 {
            match registry.try_acquire(&t2) {
                Some(p) => t2_permits.push(p),
                None => break,
            }
        }
        let t2_count = t2_permits.len();

        // Total permits must not exceed global cap
        let total = t1_count + t2_count;
        assert!(
            total <= 10,
            "Total permits ({}) must not exceed global cap (10), t1={t1_count}, t2={t2_count}",
            total
        );
    }

    #[test]
    fn over_quota_tenant_gets_none_not_panic() {
        let registry = TenantSemaphoreRegistry::new(50);
        let tenant = TenantId::new("overloaded");
        registry.register(tenant.clone(), TenantQuota::new(1, 10));

        // First acquire succeeds
        assert!(registry.try_acquire(&tenant).is_some());

        // Second should return None (not panic, not block)
        let result = registry.try_acquire(&tenant);
        assert!(
            result.is_none(),
            "Over-quota should return None, not panic"
        );
    }

    #[test]
    fn registry_tenant_isolation_with_global_cap_429_simulation() {
        let registry = TenantSemaphoreRegistry::new(20);
        let evil_tenant = TenantId::new("evil-tenant");
        let good_tenant = TenantId::new("good-tenant");

        // Evil tenant gets generous quota
        registry.register(
            evil_tenant.clone(),
            TenantQuota::new(15, 200),
        );
        // Good tenant gets generous quota too
        registry.register(
            good_tenant.clone(),
            TenantQuota::new(15, 200),
        );

        // Evil tenant exhausts global permits
        let mut evil_permits = Vec::new();
        for _ in 0..15 {
            match registry.try_acquire(&evil_tenant) {
                Some(p) => evil_permits.push(p),
                None => break,
            }
        }

        // Good tenant should still be able to acquire (some global permits remain)
        // Note: evil got 15 out of 20 global, so 5 remain for good tenant
        let good_can_acquire = registry.try_acquire(&good_tenant);
        assert!(
            good_can_acquire.is_some(),
            "Good tenant should get permits even when evil tenant is using most global capacity"
        );
    }

    #[test]
    fn registry_default_tenant_registration() {
        let registry = TenantSemaphoreRegistry::new(50);
        let tenant = TenantId::new("default-user");

        // get_or_default should auto-register with default quota
        let sem = registry.get_or_default(&tenant);

        let permit = sem.try_acquire_owned();
        assert!(permit.is_ok(), "Auto-registered tenant should work");
    }

    #[test]
    fn registry_tenant_count_tracks_registrations() {
        let registry = TenantSemaphoreRegistry::new(100);
        assert_eq!(registry.tenant_count(), 0);

        registry.register(
            TenantId::new("t1"),
            TenantQuota::new(10, 100),
        );
        assert_eq!(registry.tenant_count(), 1);

        registry.register(
            TenantId::new("t2"),
            TenantQuota::new(10, 100),
        );
        assert_eq!(registry.tenant_count(), 2);

        // Registering same tenant again overwrites
        registry.register(
            TenantId::new("t1"),
            TenantQuota::new(20, 200),
        );
        assert_eq!(registry.tenant_count(), 2);
    }

    #[tokio::test]
    async fn async_acquire_respects_tenant_isolation() {
        let registry = TenantSemaphoreRegistry::new(50);
        let tenant_a = TenantId::new("async-a");
        let tenant_b = TenantId::new("async-b");

        registry.register(tenant_a.clone(), TenantQuota::new(2, 50));
        registry.register(tenant_b.clone(), TenantQuota::new(2, 50));

        // Tenant A acquires 2 permits
        let p1 = registry.try_acquire(&tenant_a);
        let p2 = registry.try_acquire(&tenant_a);
        assert!(p1.is_some() && p2.is_some());

        // Tenant B should still have permits
        let pb = registry.try_acquire(&tenant_b);
        assert!(pb.is_some(), "Tenant B should have independent permits");

        drop(p1);
        drop(p2);
        drop(pb);
    }
}
