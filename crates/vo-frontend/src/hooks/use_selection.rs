#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use std::rc::Rc;
use std::cell::RefCell;

use dioxus::prelude::*;

use crate::ui::graph::NodeId;

const DEBOUNCE_MS: u32 = 150;

#[derive(Clone)]
pub struct SelectionState {
    pending: Rc<RefCell<Option<NodeId>>>,
    pending_timeout: Rc<RefCell<Option<gloo_timers::callback::Timeout>>>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            pending: Rc::new(RefCell::new(None)),
            pending_timeout: Rc::new(RefCell::new(None)),
        }
    }

    pub fn select_single(&self, node_id: NodeId, committed: Signal<Option<NodeId>>) {
        *self.pending.borrow_mut() = Some(node_id);

        self.pending_timeout.borrow_mut().take();

        let pending = self.pending.clone();
        let mut committed = committed.clone();
        let pending_timeout = self.pending_timeout.clone();

        let timeout = gloo_timers::callback::Timeout::new(DEBOUNCE_MS, move || {
            let pending_val = pending.borrow().clone();
            if pending_val.is_some() {
                committed.set(pending_val);
            }
            pending_timeout.borrow_mut().take();
        });

        *self.pending_timeout.borrow_mut() = Some(timeout);
    }

    pub fn clear(&self, mut committed: Signal<Option<NodeId>>) {
        self.pending_timeout.borrow_mut().take();
        *self.pending.borrow_mut() = None;
        committed.set(None);
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn use_selection() -> (SelectionState, Signal<Option<NodeId>>) {
    let committed = use_signal(|| None::<NodeId>);
    let state = SelectionState::new();
    (state, committed)
}