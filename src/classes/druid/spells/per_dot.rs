//! A value Go keeps on each instance of a dot, one for each target the dot can be on, such as
//! the amount a bleed snapshots when it is applied.

use crate::core::fight::DotId;

#[derive(Clone, Debug, Default)]
pub(crate) struct PerDot<T>(Vec<T>);

impl<T: Copy + Default> PerDot<T> {
    /// The value stored for the dot, which Go's dot holds from its last application.
    pub(crate) fn get(&self, dot: DotId) -> T {
        self.0.get(dot).copied().unwrap_or_default()
    }

    pub(crate) fn set(&mut self, dot: DotId, value: T) {
        if self.0.len() <= dot {
            self.0.resize(dot + 1, T::default());
        }
        self.0[dot] = value;
    }
}
