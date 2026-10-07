//! Go sim/core/periodic_action.go: the options a periodic action is started with.
//!
//! Go's pending actions live in the simulation's event queue. Preparation runs one reset and no
//! fight, and nothing in the prepared state records a pending action, so starting one has no
//! effect here: the options are kept so callers port Go's calls one for one.

use super::sim::{Duration, Sim};

/// Go `PeriodicActionOptions`, without the callbacks that only run in a fight.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PeriodicActionOptions {
    /// How often the action should be performed.
    pub period: Duration,
    /// Number of times to perform the action before stopping. 0 is a permanent action.
    pub num_ticks: i32,
    /// Whether the first tick should happen immediately.
    pub tick_immediately: bool,
}

impl Sim {
    /// Go `StartPeriodicAction`: nothing observable in preparation.
    pub(crate) fn start_periodic_action(&mut self, _options: PeriodicActionOptions) {}
}
