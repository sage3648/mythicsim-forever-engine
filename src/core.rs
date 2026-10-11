//! Class-independent scheduling, simulation time, deterministic random streams and the
//! fight runtime that mirrors Go's sim/core. Nothing here names a class.

pub(crate) mod events;
pub(crate) mod fight;
pub(crate) mod queue;
pub(crate) mod rng;
pub(crate) mod stopwatch;
pub(crate) mod time;
