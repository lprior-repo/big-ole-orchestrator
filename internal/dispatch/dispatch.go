package dispatch

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
)

type CommandName string

type CommandStatus string

const (
	StatusPending    CommandStatus = "pending"
	StatusValidating CommandStatus = "validating"
	StatusAuthorized CommandStatus = "authorized"
	StatusRunning    CommandStatus = "running"
	StatusCompleted  CommandStatus = "completed"
	StatusFailed     CommandStatus = "failed"
	StatusCancelled  CommandStatus = "cancelled"
)

type CommandError struct {
	Phase   string `json:"phase"`
	Message string `json:"message"`
	Code    string `json:"code"`
}

func (e *CommandError) Error() string {
	return fmt.Sprintf("%s: %s (%s)", e.Phase, e.Message, e.Code)
}

type ProgressReport struct {
	CommandID string        `json:"command_id"`
	Status    CommandStatus `json:"status"`
	Progress  float64       `json:"progress"`
	Message   string        `json:"message"`
	Timestamp time.Time     `json:"timestamp"`
}

type CommandResult struct {
	CommandID string        `json:"command_id"`
	Status    CommandStatus `json:"status"`
	Data      interface{}   `json:"data,omitempty"`
	Error     *CommandError `json:"error,omitempty"`
	Duration  time.Duration `json:"duration"`
}

type Command struct {
	ID      string            `json:"id"`
	Name    CommandName       `json:"name"`
	Payload interface{}       `json:"payload"`
	Headers map[string]string `json:"headers,omitempty"`
	Ctx     context.Context   `json:"-"`
}

type MiddlewareFunc func(ctx context.Context, cmd *Command, next MiddlewareFunc) (*CommandResult, error)

type HandlerFunc func(ctx context.Context, cmd *Command) (*CommandResult, error)

type ValidatorFunc func(ctx context.Context, cmd *Command) error

type AuthorizerFunc func(ctx context.Context, cmd *Command) error

type PostHookFunc func(ctx context.Context, cmd *Command, result *CommandResult)

type ProgressReporter func(report ProgressReport)

type MiddlewareRegistration struct {
	Name string
	Fn   MiddlewareFunc
}

type Dispatcher struct {
	mu           sync.RWMutex
	handlers     map[CommandName]HandlerFunc
	middlewares  []MiddlewareRegistration
	validators   map[CommandName][]ValidatorFunc
	authorizers  map[CommandName][]AuthorizerFunc
	postHooks    map[CommandName][]PostHookFunc
	logger       *logrus.Logger
	progressCh   chan ProgressReport
	activeCmds   map[string]*activeCommand
	activeCmdsMu sync.RWMutex
}

type activeCommand struct {
	cmd      *Command
	cancel   context.CancelFunc
	status   CommandStatus
	progress float64
}

func NewDispatcher(logger *logrus.Logger) *Dispatcher {
	if logger == nil {
		logger = logrus.New()
		logger.SetLevel(logrus.WarnLevel)
	}
	return &Dispatcher{
		handlers:    make(map[CommandName]HandlerFunc),
		middlewares: make([]MiddlewareRegistration, 0),
		validators:  make(map[CommandName][]ValidatorFunc),
		authorizers: make(map[CommandName][]AuthorizerFunc),
		postHooks:   make(map[CommandName][]PostHookFunc),
		logger:      logger,
		progressCh:  make(chan ProgressReport, 256),
		activeCmds:  make(map[string]*activeCommand),
	}
}

func (d *Dispatcher) Register(name CommandName, handler HandlerFunc) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.handlers[name] = handler
}

func (d *Dispatcher) Use(name string, mw MiddlewareFunc) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.middlewares = append(d.middlewares, MiddlewareRegistration{Name: name, Fn: mw})
}

func (d *Dispatcher) AddValidator(name CommandName, v ValidatorFunc) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.validators[name] = append(d.validators[name], v)
}

func (d *Dispatcher) AddAuthorizer(name CommandName, a AuthorizerFunc) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.authorizers[name] = append(d.authorizers[name], a)
}

func (d *Dispatcher) AddPostHook(name CommandName, h PostHookFunc) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.postHooks[name] = append(d.postHooks[name], h)
}

func (d *Dispatcher) ProgressChannel() <-chan ProgressReport {
	return d.progressCh
}

func (d *Dispatcher) ReportProgress(cmdID string, status CommandStatus, progress float64, message string) {
	report := ProgressReport{
		CommandID: cmdID,
		Status:    status,
		Progress:  progress,
		Message:   message,
		Timestamp: time.Now().UTC(),
	}
	select {
	case d.progressCh <- report:
	default:
		d.logger.Warnf("progress channel full, dropping report for %s", cmdID)
	}
}

func (d *Dispatcher) Dispatch(ctx context.Context, cmd *Command) (*CommandResult, error) {
	d.mu.RLock()
	handler, exists := d.handlers[cmd.Name]
	validators := d.validators[cmd.Name]
	authorizers := d.authorizers[cmd.Name]
	postHooks := d.postHooks[cmd.Name]
	middlewares := d.middlewares
	d.mu.RUnlock()

	if !exists {
		return nil, &CommandError{
			Phase:   "dispatch",
			Message: fmt.Sprintf("no handler registered for command: %s", cmd.Name),
			Code:    "UNKNOWN_COMMAND",
		}
	}

	start := time.Now()

	d.report(cmd.ID, StatusValidating, 0.1, "validating command")
	for _, v := range validators {
		if err := v(ctx, cmd); err != nil {
			result := &CommandResult{
				CommandID: cmd.ID,
				Status:    StatusFailed,
				Error: &CommandError{
					Phase:   "validation",
					Message: err.Error(),
					Code:    "VALIDATION_FAILED",
				},
				Duration: time.Since(start),
			}
			d.runPostHooks(ctx, cmd, postHooks, result)
			return result, err
		}
	}

	d.report(cmd.ID, StatusAuthorized, 0.3, "authorizing command")
	for _, a := range authorizers {
		if err := a(ctx, cmd); err != nil {
			result := &CommandResult{
				CommandID: cmd.ID,
				Status:    StatusFailed,
				Error: &CommandError{
					Phase:   "authorization",
					Message: err.Error(),
					Code:    "AUTHORIZATION_FAILED",
				},
				Duration: time.Since(start),
			}
			d.runPostHooks(ctx, cmd, postHooks, result)
			return result, err
		}
	}

	d.report(cmd.ID, StatusRunning, 0.5, "executing command")

	wrappedHandler := handler
	for i := len(middlewares) - 1; i >= 0; i-- {
		mw := middlewares[i]
		next := wrappedHandler
		mwFn := mw.Fn
		wrappedHandler = func(ctx context.Context, cmd *Command) (*CommandResult, error) {
			return mwFn(ctx, cmd, func(ctx context.Context, cmd *Command, _ MiddlewareFunc) (*CommandResult, error) {
				return next(ctx, cmd)
			})
		}
	}

	result, err := wrappedHandler(ctx, cmd)
	if result == nil {
		result = &CommandResult{
			CommandID: cmd.ID,
			Duration:  time.Since(start),
		}
	}
	result.Duration = time.Since(start)

	if err != nil {
		result.Status = StatusFailed
		if result.Error == nil {
			result.Error = &CommandError{
				Phase:   "execution",
				Message: err.Error(),
				Code:    "EXECUTION_FAILED",
			}
		}
	} else if result.Status == "" {
		result.Status = StatusCompleted
	}

	d.report(cmd.ID, result.Status, 1.0, "command finished")
	d.runPostHooks(ctx, cmd, postHooks, result)

	return result, err
}

func (d *Dispatcher) DispatchAsync(ctx context.Context, cmd *Command) (<-chan *CommandResult, error) {
	d.mu.RLock()
	_, exists := d.handlers[cmd.Name]
	d.mu.RUnlock()

	if !exists {
		return nil, &CommandError{
			Phase:   "dispatch",
			Message: fmt.Sprintf("no handler registered for command: %s", cmd.Name),
			Code:    "UNKNOWN_COMMAND",
		}
	}

	cmdCtx, cancel := context.WithCancel(ctx)
	d.trackCommand(cmd, cancel)

	resultCh := make(chan *CommandResult, 1)
	go func() {
		defer cancel()
		defer d.untrackCommand(cmd.ID)
		result, _ := d.Dispatch(cmdCtx, cmd)
		resultCh <- result
		close(resultCh)
	}()

	return resultCh, nil
}

func (d *Dispatcher) Cancel(commandID string) bool {
	d.activeCmdsMu.RLock()
	ac, exists := d.activeCmds[commandID]
	d.activeCmdsMu.RUnlock()

	if !exists {
		return false
	}

	ac.cancel()
	d.report(commandID, StatusCancelled, 1.0, "command cancelled")
	return true
}

func (d *Dispatcher) ActiveCommands() []ProgressReport {
	d.activeCmdsMu.RLock()
	defer d.activeCmdsMu.RUnlock()

	reports := make([]ProgressReport, 0, len(d.activeCmds))
	for id, ac := range d.activeCmds {
		reports = append(reports, ProgressReport{
			CommandID: id,
			Status:    ac.status,
			Progress:  ac.progress,
		})
	}
	return reports
}

func (d *Dispatcher) trackCommand(cmd *Command, cancel context.CancelFunc) {
	d.activeCmdsMu.Lock()
	defer d.activeCmdsMu.Unlock()
	d.activeCmds[cmd.ID] = &activeCommand{
		cmd:    cmd,
		cancel: cancel,
		status: StatusPending,
	}
}

func (d *Dispatcher) untrackCommand(id string) {
	d.activeCmdsMu.Lock()
	defer d.activeCmdsMu.Unlock()
	delete(d.activeCmds, id)
}

func (d *Dispatcher) report(cmdID string, status CommandStatus, progress float64, message string) {
	d.activeCmdsMu.Lock()
	if ac, ok := d.activeCmds[cmdID]; ok {
		ac.status = status
		ac.progress = progress
	}
	d.activeCmdsMu.Unlock()
	d.ReportProgress(cmdID, status, progress, message)
}

func (d *Dispatcher) runPostHooks(ctx context.Context, cmd *Command, hooks []PostHookFunc, result *CommandResult) {
	for _, h := range hooks {
		func() {
			defer func() {
				if r := recover(); r != nil {
					d.logger.Errorf("post-hook panic for command %s: %v", cmd.ID, r)
				}
			}()
			h(ctx, cmd, result)
		}()
	}
}
