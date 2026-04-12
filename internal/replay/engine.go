package replay

import "fmt"

type EventPayload interface {
	isPayload()
}

type WorkflowStarted struct{}
type StepScheduled struct{}
type StepStarted struct{}
type StepCompleted struct{}
type StepFailed struct{}
type TimerSet struct{}
type TimerFired struct{}
type WorkflowCompleted struct{}
type WorkflowFailed struct{}
type WorkflowCancelled struct{}
type CancelRequested struct{}
type InstanceResumed struct{}
type ContinuedAsNew struct{}

func (WorkflowStarted) isPayload()   {}
func (StepScheduled) isPayload()     {}
func (StepStarted) isPayload()       {}
func (StepCompleted) isPayload()     {}
func (StepFailed) isPayload()        {}
func (TimerSet) isPayload()          {}
func (TimerFired) isPayload()        {}
func (WorkflowCompleted) isPayload() {}
func (WorkflowFailed) isPayload()    {}
func (WorkflowCancelled) isPayload() {}
func (CancelRequested) isPayload()   {}
func (InstanceResumed) isPayload()   {}
func (ContinuedAsNew) isPayload()    {}

type EventEnvelope struct {
	InstanceID string
	Sequence   uint64
	Payload    EventPayload
}

type Engine struct{}

func NewEngine() *Engine {
	return &Engine{}
}

func (e *Engine) Replay(events []*EventEnvelope) (*ReplayResult, error) {
	if len(events) == 0 {
		return &ReplayResult{
			FinalState:    nil,
			EventsApplied: 0,
		}, nil
	}

	expectedInstanceID := events[0].InstanceID
	for _, event := range events[1:] {
		if event.InstanceID != expectedInstanceID {
			return nil, NewInstanceMismatchError(expectedInstanceID, event.InstanceID)
		}
	}

	var expectedSeq uint64
	for i, event := range events {
		if i == 0 {
			expectedSeq = event.Sequence
			continue
		}
		if event.Sequence == expectedSeq {
			return nil, NewSequenceDuplicateError(event.Sequence, i-1, i)
		}
		if event.Sequence != expectedSeq+1 {
			return nil, NewSequenceGapError(expectedSeq+1, event.Sequence, i)
		}
		expectedSeq = event.Sequence
	}

	var currentState *LifecycleState
	eventsApplied := 0

	for _, event := range events {
		payload := event.Payload

		if _, ok := payload.(ContinuedAsNew); ok {
			eventsApplied++
			continue
		}

		transition, err := payloadToTransition(payload, event.Sequence)
		if err != nil {
			return nil, err
		}

		stateForApply := StatePending
		if currentState != nil {
			stateForApply = *currentState
		}

		newState, err := apply(stateForApply, transition, event.Sequence)
		if err != nil {
			return nil, err
		}

		currentState = &newState
		eventsApplied++

		if newState == StateCompleted || newState == StateCancelled {
			break
		}
	}

	return &ReplayResult{
		FinalState:    currentState,
		EventsApplied: eventsApplied,
	}, nil
}

func apply(currentState LifecycleState, event TransitionEvent, sequence uint64) (LifecycleState, error) {
	if !currentState.CanTransitionTo(event) {
		return "", NewTransitionFailedError(
			sequence,
			currentState,
			fmt.Sprintf("invalid transition: %s from state %s", event, currentState),
		)
	}

	var nextState LifecycleState
	switch currentState {
	case StatePending:
		switch event {
		case EventAssignToNode:
			nextState = StateRunningDecision
		case EventCancel:
			nextState = StateCancelled
		}
	case StateRunningDecision:
		switch event {
		case EventStepScheduled:
			nextState = StateStepScheduled
		case EventCancel:
			nextState = StateCancelled
		case EventFail:
			nextState = StateFailed
		}
	case StateStepScheduled:
		switch event {
		case EventExecuteStep:
			nextState = StateStepExecuting
		case EventCancel:
			nextState = StateCancelled
		case EventFail:
			nextState = StateFailed
		}
	case StateStepExecuting:
		switch event {
		case EventWaitForTimer:
			nextState = StateWaitingForTimer
		case EventCompleteStep:
			nextState = StateCompleted
		case EventCancel:
			nextState = StateCancelled
		case EventFail:
			nextState = StateFailed
		}
	case StateWaitingForTimer:
		switch event {
		case EventTimerFired:
			nextState = StateStepExecuting
		case EventTimerExpired:
			nextState = StateFailed
		case EventCancel:
			nextState = StateCancelled
		case EventFail:
			nextState = StateFailed
		}
	case StateFailed:
		if event == EventInstanceResumed {
			nextState = StateRunningDecision
		}
	case StateCompleted, StateCancelled:
		return currentState, nil
	}

	return nextState, nil
}

func payloadToTransition(payload EventPayload, sequence uint64) (TransitionEvent, error) {
	switch p := payload.(type) {
	case WorkflowStarted:
		return EventAssignToNode, nil
	case StepScheduled:
		return EventStepScheduled, nil
	case StepStarted:
		return EventExecuteStep, nil
	case StepCompleted:
		return EventCompleteStep, nil
	case StepFailed:
		return EventFail, nil
	case TimerSet:
		return EventWaitForTimer, nil
	case TimerFired:
		return EventTimerFired, nil
	case WorkflowCompleted:
		return EventCompleteStep, nil
	case WorkflowFailed:
		return EventFail, nil
	case WorkflowCancelled:
		return EventCancel, nil
	case CancelRequested:
		return EventCancel, nil
	case InstanceResumed:
		return EventInstanceResumed, nil
	case ContinuedAsNew:
		return "", NewUnexpectedEventError("ContinuedAsNew", sequence)
	default:
		return "", NewUnexpectedEventError(fmt.Sprintf("%T", p), sequence)
	}
}
