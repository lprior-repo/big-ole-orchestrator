#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use std::rc::Rc;
use std::cell::RefCell;

use dioxus::prelude::*;

use crate::ui::graph::Workflow;

#[derive(Clone)]
pub struct WorkflowState {
    workflow: Rc<RefCell<Signal<Workflow>>>,
    undo_stack: Rc<RefCell<Vec<Workflow>>>,
    redo_stack: Rc<RefCell<Vec<Workflow>>>,
}

impl WorkflowState {
    pub fn new(initial: Workflow) -> Self {
        Self {
            workflow: Rc::new(RefCell::new(Signal::new(initial))),
            undo_stack: Rc::new(RefCell::new(Vec::new())),
            redo_stack: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn workflow(&self) -> Signal<Workflow> {
        self.workflow.borrow().clone()
    }

    pub fn save_undo_point(&self) {
        let current = self.workflow.borrow().read().clone();
        self.undo_stack.borrow_mut().push(current);
        self.redo_stack.borrow_mut().clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.borrow().is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.borrow().is_empty()
    }

    pub fn undo(&self) -> bool {
        if let Some(state) = self.undo_stack.borrow_mut().pop() {
            let current = self.workflow.borrow().read().clone();
            self.redo_stack.borrow_mut().push(current);
            *self.workflow.borrow_mut().write() = state;
            true
        } else {
            false
        }
    }

    pub fn redo(&self) -> bool {
        if let Some(state) = self.redo_stack.borrow_mut().pop() {
            let current = self.workflow.borrow().read().clone();
            self.undo_stack.borrow_mut().push(current);
            *self.workflow.borrow_mut().write() = state;
            true
        } else {
            false
        }
    }
}

pub fn use_workflow_state(initial: Workflow) -> (WorkflowState, Signal<Workflow>) {
    let state = WorkflowState::new(initial);
    let workflow = state.workflow();
    (state, workflow)
}