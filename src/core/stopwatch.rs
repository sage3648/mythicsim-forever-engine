//! Wall-clock timing of a run, for a report's `elapsed_ns`. `wasm32-unknown-unknown` has no
//! clock and std's `Instant::now` panics there, so a run on it reports zero and the host that
//! runs the module times it instead. Every other target, WASI included, reads the real clock.

/// Started when a run begins; reads the nanoseconds since.
pub(crate) struct Stopwatch {
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    started: std::time::Instant,
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
impl Stopwatch {
    pub(crate) fn start() -> Self {
        Stopwatch {
            started: std::time::Instant::now(),
        }
    }

    pub(crate) fn elapsed_ns(&self) -> u64 {
        self.started.elapsed().as_nanos() as u64
    }
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
impl Stopwatch {
    pub(crate) fn start() -> Self {
        Stopwatch {}
    }

    pub(crate) fn elapsed_ns(&self) -> u64 {
        0
    }
}
