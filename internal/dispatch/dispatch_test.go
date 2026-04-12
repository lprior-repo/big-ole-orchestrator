package dispatch

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
)

func newTestDispatcher() *Dispatcher {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)
	return NewDispatcher(logger)
}

func echoHandler(ctx context.Context, cmd *Command) (*CommandResult, error) {
	return &CommandResult{
		CommandID: cmd.ID,
		Data:      cmd.Payload,
	}, nil
}

func failingHandler(ctx context.Context, cmd *Command) (*CommandResult, error) {
	return nil, errors.New("intentional failure")
}

func TestDispatcherRegisterAndDispatch(t *testing.T) {
	d := newTestDispatcher()
	d.Register("echo", echoHandler)

	cmd := &Command{
		ID:      "cmd-1",
		Name:    "echo",
		Payload: "hello",
		Ctx:     context.Background(),
	}

	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}
	if result.Status != StatusCompleted {
		t.Errorf("Expected StatusCompleted, got %s", result.Status)
	}
	if result.Data != "hello" {
		t.Errorf("Expected data 'hello', got %v", result.Data)
	}
	if result.Duration == 0 {
		t.Error("Expected non-zero duration")
	}
}

func TestDispatcherUnknownCommand(t *testing.T) {
	d := newTestDispatcher()

	cmd := &Command{
		ID:   "cmd-1",
		Name: "nonexistent",
	}

	_, err := d.Dispatch(context.Background(), cmd)
	if err == nil {
		t.Fatal("Expected error for unknown command")
	}

	var cmdErr *CommandError
	if !errors.As(err, &cmdErr) {
		t.Fatalf("Expected *CommandError, got %T", err)
	}
	if cmdErr.Code != "UNKNOWN_COMMAND" {
		t.Errorf("Expected UNKNOWN_COMMAND code, got %s", cmdErr.Code)
	}
}

func TestDispatcherValidationPasses(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	called := false
	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		called = true
		return nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}
	if !called {
		t.Error("Validator should have been called")
	}
	if result.Status != StatusCompleted {
		t.Errorf("Expected StatusCompleted, got %s", result.Status)
	}
}

func TestDispatcherValidationFails(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	handlerCalled := false
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		handlerCalled = true
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		return errors.New("invalid payload")
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err == nil {
		t.Fatal("Expected validation error")
	}
	if handlerCalled {
		t.Error("Handler should NOT have been called after validation failure")
	}
	if result.Status != StatusFailed {
		t.Errorf("Expected StatusFailed, got %s", result.Status)
	}
	if result.Error == nil {
		t.Fatal("Expected error in result")
	}
	if result.Error.Phase != "validation" {
		t.Errorf("Expected validation phase, got %s", result.Error.Phase)
	}
}

func TestDispatcherAuthorizationFails(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	handlerCalled := false
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		handlerCalled = true
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	d.AddAuthorizer("test", func(ctx context.Context, cmd *Command) error {
		return errors.New("unauthorized")
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err == nil {
		t.Fatal("Expected authorization error")
	}
	if handlerCalled {
		t.Error("Handler should NOT have been called after auth failure")
	}
	if result.Error.Phase != "authorization" {
		t.Errorf("Expected authorization phase, got %s", result.Error.Phase)
	}
	if result.Error.Code != "AUTHORIZATION_FAILED" {
		t.Errorf("Expected AUTHORIZATION_FAILED code, got %s", result.Error.Code)
	}
}

func TestDispatcherMultipleValidators(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	var callOrder []string
	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		callOrder = append(callOrder, "v1")
		return nil
	})
	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		callOrder = append(callOrder, "v2")
		return nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	_, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}
	if len(callOrder) != 2 || callOrder[0] != "v1" || callOrder[1] != "v2" {
		t.Errorf("Expected validators called in order [v1, v2], got %v", callOrder)
	}
}

func TestDispatcherValidationStopsOnFirstError(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	v2Called := false
	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		return errors.New("fail")
	})
	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		v2Called = true
		return nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	d.Dispatch(context.Background(), cmd)
	if v2Called {
		t.Error("Second validator should not be called after first fails")
	}
}

func TestDispatcherMiddlewareChaining(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	var callOrder []string
	d.Use("mw1", func(ctx context.Context, cmd *Command, next MiddlewareFunc) (*CommandResult, error) {
		callOrder = append(callOrder, "mw1-before")
		result, err := next(ctx, cmd, nil)
		callOrder = append(callOrder, "mw1-after")
		return result, err
	})
	d.Use("mw2", func(ctx context.Context, cmd *Command, next MiddlewareFunc) (*CommandResult, error) {
		callOrder = append(callOrder, "mw2-before")
		result, err := next(ctx, cmd, nil)
		callOrder = append(callOrder, "mw2-after")
		return result, err
	})

	cmd := &Command{ID: "cmd-1", Name: "test", Payload: "data"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}
	if result.Status != StatusCompleted {
		t.Errorf("Expected StatusCompleted, got %s", result.Status)
	}

	expected := []string{"mw1-before", "mw2-before", "mw2-after", "mw1-after"}
	if len(callOrder) != len(expected) {
		t.Fatalf("Expected %d calls, got %d: %v", len(expected), len(callOrder), callOrder)
	}
	for i, exp := range expected {
		if callOrder[i] != exp {
			t.Errorf("At position %d: expected %s, got %s", i, exp, callOrder[i])
		}
	}
}

func TestDispatcherMiddlewareCanShortCircuit(t *testing.T) {
	d := newTestDispatcher()

	handlerCalled := false
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		handlerCalled = true
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	d.Use("blocker", func(ctx context.Context, cmd *Command, next MiddlewareFunc) (*CommandResult, error) {
		return &CommandResult{
			CommandID: cmd.ID,
			Status:    StatusFailed,
			Error: &CommandError{
				Phase:   "middleware",
				Message: "blocked",
				Code:    "BLOCKED",
			},
		}, nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Middleware short-circuit should not return error: %v", err)
	}
	if handlerCalled {
		t.Error("Handler should NOT be called when middleware short-circuits")
	}
	if result.Status != StatusFailed {
		t.Errorf("Expected StatusFailed, got %s", result.Status)
	}
}

func TestDispatcherPostHooks(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", echoHandler)

	var hookResults []*CommandResult
	var mu sync.Mutex
	d.AddPostHook("test", func(ctx context.Context, cmd *Command, result *CommandResult) {
		mu.Lock()
		hookResults = append(hookResults, result)
		mu.Unlock()
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, _ := d.Dispatch(context.Background(), cmd)

	mu.Lock()
	defer mu.Unlock()
	if len(hookResults) != 1 {
		t.Fatalf("Expected 1 post-hook call, got %d", len(hookResults))
	}
	if hookResults[0].CommandID != result.CommandID {
		t.Error("Post-hook should receive the result")
	}
}

func TestDispatcherPostHooksCalledOnFailure(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", failingHandler)

	hookCalled := false
	d.AddPostHook("test", func(ctx context.Context, cmd *Command, result *CommandResult) {
		hookCalled = true
		if result.Status != StatusFailed {
			t.Errorf("Post-hook expected StatusFailed, got %s", result.Status)
		}
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	d.Dispatch(context.Background(), cmd)
	if !hookCalled {
		t.Error("Post-hook should be called even on failure")
	}
}

func TestDispatcherPostHookPanicRecovery(t *testing.T) {
	logger := logrus.New()
	logger.SetLevel(logrus.ErrorLevel)
	d := NewDispatcher(logger)
	d.Register("test", echoHandler)

	panicHookCalled := false
	secondHookCalled := false
	d.AddPostHook("test", func(ctx context.Context, cmd *Command, result *CommandResult) {
		panicHookCalled = true
		panic("hook panic")
	})
	d.AddPostHook("test", func(ctx context.Context, cmd *Command, result *CommandResult) {
		secondHookCalled = true
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch should not fail from post-hook panic: %v", err)
	}
	if result.Status != StatusCompleted {
		t.Errorf("Main result should still be completed: %s", result.Status)
	}
	if !panicHookCalled {
		t.Error("Panicking hook should have been called")
	}
	if !secondHookCalled {
		t.Error("Second hook should still run after first hook panics")
	}
}

func TestDispatcherAsyncDispatch(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		time.Sleep(50 * time.Millisecond)
		return &CommandResult{
			CommandID: cmd.ID,
			Data:      "async-done",
		}, nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	resultCh, err := d.DispatchAsync(context.Background(), cmd)
	if err != nil {
		t.Fatalf("DispatchAsync failed: %v", err)
	}

	select {
	case result := <-resultCh:
		if result.Status != StatusCompleted {
			t.Errorf("Expected StatusCompleted, got %s", result.Status)
		}
		if result.Data != "async-done" {
			t.Errorf("Expected 'async-done', got %v", result.Data)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("Async dispatch timed out")
	}
}

func TestDispatcherAsyncCancel(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(5 * time.Second):
			return &CommandResult{CommandID: cmd.ID}, nil
		}
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	resultCh, err := d.DispatchAsync(context.Background(), cmd)
	if err != nil {
		t.Fatalf("DispatchAsync failed: %v", err)
	}

	time.Sleep(50 * time.Millisecond)
	cancelled := d.Cancel("cmd-1")
	if !cancelled {
		t.Fatal("Cancel should return true for active command")
	}

	select {
	case result := <-resultCh:
		if result.Status != StatusFailed {
			t.Errorf("Expected StatusFailed after cancel, got %s", result.Status)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("Cancelled dispatch should complete")
	}
}

func TestDispatcherCancelNonExistent(t *testing.T) {
	d := newTestDispatcher()
	if d.Cancel("nonexistent") {
		t.Error("Cancel should return false for non-existent command")
	}
}

func TestDispatcherProgressReporting(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		d.ReportProgress(cmd.ID, StatusRunning, 0.7, "working")
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	resultCh, _ := d.DispatchAsync(context.Background(), cmd)

	progressReports := []ProgressReport{}
	timeout := time.After(2 * time.Second)
collect:
	for {
		select {
		case report, ok := <-d.ProgressChannel():
			if !ok {
				break collect
			}
			progressReports = append(progressReports, report)
			if report.Status == StatusCompleted || report.Status == StatusFailed || report.Status == StatusCancelled {
				if len(progressReports) >= 2 {
					break collect
				}
			}
		case <-timeout:
			break collect
		}
	}

	<-resultCh

	found := false
	for _, r := range progressReports {
		if r.CommandID == "cmd-1" && r.Status == StatusRunning && r.Progress == 0.7 {
			found = true
		}
	}
	if !found {
		t.Errorf("Expected progress report with status=running progress=0.7, got %d reports", len(progressReports))
	}
}

func TestDispatcherPipelineOrder(t *testing.T) {
	d := newTestDispatcher()

	var order []string
	var mu sync.Mutex
	record := func(phase string) {
		mu.Lock()
		order = append(order, phase)
		mu.Unlock()
	}

	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		record("handler")
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	d.AddValidator("test", func(ctx context.Context, cmd *Command) error {
		record("validate")
		return nil
	})
	d.AddAuthorizer("test", func(ctx context.Context, cmd *Command) error {
		record("authorize")
		return nil
	})
	d.Use("mw", func(ctx context.Context, cmd *Command, next MiddlewareFunc) (*CommandResult, error) {
		record("middleware-before")
		result, err := next(ctx, cmd, nil)
		record("middleware-after")
		return result, err
	})
	d.AddPostHook("test", func(ctx context.Context, cmd *Command, result *CommandResult) {
		record("post-hook")
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	_, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}

	mu.Lock()
	defer mu.Unlock()
	expected := []string{"validate", "authorize", "middleware-before", "handler", "middleware-after", "post-hook"}
	if len(order) != len(expected) {
		t.Fatalf("Expected %d phases, got %d: %v", len(expected), len(order), order)
	}
	for i, exp := range expected {
		if order[i] != exp {
			t.Errorf("Phase %d: expected %s, got %s", i, exp, order[i])
		}
	}
}

func TestDispatcherHeaders(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		return &CommandResult{
			CommandID: cmd.ID,
			Data:      cmd.Headers["x-test"],
		}, nil
	})

	cmd := &Command{
		ID:   "cmd-1",
		Name: "test",
		Headers: map[string]string{
			"x-test": "value",
		},
	}

	result, err := d.Dispatch(context.Background(), cmd)
	if err != nil {
		t.Fatalf("Dispatch failed: %v", err)
	}
	if result.Data != "value" {
		t.Errorf("Expected header value 'value', got %v", result.Data)
	}
}

func TestDispatcherCommandErrorFormat(t *testing.T) {
	err := &CommandError{
		Phase:   "validation",
		Message: "bad input",
		Code:    "INVALID",
	}
	expected := "validation: bad input (INVALID)"
	if err.Error() != expected {
		t.Errorf("Expected %q, got %q", expected, err.Error())
	}
}

func TestDispatcherConcurrentDispatch(t *testing.T) {
	d := newTestDispatcher()
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		time.Sleep(10 * time.Millisecond)
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	var wg sync.WaitGroup
	var errors_ atomic.Int64
	const n = 50

	for i := 0; i < n; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			cmd := &Command{
				ID:   fmt.Sprintf("cmd-%d", i),
				Name: "test",
			}
			result, err := d.Dispatch(context.Background(), cmd)
			if err != nil || result.Status != StatusCompleted {
				errors_.Add(1)
			}
		}(i)
	}

	wg.Wait()
	if count := errors_.Load(); count > 0 {
		t.Errorf("Expected 0 errors in concurrent dispatch, got %d", count)
	}
}

func TestDispatcherActiveCommands(t *testing.T) {
	d := newTestDispatcher()
	started := make(chan struct{})
	d.Register("test", func(ctx context.Context, cmd *Command) (*CommandResult, error) {
		close(started)
		time.Sleep(100 * time.Millisecond)
		return &CommandResult{CommandID: cmd.ID}, nil
	})

	cmd := &Command{ID: "cmd-1", Name: "test"}
	resultCh, _ := d.DispatchAsync(context.Background(), cmd)

	<-started
	active := d.ActiveCommands()
	if len(active) != 1 {
		t.Errorf("Expected 1 active command, got %d", len(active))
	}
	if active[0].CommandID != "cmd-1" {
		t.Errorf("Expected cmd-1, got %s", active[0].CommandID)
	}

	<-resultCh

	time.Sleep(50 * time.Millisecond)
	active = d.ActiveCommands()
	if len(active) != 0 {
		t.Errorf("Expected 0 active commands after completion, got %d", len(active))
	}
}
