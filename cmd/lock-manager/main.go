package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/lprior-repo/veloxide/internal/lockmanager"
	"github.com/sirupsen/logrus"
)

func main() {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)
	logger.SetFormatter(&logrus.TextFormatter{
		FullTimestamp: true,
	})

	lockManager := lockmanager.NewLockManager(logger)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		logger.Info("Received shutdown signal")
		cancel()
	}()

	// Example usage
	go func() {
		owner := lockmanager.NewOwnerId("worker-1")
		lockID := lockmanager.NewLockId("resource-A")

		req := lockmanager.LockRequest{
			LockId:    lockID,
			Owner:     owner,
			Mode:      lockmanager.Exclusive,
			TTL:       10000,
			RequestID: "req-1",
		}

		resp, err := lockManager.Acquire(ctx, req)
		if err != nil {
			logger.Errorf("Acquire failed: %v", err)
			return
		}

		if resp.Granted {
			logger.Infof("Lock acquired: %s (token: %s, expires: %v)",
				resp.LockId, *resp.HoldToken, *resp.ExpiresAt)
		}

		// Simulate work
		select {
		case <-ctx.Done():
			return
		case <-time.After(2 * time.Second):
		}

		// Release lock
		release := lockmanager.LockRelease{
			LockId:    lockID,
			Owner:     owner,
			HoldToken: *resp.HoldToken,
		}

		resp, err = lockManager.Release(ctx, release)
		if err != nil {
			logger.Errorf("Release failed: %v", err)
			return
		}

		if resp.Granted {
			logger.Infof("Lock released: %s", lockID)
		}
	}()

	<-ctx.Done()
	logger.Info("Shutdown complete")
}
