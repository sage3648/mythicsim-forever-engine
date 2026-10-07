//! Go sim/rogue/talents_*.go: the Rogue's talents, registered by `Rogue.ApplyTalents` in Go's
//! order. A talent Go leaves unmodelled registers nothing here either.

mod assassination;
mod combat;
mod subtlety;

use crate::prepare::sim::{Duration, MILLISECOND};

/// `time.Duration(value) * time.Millisecond`: the float is cut to whole milliseconds first.
fn millis(value: f64) -> Duration {
    (value as i64).wrapping_mul(MILLISECOND)
}
