//! Engine entry point: validation, iteration lifecycle and aggregate results.
//! Class-specific fight decisions are delegated to their spec domain.

pub(crate) mod coverage;
pub(crate) mod prepared;
mod validation;

use std::collections::BinaryHeap;

use crate::{
    classes::mage::{specs::frost, spells::frostbolt},
    contracts::Request,
    core::time::SECOND,
    report::{Counts, Report, Work},
    SOURCE_REVISION,
};

pub fn simulate(request: &Request, capture_trace: bool) -> Result<Report, String> {
    request.validate()?;
    let started = crate::core::stopwatch::Stopwatch::start();
    let mut mean = 0.0;
    let mut m2 = 0.0;
    let mut mana_mean = 0.0;
    let mut counts = Counts::default();
    let mut work = Work::default();
    let mut trace = Vec::new();
    let mut queue = BinaryHeap::with_capacity(8);
    let hit_chance = frostbolt::hit_chance(request);
    for iteration in 0..request.iterations {
        let seed = request.seed + u64::from(iteration);
        let iteration_trace = if capture_trace && iteration == 0 {
            Some(&mut trace)
        } else {
            None
        };
        let result = frost::run_iteration(
            request,
            seed,
            hit_chance,
            &mut queue,
            &mut counts,
            &mut work,
            iteration_trace,
        );
        let dps = result.damage / (request.duration_ns as f64 / SECOND as f64);
        let n = f64::from(iteration + 1);
        let delta = dps - mean;
        mean += delta / n;
        m2 += delta * (dps - mean);
        mana_mean += (result.mana_end - mana_mean) / n;
    }
    let stdev = (m2 / f64::from(request.iterations)).sqrt();
    let elapsed_ns = started.elapsed_ns();
    Ok(Report {
        engine: format!("forever-rust-prototype-{}", env!("CARGO_PKG_VERSION")),
        source_revision: SOURCE_REVISION.into(),
        scenario_id: request.scenario_id.clone(),
        iterations: request.iterations,
        seed: request.seed,
        dps_mean: mean,
        dps_stdev: stdev,
        dps_standard_error: stdev / f64::from(request.iterations).sqrt(),
        counts,
        work,
        mana_end_mean: mana_mean,
        mana_delta_mean: mana_mean - request.caster.max_mana,
        elapsed_ns,
        trace,
    })
}
