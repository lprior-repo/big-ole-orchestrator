// Package lockmanager implements a distributed lock manager with TTL,
// deadlock detection, and crash-safe recovery.
package lockmanager

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/oklog/ulid/v2"
	"github.com/sirupsen/logrus"
)

var (
	ErrNotFound             = errors.New("lock not found")
	ErrNotOwner             = errors.New("not lock owner")
	ErrInvalidToken         = errors.New("invalid hold token")
	ErrDeadlockDetected     = errors.New("deadlock detected")
	ErrIncompatibleMode     = errors.New("lock held in incompatible mode")
	ErrInvalidTTL           = errors.New("TTL must be positive")
	ErrTimeout              = errors.New("timeout waiting for lock")
	ErrUpgradeWouldDeadlock = errors.New("already holds lock in shared mode, cannot upgrade")
)

type LockMode int

const (
	Shared LockMode = iota
	Exclusive
)

func (m LockMode) canUpgradeTo(other LockMode) bool {
	return m == Shared && other == Exclusive
}

func (m LockMode) canDowngradeTo(other LockMode) bool {
	return m == Exclusive && other == Shared
}

type LockStatus int

const (
	Held LockStatus = iota
	Pending
	Expired
)

type LockId string

func NewLockId(id string) LockId {
	return LockId(id)
}

func (l LockId) String() string {
	return string(l)
}

type OwnerId string

func NewOwnerId(id string) OwnerId {
	return OwnerId(id)
}

func (o OwnerId) String() string {
	return string(o)
}

type LockEntry struct {
	LockId     LockId     `json:"lock_id"`
	Owner      OwnerId    `json:"owner"`
	Mode       LockMode   `json:"mode"`
	Status     LockStatus `json:"status"`
	AcquiredAt time.Time  `json:"acquired_at"`
	ExpiresAt  time.Time  `json:"expires_at"`
	HoldToken  string     `json:"hold_token"`
}

func NewLockEntry(lockId LockId, owner OwnerId, mode LockMode, ttlMs uint64) *LockEntry {
	now := time.Now().UTC()
	expiresAt := now.Add(time.Duration(ttlMs) * time.Millisecond)
	return &LockEntry{
		LockId:     lockId,
		Owner:      owner,
		Mode:       mode,
		Status:     Held,
		AcquiredAt: now,
		ExpiresAt:  expiresAt,
		HoldToken:  ulid.Make().String(),
	}
}

func (e *LockEntry) IsExpired() bool {
	return time.Now().UTC().After(e.ExpiresAt)
}

func (e *LockEntry) RemainingTTL() (time.Duration, bool) {
	remaining := time.Until(e.ExpiresAt)
	if remaining <= 0 {
		return 0, false
	}
	return remaining, true
}

type LockRequest struct {
	LockId    LockId   `json:"lock_id"`
	Owner     OwnerId  `json:"owner"`
	Mode      LockMode `json:"mode"`
	TTL       uint64   `json:"ttl"`
	RequestID string   `json:"request_id"`
}

type LockResponse struct {
	RequestID string     `json:"request_id"`
	LockId    LockId     `json:"lock_id"`
	Owner     OwnerId    `json:"owner"`
	Granted   bool       `json:"granted"`
	Status    LockStatus `json:"status,omitempty"`
	HoldToken *string    `json:"hold_token,omitempty"`
	ExpiresAt *time.Time `json:"expires_at,omitempty"`
	Error     *string    `json:"error,omitempty"`
}

type LockRelease struct {
	LockId    LockId  `json:"lock_id"`
	Owner     OwnerId `json:"owner"`
	HoldToken string  `json:"hold_token"`
}

type LockQuery struct {
	LockId *LockId  `json:"lock_id,omitempty"`
	Owner  *OwnerId `json:"owner,omitempty"`
}

type LockQueryResponse struct {
	Locks []*LockEntry `json:"locks"`
}

type LockPromote struct {
	LockId    LockId   `json:"lock_id"`
	Owner     OwnerId  `json:"owner"`
	HoldToken string   `json:"hold_token"`
	NewMode   LockMode `json:"new_mode"`
}

type LockPromoteResponse struct {
	RequestID string    `json:"request_id"`
	LockId    LockId    `json:"lock_id"`
	Granted   bool      `json:"granted"`
	NewMode   *LockMode `json:"new_mode,omitempty"`
	Error     *string   `json:"error,omitempty"`
}

type WaitEdge struct {
	Waiter        OwnerId  `json:"waiter"`
	LockId        LockId   `json:"lock_id"`
	RequestedMode LockMode `json:"requested_mode"`
}

type WaitForGraph struct {
	mu          sync.RWMutex
	waitEdges   []WaitEdge
	lockHolders map[LockId]OwnerId
}

func NewWaitForGraph() *WaitForGraph {
	return &WaitForGraph{
		waitEdges:   make([]WaitEdge, 0),
		lockHolders: make(map[LockId]OwnerId),
	}
}

func (g *WaitForGraph) AddEdge(edge WaitEdge) {
	g.mu.Lock()
	defer g.mu.Unlock()

	// Remove existing edge for same waiter/lock combination
	remaining := make([]WaitEdge, 0)
	for _, e := range g.waitEdges {
		if !(e.Waiter == edge.Waiter && e.LockId == edge.LockId) {
			remaining = append(remaining, e)
		}
	}
	g.waitEdges = remaining
	g.waitEdges = append(g.waitEdges, edge)
}

func (g *WaitForGraph) SetLockHolder(lockId LockId, owner OwnerId) {
	g.mu.Lock()
	defer g.mu.Unlock()
	g.lockHolders[lockId] = owner
}

func (g *WaitForGraph) RemoveEdgesForOwner(owner *OwnerId) {
	g.mu.Lock()
	defer g.mu.Unlock()

	remaining := make([]WaitEdge, 0)
	for _, e := range g.waitEdges {
		if owner == nil || &e.Waiter != owner {
			remaining = append(remaining, e)
		}
	}
	g.waitEdges = remaining
}

func (g *WaitForGraph) RemoveEdgesForLock(lockId *LockId) {
	g.mu.Lock()
	defer g.mu.Unlock()

	remaining := make([]WaitEdge, 0)
	for _, e := range g.waitEdges {
		if lockId == nil || &e.LockId != lockId {
			remaining = append(remaining, e)
		}
	}
	g.waitEdges = remaining
}

func (g *WaitForGraph) GetWaiters(lockId *LockId) []OwnerId {
	g.mu.RLock()
	defer g.mu.RUnlock()

	var waiters []OwnerId
	for _, e := range g.waitEdges {
		if lockId == nil || &e.LockId == lockId {
			waiters = append(waiters, e.Waiter)
		}
	}
	return waiters
}

func (g *WaitForGraph) DetectCycle() []OwnerId {
	g.mu.RLock()
	defer g.mu.RUnlock()

	inDegree := make(map[OwnerId]int)
	adjacency := make(map[OwnerId][]OwnerId)

	for _, edge := range g.waitEdges {
		if holder, ok := g.lockHolders[edge.LockId]; ok {
			if holder == edge.Waiter {
				continue
			}
			inDegree[holder]++
			adjacency[edge.Waiter] = append(adjacency[edge.Waiter], holder)
		}
	}

	allOwners := make(map[OwnerId]bool)
	for owner := range adjacency {
		allOwners[owner] = true
	}

	queue := make([]OwnerId, 0)
	for owner, deg := range inDegree {
		if deg == 0 {
			queue = append(queue, owner)
		}
	}

	for len(queue) > 0 {
		owner := queue[0]
		queue = queue[1:]

		if waiters, ok := adjacency[owner]; ok {
			for _, waiter := range waiters {
				inDegree[waiter]--
				if inDegree[waiter] == 0 {
					queue = append(queue, waiter)
				}
			}
		}
	}

	var remaining []OwnerId
	for owner := range allOwners {
		if inDegree[owner] > 0 {
			remaining = append(remaining, owner)
		}
	}

	return remaining
}

type LockManager struct {
	mu        sync.RWMutex
	locks     map[LockId]*LockEntry
	waitGraph *WaitForGraph
	logger    *logrus.Logger
}

func NewLockManager(logger *logrus.Logger) *LockManager {
	if logger == nil {
		logger = logrus.New()
	}
	return &LockManager{
		locks:     make(map[LockId]*LockEntry),
		waitGraph: NewWaitForGraph(),
		logger:    logger,
	}
}

func (m *LockManager) Acquire(ctx context.Context, req LockRequest) (*LockResponse, error) {
	if req.TTL == 0 {
		return nil, ErrInvalidTTL
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	if existing, ok := m.locks[req.LockId]; ok {
		switch existing.Status {
		case Expired:
			delete(m.locks, req.LockId)
			m.waitGraph.RemoveEdgesForLock(&req.LockId)
		case Held:
			if existing.Owner == req.Owner {
				if existing.Mode == Shared && req.Mode == Exclusive {
					return &LockResponse{
						RequestID: req.RequestID,
						LockId:    req.LockId,
						Owner:     req.Owner,
						Granted:   false,
						Error:     stringPtr("already holds in shared mode"),
					}, nil
				}
				return &LockResponse{
					RequestID: req.RequestID,
					LockId:    req.LockId,
					Owner:     req.Owner,
					Granted:   true,
					HoldToken: &existing.HoldToken,
					ExpiresAt: &existing.ExpiresAt,
				}, nil
			}
			if existing.Mode == Exclusive {
				return &LockResponse{
					RequestID: req.RequestID,
					LockId:    req.LockId,
					Owner:     req.Owner,
					Granted:   false,
					Error:     stringPtr("lock held exclusively by another owner"),
				}, nil
			}
			if req.Mode == Shared {
				return &LockResponse{
					RequestID: req.RequestID,
					LockId:    req.LockId,
					Owner:     req.Owner,
					Granted:   true,
				}, nil
			}
			if m.waitGraph.DetectCycle() != nil {
				return &LockResponse{
					RequestID: req.RequestID,
					LockId:    req.LockId,
					Owner:     req.Owner,
					Granted:   false,
					Error:     stringPtr("deadlock detected"),
				}, nil
			}
			m.waitGraph.AddEdge(WaitEdge{
				Waiter:        req.Owner,
				LockId:        req.LockId,
				RequestedMode: req.Mode,
			})
			return &LockResponse{
				RequestID: req.RequestID,
				LockId:    req.LockId,
				Owner:     req.Owner,
				Granted:   false,
				Status:    Pending,
			}, nil
		}
	}

	entry := NewLockEntry(req.LockId, req.Owner, req.Mode, req.TTL)
	m.locks[req.LockId] = entry
	m.waitGraph.SetLockHolder(req.LockId, req.Owner)

	return &LockResponse{
		RequestID: req.RequestID,
		LockId:    req.LockId,
		Owner:     req.Owner,
		Granted:   true,
		HoldToken: &entry.HoldToken,
		ExpiresAt: &entry.ExpiresAt,
	}, nil
}

func (m *LockManager) Release(ctx context.Context, release LockRelease) (*LockResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	entry, ok := m.locks[release.LockId]
	if !ok {
		return &LockResponse{
			RequestID: release.LockId.String(),
			LockId:    release.LockId,
			Owner:     release.Owner,
			Granted:   false,
			Error:     stringPtr(ErrNotFound.Error()),
		}, ErrNotFound
	}

	if entry.Owner != release.Owner {
		return &LockResponse{
			RequestID: release.LockId.String(),
			LockId:    release.LockId,
			Owner:     release.Owner,
			Granted:   false,
			Error:     stringPtr(ErrNotOwner.Error()),
		}, ErrNotOwner
	}

	if entry.HoldToken != release.HoldToken {
		return &LockResponse{
			RequestID: release.LockId.String(),
			LockId:    release.LockId,
			Owner:     release.Owner,
			Granted:   false,
			Error:     stringPtr(ErrInvalidToken.Error()),
		}, ErrInvalidToken
	}

	delete(m.locks, release.LockId)
	m.waitGraph.RemoveEdgesForOwner(&release.Owner)
	m.waitGraph.RemoveEdgesForLock(&release.LockId)

	m.logger.WithFields(logrus.Fields{
		"lock_id": release.LockId,
		"owner":   release.Owner,
		"action":  "released",
	}).Info("Lock released")

	return &LockResponse{
		RequestID: release.LockId.String(),
		LockId:    release.LockId,
		Owner:     release.Owner,
		Granted:   true,
	}, nil
}

func (m *LockManager) Promote(ctx context.Context, promote LockPromote) (*LockPromoteResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	entry, ok := m.locks[promote.LockId]
	if !ok {
		return &LockPromoteResponse{
			RequestID: promote.LockId.String(),
			LockId:    promote.LockId,
			Granted:   false,
			Error:     stringPtr(ErrNotFound.Error()),
		}, ErrNotFound
	}

	if entry.Owner != promote.Owner {
		return &LockPromoteResponse{
			RequestID: promote.LockId.String(),
			LockId:    promote.LockId,
			Granted:   false,
			Error:     stringPtr(ErrNotOwner.Error()),
		}, ErrNotOwner
	}

	if entry.HoldToken != promote.HoldToken {
		return &LockPromoteResponse{
			RequestID: promote.LockId.String(),
			LockId:    promote.LockId,
			Granted:   false,
			Error:     stringPtr(ErrInvalidToken.Error()),
		}, ErrInvalidToken
	}

	if entry.Mode == Exclusive && promote.NewMode == Shared {
		entry.Mode = promote.NewMode
		return &LockPromoteResponse{
			RequestID: promote.LockId.String(),
			LockId:    promote.LockId,
			Granted:   true,
			NewMode:   &promote.NewMode,
		}, nil
	}

	if entry.Mode == Shared && promote.NewMode == Exclusive {
		if m.waitGraph.DetectCycle() != nil {
			return &LockPromoteResponse{
				RequestID: promote.LockId.String(),
				LockId:    promote.LockId,
				Granted:   false,
				Error:     stringPtr(ErrDeadlockDetected.Error()),
			}, ErrDeadlockDetected
		}
		entry.Mode = promote.NewMode
		return &LockPromoteResponse{
			RequestID: promote.LockId.String(),
			LockId:    promote.LockId,
			Granted:   true,
			NewMode:   &promote.NewMode,
		}, nil
	}

	return &LockPromoteResponse{
		RequestID: promote.LockId.String(),
		LockId:    promote.LockId,
		Granted:   false,
		Error:     stringPtr("invalid mode transition"),
	}, ErrIncompatibleMode
}

func (m *LockManager) Query(ctx context.Context, query LockQuery) (*LockQueryResponse, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var result []*LockEntry
	for _, entry := range m.locks {
		if query.LockId != nil && entry.LockId != *query.LockId {
			continue
		}
		if query.Owner != nil && entry.Owner != *query.Owner {
			continue
		}
		result = append(result, entry)
	}

	return &LockQueryResponse{Locks: result}, nil
}

func (m *LockManager) Renew(ctx context.Context, lockId LockId, owner OwnerId, holdToken string, ttlMs uint64) (*LockResponse, error) {
	if ttlMs == 0 {
		return nil, ErrInvalidTTL
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	entry, ok := m.locks[lockId]
	if !ok {
		return &LockResponse{
			RequestID: lockId.String(),
			LockId:    lockId,
			Owner:     owner,
			Granted:   false,
			Error:     stringPtr(ErrNotFound.Error()),
		}, ErrNotFound
	}

	if entry.Owner != owner {
		return &LockResponse{
			RequestID: lockId.String(),
			LockId:    lockId,
			Owner:     owner,
			Granted:   false,
			Error:     stringPtr(ErrNotOwner.Error()),
		}, ErrNotOwner
	}

	if entry.HoldToken != holdToken {
		return &LockResponse{
			RequestID: lockId.String(),
			LockId:    lockId,
			Owner:     owner,
			Granted:   false,
			Error:     stringPtr(ErrInvalidToken.Error()),
		}, ErrInvalidToken
	}

	now := time.Now().UTC()
	entry.ExpiresAt = now.Add(time.Duration(ttlMs) * time.Millisecond)
	entry.AcquiredAt = now

	return &LockResponse{
		RequestID: lockId.String(),
		LockId:    lockId,
		Owner:     owner,
		Granted:   true,
		HoldToken: &entry.HoldToken,
		ExpiresAt: &entry.ExpiresAt,
	}, nil
}

func stringPtr(s string) *string {
	return &s
}

// Export locks map for test access
func (m *LockManager) GetLocks() map[LockId]*LockEntry {
	return m.locks
}
