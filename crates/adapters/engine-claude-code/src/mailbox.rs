//! Bounded cross-thread handoff to the document-owning thread. No document or bridge crosses a thread.
use crate::McpCall;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
struct State {
    queue: VecDeque<McpCall>,
    closed: bool,
    waker: Option<Waker>,
}
#[derive(Clone)]
pub struct Sender(Arc<Mutex<State>>);
pub struct Receiver(Arc<Mutex<State>>);
pub fn channel() -> (Sender, Receiver) {
    let state = Arc::new(Mutex::new(State {
        queue: VecDeque::new(),
        closed: false,
        waker: None,
    }));
    (Sender(state.clone()), Receiver(state))
}
impl Sender {
    pub fn send(&self, call: McpCall) -> Result<(), McpCall> {
        let mut state = self.0.lock().unwrap();
        if state.closed || state.queue.len() >= 32 {
            return Err(call);
        }
        state.queue.push_back(call);
        if let Some(w) = state.waker.take() {
            w.wake();
        }
        Ok(())
    }
    pub fn is_closed(&self) -> bool {
        self.0.lock().unwrap().closed
    }
}
impl Receiver {
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Option<McpCall>> {
        let mut state = self.0.lock().unwrap();
        if let Some(call) = state.queue.pop_front() {
            Poll::Ready(Some(call))
        } else if state.closed {
            Poll::Ready(None)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        let mut state = self.0.lock().unwrap();
        state.closed = true;
        state.queue.clear();
    }
}
