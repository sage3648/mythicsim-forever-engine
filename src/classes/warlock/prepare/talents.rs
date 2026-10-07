//! Go sim/warlock/talents.go: the Warlock's talents, registered in Go's order.

mod affliction;
mod demonology;
mod destruction;

use crate::prepare::sim::{Duration, MILLISECOND};

/// `time.Millisecond * time.Duration(value)`: the float is cut to whole milliseconds.
pub(super) fn millis(value: f64) -> Duration {
    (value as i64).wrapping_mul(MILLISECOND)
}
