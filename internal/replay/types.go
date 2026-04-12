package replay

import "fmt"

type LifecycleState string

const (
	StatePending         LifecycleState = "Pending"
	StateRunningDecision LifecycleState = "RunningDecision"
	StateStepScheduled   LifecycleState = "StepScheduled"
	StateStepExecuting   LifecycleState = "StepExecuting"
	StateWaitingForTimer LifecycleState = "WaitingForTimer"
	StateCompleted       LifecycleState = "Completed"
	StateFailed          LifecycleState = "Failed"
	StateCancelled       LifecycleState = "Cancelled"
)

type TransitionEvent string

const (
	EventAssignToNode    TransitionEvent = "AssignToNode"
	EventCancel          TransitionEvent = "Cancel"
	EventStepScheduled   TransitionEvent = "StepScheduled"
	EventFail            TransitionEvent = "Fail"
	EventExecuteStep     TransitionEvent = "ExecuteStep"
	EventWaitForTimer    TransitionEvent = "WaitForTimer"
	EventCompleteStep    TransitionEvent = "CompleteStep"
	EventTimerFired      TransitionEvent = "TimerFired"
	EventTimerExpired    TransitionEvent = "TimerExpired"
	EventInstanceResumed TransitionEvent = "InstanceResumed"
)

func (s LifecycleState) IsTerminal() bool {
	switch s {
	case StateCompleted, StateFailed, StateCancelled:
		return true
	default:
		return false
	}
}

func (s LifecycleState) ValidTransitions() []TransitionEvent {
	switch s {
	case StatePending:
		return []TransitionEvent{EventAssignToNode, EventCancel}
	case StateRunningDecision:
		return []TransitionEvent{EventStepScheduled, EventCancel, EventFail}
	case StateStepScheduled:
		return []TransitionEvent{EventExecuteStep, EventCancel, EventFail}
	case StateStepExecuting:
		return []TransitionEvent{EventWaitForTimer, EventCompleteStep, EventCancel, EventFail}
	case StateWaitingForTimer:
		return []TransitionEvent{EventTimerFired, EventTimerExpired, EventCancel, EventFail}
	case StateCompleted, StateCancelled:
		return nil
	case StateFailed:
		return []TransitionEvent{EventInstanceResumed}
	default:
		return nil
	}
}

func (s LifecycleState) CanTransitionTo(event TransitionEvent) bool {
	for _, valid := range s.ValidTransitions() {
		if valid == event {
			return true
		}
	}
	return false
}

type ReplayResult struct {
	FinalState    *LifecycleState
	EventsApplied int
}

type ReplayError struct {
	Kind    ReplayErrorKind
	Message string
}

type ReplayErrorKind string

const (
	ErrInstanceMismatch  ReplayErrorKind = "InstanceMismatch"
	ErrSequenceGap       ReplayErrorKind = "SequenceGap"
	ErrSequenceDuplicate ReplayErrorKind = "SequenceDuplicate"
	ErrPayloadDecode     ReplayErrorKind = "PayloadDecodeFailed"
	ErrTransitionFailed  ReplayErrorKind = "TransitionFailed"
	ErrUnexpectedEvent   ReplayErrorKind = "UnexpectedEventType"
)

func (e *ReplayError) Error() string {
	return fmt.Sprintf("%s: %s", e.Kind, e.Message)
}

func NewInstanceMismatchError(expected, actual string) *ReplayError {
	return &ReplayError{
		Kind:    ErrInstanceMismatch,
		Message: fmt.Sprintf("instance ID mismatch: expected '%s', got '%s'", expected, actual),
	}
}

func NewSequenceGapError(expected, actual uint64, atIndex int) *ReplayError {
	return &ReplayError{
		Kind:    ErrSequenceGap,
		Message: fmt.Sprintf("sequence gap at index %d: expected %d, got %d", atIndex, expected, actual),
	}
}

func NewSequenceDuplicateError(sequence uint64, firstIdx, secondIdx int) *ReplayError {
	return &ReplayError{
		Kind:    ErrSequenceDuplicate,
		Message: fmt.Sprintf("duplicate sequence %d at indices %d and %d", sequence, firstIdx, secondIdx),
	}
}

func NewPayloadDecodeError(sequence uint64, source string) *ReplayError {
	return &ReplayError{
		Kind:    ErrPayloadDecode,
		Message: fmt.Sprintf("payload decode failed at sequence %d: %s", sequence, source),
	}
}

func NewTransitionFailedError(sequence uint64, state LifecycleState, reason string) *ReplayError {
	return &ReplayError{
		Kind:    ErrTransitionFailed,
		Message: fmt.Sprintf("transition failed at sequence %d in state %s: %s", sequence, state, reason),
	}
}

func NewUnexpectedEventError(payloadType string, sequence uint64) *ReplayError {
	return &ReplayError{
		Kind:    ErrUnexpectedEvent,
		Message: fmt.Sprintf("unexpected event type '%s' at sequence %d", payloadType, sequence),
	}
}
