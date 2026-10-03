//! SplitMix64 and labeled streams ported from the pinned MIT wowsims engine.

pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut result = self.0;
        result = (result ^ (result >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        result = (result ^ (result >> 27)).wrapping_mul(0x94d049bb133111eb);
        result ^ (result >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    }
}

pub fn labeled_seed(seed: u64, label: &str) -> u64 {
    let text = format!("{label}{seed:x}");
    let hash = text.bytes().fold(2166136261u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    });
    u64::from(hash)
}

/// Go's simulation random source: one shared stream, or one stream per label when the
/// request sets `useLabeledRands`. Every iteration reseeds with `seed + iteration`.
pub(crate) enum SimRng {
    Shared(SplitMix64),
    Labeled {
        seed: u64,
        streams: Vec<(String, SplitMix64)>,
    },
}

impl SimRng {
    pub(crate) fn new(labeled: bool, seed: u64) -> Self {
        if labeled {
            SimRng::Labeled {
                seed,
                streams: Vec::new(),
            }
        } else {
            SimRng::Shared(SplitMix64::new(seed))
        }
    }

    /// Go `reseedRands`: existing label streams restart from the new iteration seed and
    /// later labels are created from it.
    pub(crate) fn reseed(&mut self, iteration_seed: u64) {
        match self {
            SimRng::Shared(rng) => *rng = SplitMix64::new(iteration_seed),
            SimRng::Labeled { seed, streams } => {
                *seed = iteration_seed;
                for (label, rng) in streams.iter_mut() {
                    *rng = SplitMix64::new(labeled_seed(iteration_seed, label));
                }
            }
        }
    }

    /// Go `RandomFloat(label)`.
    pub(crate) fn next_f64(&mut self, label: &str) -> f64 {
        match self {
            SimRng::Shared(rng) => rng.next_f64(),
            SimRng::Labeled { seed, streams } => {
                if let Some((_, rng)) = streams.iter_mut().find(|(name, _)| name == label) {
                    return rng.next_f64();
                }
                let mut rng = SplitMix64::new(labeled_seed(*seed, label));
                let value = rng.next_f64();
                streams.push((label.to_string(), rng));
                value
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_matches_published_reference_vector() {
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u64(), 0xe220a8397b1dcdaf);
        assert_eq!(rng.next_u64(), 0x6e789e6aa1b965f4);
        assert_eq!(rng.next_u64(), 0x06c45d188009454f);
    }

    #[test]
    fn labeled_streams_are_independent_and_reseed_per_iteration() {
        let mut shared = SimRng::new(false, 42);
        let mut labeled = SimRng::new(true, 42);
        let first_hit = labeled.next_f64("Magical Hit Roll");
        let _ = labeled.next_f64("Damage Roll");
        assert_eq!(
            first_hit,
            SplitMix64::new(labeled_seed(42, "Magical Hit Roll")).next_f64()
        );
        labeled.reseed(43);
        assert_eq!(
            labeled.next_f64("Magical Hit Roll"),
            SplitMix64::new(labeled_seed(43, "Magical Hit Roll")).next_f64()
        );
        let a = shared.next_f64("Damage Roll");
        let b = shared.next_f64("Magical Hit Roll");
        assert_eq!((a, b), {
            let mut rng = SplitMix64::new(42);
            (rng.next_f64(), rng.next_f64())
        });
    }
}
