package replay

import (
	"errors"
	"fmt"
	"testing"
)

func TestLifecycleState_IsTerminal(t *testing.T) {
	tests := []struct {
		state    LifecycleState
		terminal bool
	}{
		{StatePending, false},
		{StateRunningDecision, false},
		{StateStepScheduled, false},
		{StateStepExecuting, false},
		{StateWaitingForTimer, false},
		{StateCompleted, true},
		{StateFailed, true},
		{StateCancelled, true},
	}

	for _, tc := range tests {
		t.Run(string(tc.state), func(t *testing.T) {
			if got := tc.state.IsTerminal(); got != tc.terminal {
				t.Errorf("IsTerminal() = %v, want %v", got, tc.terminal)
			}
		})
	}
}

func TestLifecycleState_CanTransitionTo(t *testing.T) {
	tests := []struct {
		state    LifecycleState
		event    TransitionEvent
		expected bool
	}{
		{StatePending, EventAssignToNode, true},
		{StatePending, EventCancel, true},
		{StatePending, EventStepScheduled, false},
		{StateRunningDecision, EventStepScheduled, true},
		{StateRunningDecision, EventCancel, true},
		{StateRunningDecision, EventFail, true},
		{StateStepScheduled, EventExecuteStep, true},
		{StateStepScheduled, EventCancel, true},
		{StateStepScheduled, EventFail, true},
		{StateStepExecuting, EventWaitForTimer, true},
		{StateStepExecuting, EventCompleteStep, true},
		{StateStepExecuting, EventCancel, true},
		{StateStepExecuting, EventFail, true},
		{StateWaitingForTimer, EventTimerFired, true},
		{StateWaitingForTimer, EventTimerExpired, true},
		{StateWaitingForTimer, EventCancel, true},
		{StateWaitingForTimer, EventFail, true},
		{StateCompleted, EventCancel, false},
		{StateFailed, EventInstanceResumed, true},
		{StateCancelled, EventCancel, false},
	}

	for _, tc := range tests {
		t.Run(string(tc.state)+"_"+string(tc.event), func(t *testing.T) {
			if got := tc.state.CanTransitionTo(tc.event); got != tc.expected {
				t.Errorf("CanTransitionTo(%s) = %v, want %v", tc.event, got, tc.expected)
			}
		})
	}
}

func TestReplayEngine_EmptyEvents(t *testing.T) {
	engine := NewEngine()
	result, err := engine.Replay([]*EventEnvelope{})

	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState != nil {
		t.Errorf("FinalState = %v, want nil", result.FinalState)
	}
	if result.EventsApplied != 0 {
		t.Errorf("EventsApplied = %d, want 0", result.EventsApplied)
	}
}

func TestReplayEngine_InstanceMismatch(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-2", Sequence: 2, Payload: StepScheduled{}},
	}

	_, err := engine.Replay(events)
	if err == nil {
		t.Fatal("Replay() error = nil, want instance mismatch error")
	}
	var replayErr *ReplayError
	if !errors.As(err, &replayErr) || replayErr.Kind != ErrInstanceMismatch {
		t.Errorf("error Kind = %v, want %s", err, ErrInstanceMismatch)
	}
}

func TestReplayEngine_SequenceGap(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepScheduled{}},
	}

	_, err := engine.Replay(events)
	if err == nil {
		t.Fatal("Replay() error = nil, want sequence gap error")
	}
	var replayErr *ReplayError
	if !errors.As(err, &replayErr) || replayErr.Kind != ErrSequenceGap {
		t.Errorf("error Kind = %v, want %s", err, ErrSequenceGap)
	}
}

func TestReplayEngine_SequenceDuplicate(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 1, Payload: StepScheduled{}},
	}

	_, err := engine.Replay(events)
	if err == nil {
		t.Fatal("Replay() error = nil, want sequence duplicate error")
	}
	var replayErr *ReplayError
	if !errors.As(err, &replayErr) || replayErr.Kind != ErrSequenceDuplicate {
		t.Errorf("error Kind = %v, want %s", err, ErrSequenceDuplicate)
	}
}

func TestReplayEngine_HappyPath(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepStarted{}},
		{InstanceID: "instance-1", Sequence: 4, Payload: StepCompleted{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateCompleted")
	}
	if *result.FinalState != StateCompleted {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateCompleted)
	}
	if result.EventsApplied != 4 {
		t.Errorf("EventsApplied = %d, want 4", result.EventsApplied)
	}
}

func TestReplayEngine_FailedState(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepFailed{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateFailed")
	}
	if *result.FinalState != StateFailed {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateFailed)
	}
}

func TestReplayEngine_CancelledState(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: CancelRequested{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateCancelled")
	}
	if *result.FinalState != StateCancelled {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateCancelled)
	}
}

func TestReplayEngine_ContinuedAsNewIsNoOp(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: ContinuedAsNew{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 4, Payload: StepStarted{}},
		{InstanceID: "instance-1", Sequence: 5, Payload: StepCompleted{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateCompleted")
	}
	if *result.FinalState != StateCompleted {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateCompleted)
	}
	if result.EventsApplied != 5 {
		t.Errorf("EventsApplied = %d, want 5 (ContinuedAsNew is no-op but counted)", result.EventsApplied)
	}
}

func TestReplayEngine_InvalidTransition(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: StepScheduled{}},
	}

	_, err := engine.Replay(events)
	if err == nil {
		t.Fatal("Replay() error = nil, want invalid transition error")
	}
	var replayErr *ReplayError
	if !errors.As(err, &replayErr) || replayErr.Kind != ErrTransitionFailed {
		t.Errorf("error Kind = %v, want %s", err, ErrTransitionFailed)
	}
}

func TestReplayEngine_UnexpectedEvent(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: &testUnknownPayload{}},
	}

	_, err := engine.Replay(events)
	if err == nil {
		t.Fatal("Replay() error = nil, want unexpected event error")
	}
	var replayErr *ReplayError
	if !errors.As(err, &replayErr) || replayErr.Kind != ErrUnexpectedEvent {
		t.Errorf("error Kind = %v, want %s", err, ErrUnexpectedEvent)
	}
}

type testUnknownPayload struct{}

func (testUnknownPayload) isPayload() {}

func TestReplayEngine_WaitingForTimer(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepStarted{}},
		{InstanceID: "instance-1", Sequence: 4, Payload: TimerSet{}},
		{InstanceID: "instance-1", Sequence: 5, Payload: TimerFired{}},
		{InstanceID: "instance-1", Sequence: 6, Payload: StepCompleted{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateCompleted")
	}
	if *result.FinalState != StateCompleted {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateCompleted)
	}
}

func TestReplayEngine_FailedThenResumed(t *testing.T) {
	engine := NewEngine()
	events := []*EventEnvelope{
		{InstanceID: "instance-1", Sequence: 1, Payload: WorkflowStarted{}},
		{InstanceID: "instance-1", Sequence: 2, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 3, Payload: StepFailed{}},
		{InstanceID: "instance-1", Sequence: 4, Payload: InstanceResumed{}},
		{InstanceID: "instance-1", Sequence: 5, Payload: StepScheduled{}},
		{InstanceID: "instance-1", Sequence: 6, Payload: StepStarted{}},
		{InstanceID: "instance-1", Sequence: 7, Payload: StepCompleted{}},
	}

	result, err := engine.Replay(events)
	if err != nil {
		t.Fatalf("Replay() error = %v, want nil", err)
	}
	if result.FinalState == nil {
		t.Fatal("FinalState = nil, want StateCompleted")
	}
	if *result.FinalState != StateCompleted {
		t.Errorf("FinalState = %s, want %s", *result.FinalState, StateCompleted)
	}
}

func TestPayloadToTransition(t *testing.T) {
	tests := []struct {
		payload   EventPayload
		wantEvent TransitionEvent
		wantErr   bool
	}{
		{WorkflowStarted{}, EventAssignToNode, false},
		{StepScheduled{}, EventStepScheduled, false},
		{StepStarted{}, EventExecuteStep, false},
		{StepCompleted{}, EventCompleteStep, false},
		{StepFailed{}, EventFail, false},
		{TimerSet{}, EventWaitForTimer, false},
		{TimerFired{}, EventTimerFired, false},
		{WorkflowCompleted{}, EventCompleteStep, false},
		{WorkflowFailed{}, EventFail, false},
		{WorkflowCancelled{}, EventCancel, false},
		{CancelRequested{}, EventCancel, false},
		{InstanceResumed{}, EventInstanceResumed, false},
		{ContinuedAsNew{}, "", true},
	}

	for _, tc := range tests {
		t.Run(fmt.Sprintf("%T", tc.payload), func(t *testing.T) {
			got, err := payloadToTransition(tc.payload, 1)
			if tc.wantErr {
				if err == nil {
					t.Errorf("payloadToTransition() error = nil, want error")
				}
				return
			}
			if err != nil {
				t.Errorf("payloadToTransition() error = %v, want nil", err)
				return
			}
			if got != tc.wantEvent {
				t.Errorf("payloadToTransition() = %v, want %v", got, tc.wantEvent)
			}
		})
	}
}
