package lockmanager

import (
	"context"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
)

func TestLockModeOperations(t *testing.T) {
	// Shared can upgrade to Exclusive
	if Shared.canUpgradeTo(Exclusive) != true {
		t.Error("Shared should be able to upgrade to Exclusive")
	}
	// Exclusive cannot downgrade to Shared in upgrade context
	if Exclusive.canUpgradeTo(Shared) != false {
		t.Error("Exclusive cannot upgrade to Shared")
	}

	// Exclusive can downgrade to Shared
	if Exclusive.canDowngradeTo(Shared) != true {
		t.Error("Exclusive should be able to downgrade to Shared")
	}
	// Shared cannot upgrade to Exclusive in downgrade context
	if Shared.canDowngradeTo(Exclusive) != false {
		t.Error("Shared cannot downgrade to Exclusive")
	}
}

func TestLockEntryExpiry(t *testing.T) {
	owner := NewOwnerId("owner1")
	lockID := NewLockId("test")
	entry := NewLockEntry(lockID, owner, Exclusive, 1000)

	if entry.IsExpired() {
		t.Error("Entry should not be expired immediately")
	}

	remaining, ok := entry.RemainingTTL()
	if !ok {
		t.Error("RemainingTTL should return ok for non-expired entry")
	}
	if remaining <= 0 {
		t.Error("RemainingTTL should be positive")
	}

	expiredEntry := &LockEntry{
		LockId:     lockID,
		Owner:      owner,
		Mode:       Exclusive,
		Status:     Held,
		AcquiredAt: entry.AcquiredAt,
		ExpiresAt:  time.Now().UTC().Add(-time.Second),
		HoldToken:  entry.HoldToken,
	}

	if !expiredEntry.IsExpired() {
		t.Error("Expired entry should be expired")
	}

	_, ok = expiredEntry.RemainingTTL()
	if ok {
		t.Error("RemainingTTL should return not ok for expired entry")
	}
}

func TestWaitForGraphCycleDetection(t *testing.T) {
	graph := NewWaitForGraph()
	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")
	lock1 := NewLockId("lock1")
	lock2 := NewLockId("lock2")

	graph.SetLockHolder(lock1, owner1)
	graph.SetLockHolder(lock2, owner2)

	graph.AddEdge(WaitEdge{
		Waiter:        owner1,
		LockId:        lock2,
		RequestedMode: Exclusive,
	})

	graph.AddEdge(WaitEdge{
		Waiter:        owner2,
		LockId:        lock1,
		RequestedMode: Exclusive,
	})

	cycle := graph.DetectCycle()
	if len(cycle) == 0 {
		t.Error("Cycle should be detected in circular wait")
	}
}

func TestWaitForGraphNoCycle(t *testing.T) {
	graph := NewWaitForGraph()
	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")
	lock1 := NewLockId("lock1")
	lock2 := NewLockId("lock2")

	// Linear wait: owner1 waits for lock2 (held by owner2)
	// No cycle since owner2 doesn't wait for anything
	graph.SetLockHolder(lock2, owner2)
	graph.AddEdge(WaitEdge{
		Waiter:        owner1,
		LockId:        lock2,
		RequestedMode: Exclusive,
	})

	cycle := graph.DetectCycle()
	if len(cycle) > 0 {
		t.Errorf("No cycle should be detected in linear wait, got %v", cycle)
	}

	// Now add owner1 as lock1 holder and owner2 waiting for lock1 to create cycle
	graph.SetLockHolder(lock1, owner1)
	graph.AddEdge(WaitEdge{
		Waiter:        owner2,
		LockId:        lock1,
		RequestedMode: Exclusive,
	})

	cycle = graph.DetectCycle()
	if len(cycle) == 0 {
		t.Error("Cycle should be detected in circular wait")
	}
}

func TestLockManagerBasic(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// Acquire lock
	req := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp, err := manager.Acquire(ctx, req)
	if err != nil {
		t.Fatalf("Acquire failed: %v", err)
	}
	if !resp.Granted {
		t.Error("Lock should be granted")
	}
	if resp.HoldToken == nil || *resp.HoldToken == "" {
		t.Error("Hold token should be returned")
	}

	// Query lock
	query := LockQuery{LockId: &lockID}
	qresp, err := manager.Query(ctx, query)
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(qresp.Locks) != 1 {
		t.Errorf("Expected 1 lock, got %d", len(qresp.Locks))
	}

	// Release lock
	release := LockRelease{
		LockId:    lockID,
		Owner:     owner,
		HoldToken: *resp.HoldToken,
	}

	resp, err = manager.Release(ctx, release)
	if err != nil {
		t.Fatalf("Release failed: %v", err)
	}
	if !resp.Granted {
		t.Error("Release should succeed")
	}

	// Query again - should be empty
	qresp, err = manager.Query(ctx, query)
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(qresp.Locks) != 0 {
		t.Errorf("Expected 0 locks after release, got %d", len(qresp.Locks))
	}
}

func TestLockManagerDuplicateAcquire(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// First acquire
	req1 := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp1, err := manager.Acquire(ctx, req1)
	if err != nil || !resp1.Granted {
		t.Fatalf("First acquire failed: %v", err)
	}

	// Second acquire by same owner (re-acquire)
	req2 := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-2",
	}

	resp2, err := manager.Acquire(ctx, req2)
	if err != nil {
		t.Fatalf("Re-acquire failed: %v", err)
	}
	if !resp2.Granted {
		t.Error("Same owner re-acquire should succeed")
	}
}

func TestLockManagerIncompatibleAcquire(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")
	lockID := NewLockId("test-lock")

	// Owner1 acquires exclusive
	req1 := LockRequest{
		LockId:    lockID,
		Owner:     owner1,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	_, err := manager.Acquire(ctx, req1)
	if err != nil {
		t.Fatalf("First acquire failed: %v", err)
	}

	// Owner2 tries to acquire exclusive (should fail)
	req2 := LockRequest{
		LockId:    lockID,
		Owner:     owner2,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-2",
	}

	resp2, err := manager.Acquire(ctx, req2)
	if err != nil {
		t.Fatalf("Acquire failed: %v", err)
	}
	if resp2.Granted {
		t.Error("Exclusive lock should not be granted when held by another owner")
	}
}

func TestLockManagerSharedLocks(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")
	lockID := NewLockId("shared-lock")

	// Owner1 acquires shared
	req1 := LockRequest{
		LockId:    lockID,
		Owner:     owner1,
		Mode:      Shared,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp1, err := manager.Acquire(ctx, req1)
	if err != nil || !resp1.Granted {
		t.Fatalf("First acquire failed: %v", err)
	}

	// Owner2 acquires shared (should succeed)
	req2 := LockRequest{
		LockId:    lockID,
		Owner:     owner2,
		Mode:      Shared,
		TTL:       5000,
		RequestID: "req-2",
	}

	resp2, err := manager.Acquire(ctx, req2)
	if err != nil {
		t.Fatalf("Second acquire failed: %v", err)
	}
	if !resp2.Granted {
		t.Error("Shared lock should be granted when shared lock exists")
	}

	// Owner3 tries exclusive (should fail due to shared holders)
	owner3 := NewOwnerId("owner3")
	req3 := LockRequest{
		LockId:    lockID,
		Owner:     owner3,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-3",
	}

	resp3, err := manager.Acquire(ctx, req3)
	if err != nil {
		t.Fatalf("Third acquire failed: %v", err)
	}
	if resp3.Granted {
		t.Error("Exclusive lock should not be granted when shared holders exist")
	}
}

func TestLockManagerReleaseNonOwner(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")
	lockID := NewLockId("test-lock")

	// Owner1 acquires lock
	req1 := LockRequest{
		LockId:    lockID,
		Owner:     owner1,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp1, err := manager.Acquire(ctx, req1)
	if err != nil || !resp1.Granted {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Owner2 tries to release (should fail)
	release := LockRelease{
		LockId:    lockID,
		Owner:     owner2,
		HoldToken: *resp1.HoldToken,
	}

	_, err = manager.Release(ctx, release)
	if err != ErrNotOwner {
		t.Errorf("Expected ErrNotOwner, got %v", err)
	}
}

func TestLockManagerInvalidToken(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// Acquire lock
	req := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp, err := manager.Acquire(ctx, req)
	if err != nil || !resp.Granted {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Release with wrong token
	release := LockRelease{
		LockId:    lockID,
		Owner:     owner,
		HoldToken: "wrong-token",
	}

	_, err = manager.Release(ctx, release)
	if err != ErrInvalidToken {
		t.Errorf("Expected ErrInvalidToken, got %v", err)
	}
}

func TestLockManagerInvalidTTL(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	req := LockRequest{
		LockId:    NewLockId("test"),
		Owner:     NewOwnerId("owner1"),
		Mode:      Exclusive,
		TTL:       0, // Invalid TTL
		RequestID: "req-1",
	}

	_, err := manager.Acquire(ctx, req)
	if err != ErrInvalidTTL {
		t.Errorf("Expected ErrInvalidTTL for TTL=0, got %v", err)
	}
}

func TestLockManagerPromoteExclusiveToShared(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// Acquire exclusive
	req := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp, err := manager.Acquire(ctx, req)
	if err != nil || !resp.Granted {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Promote to shared
	promote := LockPromote{
		LockId:    lockID,
		Owner:     owner,
		HoldToken: *resp.HoldToken,
		NewMode:   Shared,
	}

	respPromote, err := manager.Promote(ctx, promote)
	if err != nil {
		t.Fatalf("Promote failed: %v", err)
	}
	if !respPromote.Granted {
		t.Error("Promote should succeed")
	}
	if respPromote.NewMode == nil || *respPromote.NewMode != Shared {
		t.Errorf("Expected mode to be Shared, got %v", respPromote.NewMode)
	}
}

func TestLockManagerUpgradeSharedToExclusive(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// Acquire shared
	req := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Shared,
		TTL:       5000,
		RequestID: "req-1",
	}

	resp, err := manager.Acquire(ctx, req)
	if err != nil || !resp.Granted {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Upgrade to exclusive (should succeed since no other holders)
	promote := LockPromote{
		LockId:    lockID,
		Owner:     owner,
		HoldToken: *resp.HoldToken,
		NewMode:   Exclusive,
	}

	respPromote, err := manager.Promote(ctx, promote)
	if err != nil {
		t.Fatalf("Promote failed: %v", err)
	}
	if !respPromote.Granted {
		t.Error("Upgrade should succeed when no other holders")
	}
}

func TestLockManagerRenew(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner := NewOwnerId("owner1")
	lockID := NewLockId("test-lock")

	// Acquire lock with short TTL
	req := LockRequest{
		LockId:    lockID,
		Owner:     owner,
		Mode:      Exclusive,
		TTL:       100,
		RequestID: "req-1",
	}

	resp, err := manager.Acquire(ctx, req)
	if err != nil || !resp.Granted {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Wait for expiry
	time.Sleep(150 * time.Millisecond)

	// Renew lock
	respRenew, err := manager.Renew(ctx, lockID, owner, *resp.HoldToken, 5000)
	if err != nil {
		t.Fatalf("Renew failed: %v", err)
	}
	if !respRenew.Granted {
		t.Error("Renew should succeed")
	}

	// Check lock is not expired
	entry, ok := manager.GetLocks()[lockID]
	if !ok {
		t.Fatal("Lock should still exist after renew")
	}
	if entry.IsExpired() {
		t.Error("Lock should not be expired after renew")
	}
}

func TestLockManagerQueryByOwner(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)

	manager := NewLockManager(logger)
	ctx := context.Background()

	owner1 := NewOwnerId("owner1")
	owner2 := NewOwnerId("owner2")

	// Owner1 acquires two locks
	lock1 := NewLockId("lock1")
	lock2 := NewLockId("lock2")

	for _, lockID := range []LockId{lock1, lock2} {
		req := LockRequest{
			LockId:    lockID,
			Owner:     owner1,
			Mode:      Exclusive,
			TTL:       5000,
			RequestID: "req-1",
		}
		_, err := manager.Acquire(ctx, req)
		if err != nil {
			t.Fatalf("Acquire failed: %v", err)
		}
	}

	// Owner2 acquires one lock
	lock3 := NewLockId("lock3")
	req := LockRequest{
		LockId:    lock3,
		Owner:     owner2,
		Mode:      Exclusive,
		TTL:       5000,
		RequestID: "req-2",
	}
	_, err := manager.Acquire(ctx, req)
	if err != nil {
		t.Fatalf("Acquire failed: %v", err)
	}

	// Query by owner1 - should return 2 locks
	query := LockQuery{Owner: &owner1}
	qresp, err := manager.Query(ctx, query)
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(qresp.Locks) != 2 {
		t.Errorf("Expected 2 locks for owner1, got %d", len(qresp.Locks))
	}

	// Query by owner2 - should return 1 lock
	query = LockQuery{Owner: &owner2}
	qresp, err = manager.Query(ctx, query)
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(qresp.Locks) != 1 {
		t.Errorf("Expected 1 lock for owner2, got %d", len(qresp.Locks))
	}
}
