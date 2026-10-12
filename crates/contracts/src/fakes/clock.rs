use std::sync::atomic::{AtomicI64, Ordering};

use vellum_core::ports::{Clock, IdGen, Timestamp};
use vellum_types::NodeId;

/// A clock that only moves when told to.
#[derive(Debug)]
pub struct ManualClock(AtomicI64);

impl ManualClock {
    /// A clock showing `now` milliseconds since the epoch.
    pub fn new(now: i64) -> Self {
        Self(AtomicI64::new(now))
    }

    /// Moves the clock forward by `ms` (backward if negative; the suite does not).
    pub fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.load(Ordering::SeqCst))
    }
}

/// Ids `id00000001`, `id00000002`, …: ten characters each, the same sequence every run.
#[derive(Debug, Default)]
pub struct SequentialIds {
    next: u64,
}

impl SequentialIds {
    /// Starts at `id00000001`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdGen for SequentialIds {
    fn node_id(&mut self) -> NodeId {
        self.next += 1;
        NodeId::new(format!("id{:08}", self.next)).expect("a valid id")
    }
}
