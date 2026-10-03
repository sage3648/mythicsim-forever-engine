//! Prototype report types and metrics. This is not the production report adapter.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub casts: u64,
    pub hits: u64,
    pub crits: u64,
    pub misses: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Report {
    pub engine: String,
    pub source_revision: String,
    pub scenario_id: String,
    pub iterations: u32,
    pub seed: u64,
    pub dps_mean: f64,
    pub dps_stdev: f64,
    pub dps_standard_error: f64,
    pub counts: Counts,
    #[serde(default)]
    pub work: Work,
    pub mana_end_mean: f64,
    pub mana_delta_mean: f64,
    pub elapsed_ns: u64,
    pub trace: Vec<TraceEvent>,
}

/// Observable work counters keep matched-language benchmarks honest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Work {
    pub mana_ticks: u64,
    pub ready_checks: u64,
    pub cast_completions: u64,
    pub impacts: u64,
    pub damage_rolls: u64,
    pub hit_rolls: u64,
    pub crit_rolls: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceEvent {
    pub time_ns: u64,
    pub event: String,
    pub mana: f64,
    pub damage: f64,
}
