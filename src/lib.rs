//! A deliberately bounded, independently executing Frostbolt kernel.
//!
//! Inputs are prepared character snapshots, not arbitrary RaidSimRequests.
//! Character construction and static talent resolution belong to the oracle
//! exporter in this prototype. Unsupported fields and revisions fail closed.

mod rng;

use rng::{labeled_seed, SplitMix64};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const SOURCE_REVISION: &str = "6823b49eb8aff741f197ef36d83766ef6a218285";
const SECOND: u64 = 1_000_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub source_revision: String,
    pub scenario_id: String,
    pub iterations: u32,
    pub seed: u64,
    pub duration_ns: u64,
    pub reaction_ns: u64,
    pub caster: Caster,
    pub target: Target,
    pub spell: Spell,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Caster {
    pub level: u32,
    pub max_mana: f64,
    pub spell_power: f64,
    pub hit_percent: f64,
    pub crit_percent: f64,
    pub spell_penetration: f64,
    pub regen_casting_per_second: f64,
    pub regen_idle_per_second: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub level: u32,
    pub frost_resistance: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    pub id: u32,
    pub min_damage: f64,
    pub max_damage: f64,
    pub coefficient: f64,
    pub damage_multiplier: f64,
    pub crit_multiplier: f64,
    pub mana_cost: f64,
    pub cast_ns: u64,
    pub gcd_ns: u64,
    pub travel_ns: u64,
}

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.source_revision != SOURCE_REVISION {
            return Err("unsupported schema or source revision".into());
        }
        if self.spell.id != 25304
            || self.caster.level != 60
            || !(60..=63).contains(&self.target.level)
        {
            return Err(
                "only level 60 Frostbolt (25304) against level 60 to 63 targets is supported"
                    .into(),
            );
        }
        if self.scenario_id.is_empty() || self.scenario_id.len() > 200 {
            return Err("scenario_id must contain 1 to 200 bytes".into());
        }
        if !(1..=1_000_000).contains(&self.iterations)
            || self.seed == 0
            || self.seed > i64::MAX as u64 - u64::from(self.iterations)
            || !(SECOND..=600 * SECOND).contains(&self.duration_ns)
            || !(10_000_000..=SECOND).contains(&self.reaction_ns)
            || !(SECOND..=10 * SECOND).contains(&self.spell.cast_ns)
            || !(SECOND..=10 * SECOND).contains(&self.spell.gcd_ns)
            || self.spell.travel_ns > 10 * SECOND
        {
            return Err("iterations, seed, duration, reaction or spell timing is outside the prototype limits".into());
        }
        for (name, value) in [
            ("max_mana", self.caster.max_mana),
            ("spell_power", self.caster.spell_power),
            ("hit_percent", self.caster.hit_percent),
            ("crit_percent", self.caster.crit_percent),
            ("spell_penetration", self.caster.spell_penetration),
            (
                "regen_casting_per_second",
                self.caster.regen_casting_per_second,
            ),
            ("regen_idle_per_second", self.caster.regen_idle_per_second),
            ("frost_resistance", self.target.frost_resistance),
            ("min_damage", self.spell.min_damage),
            ("max_damage", self.spell.max_damage),
            ("coefficient", self.spell.coefficient),
            ("damage_multiplier", self.spell.damage_multiplier),
            ("crit_multiplier", self.spell.crit_multiplier),
            ("mana_cost", self.spell.mana_cost),
        ] {
            if !value.is_finite() || !(0.0..=1_000_000.0).contains(&value) {
                return Err(format!(
                    "{name} must be finite, nonnegative and at most 1000000"
                ));
            }
        }
        if self.caster.max_mana == 0.0
            || self.spell.mana_cost > self.caster.max_mana
            || self.caster.crit_percent > 100.0
            || self.caster.hit_percent > 100.0
            || self.spell.min_damage > self.spell.max_damage
            || self.spell.crit_multiplier < 1.0
            || self.spell.damage_multiplier == 0.0
        {
            return Err("inconsistent mana, damage or chance parameters".into());
        }
        Ok(())
    }
}

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

#[derive(Clone, Copy, Debug)]
enum Kind {
    ManaTick,
    Complete,
    Ready,
    Impact { damage: f64, outcome: Outcome },
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Hit,
    Crit,
    Miss,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    time: u64,
    priority: i8,
    sequence: u64,
    kind: Kind,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        (self.time, self.priority, self.sequence) == (other.time, other.priority, other.sequence)
    }
}
impl Eq for Event {}
impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .time
            .cmp(&self.time)
            .then(self.priority.cmp(&other.priority))
            .then(other.sequence.cmp(&self.sequence))
    }
}

fn enqueue(queue: &mut BinaryHeap<Event>, sequence: &mut u64, time: u64, priority: i8, kind: Kind) {
    queue.push(Event {
        time,
        priority,
        sequence: *sequence,
        kind,
    });
    *sequence += 1;
}

/// Frostbolt is binary: resistance reduces its hit chance rather than damage.
pub fn hit_chance(request: &Request) -> f64 {
    let base_miss = match request.target.level {
        60 => 0.04,
        61 => 0.05,
        62 => 0.06,
        _ => 0.17,
    };
    let resistance = (request.target.frost_resistance - request.caster.spell_penetration).max(0.0);
    let coefficient = (resistance / (5.0 * f64::from(request.caster.level))).min(1.0);
    let base_hit = (1.0 - base_miss) * (1.0 - 0.75 * coefficient);
    (base_hit + request.caster.hit_percent / 100.0).min(0.99)
}

pub fn simulate(request: &Request, capture_trace: bool) -> Result<Report, String> {
    request.validate()?;
    let started = std::time::Instant::now();
    let mut mean = 0.0;
    let mut m2 = 0.0;
    let mut mana_mean = 0.0;
    let mut counts = Counts::default();
    let mut work = Work::default();
    let mut trace = Vec::new();
    let mut queue = BinaryHeap::with_capacity(8);
    let hit_chance = hit_chance(request);
    for iteration in 0..request.iterations {
        let seed = request.seed + u64::from(iteration);
        let mut damage_rng = SplitMix64::new(labeled_seed(seed, "Damage Roll"));
        let mut hit_rng = SplitMix64::new(labeled_seed(seed, "Magical Hit Roll"));
        let mut crit_rng = SplitMix64::new(labeled_seed(seed, "Magical Crit Roll"));
        let mut mana = request.caster.max_mana;
        let mut five_second_rule = 0;
        let mut total_damage = 0.0;
        let mut sequence = 0;
        queue.clear();
        enqueue(&mut queue, &mut sequence, 0, 0, Kind::Ready);
        enqueue(&mut queue, &mut sequence, 2 * SECOND, 1, Kind::ManaTick);
        while let Some(event) = queue.pop() {
            if event.time > request.duration_ns {
                break;
            }
            let (name, damage) = match event.kind {
                Kind::ManaTick => {
                    work.mana_ticks += 1;
                    let regen = if event.time < five_second_rule {
                        request.caster.regen_casting_per_second
                    } else {
                        request.caster.regen_idle_per_second
                    };
                    mana = (mana + 2.0 * regen).min(request.caster.max_mana);
                    enqueue(
                        &mut queue,
                        &mut sequence,
                        event.time + 2 * SECOND,
                        1,
                        Kind::ManaTick,
                    );
                    ("mana_tick", 0.0)
                }
                Kind::Ready => {
                    work.ready_checks += 1;
                    if mana >= request.spell.mana_cost {
                        enqueue(
                            &mut queue,
                            &mut sequence,
                            event.time + request.spell.cast_ns,
                            0,
                            Kind::Complete,
                        );
                        // Complete schedules Ready when the hardcast also releases the GCD.
                        if request.spell.gcd_ns > request.spell.cast_ns {
                            enqueue(
                                &mut queue,
                                &mut sequence,
                                event.time + request.spell.gcd_ns,
                                0,
                                Kind::Ready,
                            );
                        }
                        ("cast_start", 0.0)
                    } else {
                        // Only mana ticks can make this static rotation ready.
                        // Skip failed polls, but keep the Go reaction-time grid.
                        let next_tick = (event.time / (2 * SECOND) + 1) * 2 * SECOND;
                        let polls = (next_tick - event.time).div_ceil(request.reaction_ns);
                        enqueue(
                            &mut queue,
                            &mut sequence,
                            event.time + polls * request.reaction_ns,
                            0,
                            Kind::Ready,
                        );
                        ("wait_for_mana", 0.0)
                    }
                }
                Kind::Complete => {
                    work.cast_completions += 1;
                    mana -= request.spell.mana_cost;
                    if request.spell.mana_cost > 0.0 {
                        five_second_rule = event.time + 5 * SECOND;
                    }
                    counts.casts += 1;
                    let base = if request.spell.max_damage > request.spell.min_damage {
                        work.damage_rolls += 1;
                        request.spell.min_damage
                            + (request.spell.max_damage - request.spell.min_damage)
                                * damage_rng.next_f64()
                    } else {
                        request.spell.min_damage
                    };
                    let mut damage = (base
                        + request.spell.coefficient * request.caster.spell_power)
                        * request.spell.damage_multiplier;
                    work.hit_rolls += 1;
                    let outcome = if hit_rng.next_f64() >= hit_chance {
                        damage = 0.0;
                        Outcome::Miss
                    } else {
                        work.crit_rolls += 1;
                        if crit_rng.next_f64() < request.caster.crit_percent / 100.0 {
                            damage *= request.spell.crit_multiplier;
                            Outcome::Crit
                        } else {
                            Outcome::Hit
                        }
                    };
                    // Go records the outcome when CalcDamage runs at cast
                    // completion. Damage is recorded only on missile arrival.
                    match outcome {
                        Outcome::Hit => counts.hits += 1,
                        Outcome::Crit => counts.crits += 1,
                        Outcome::Miss => counts.misses += 1,
                    }
                    enqueue(
                        &mut queue,
                        &mut sequence,
                        event.time + request.spell.travel_ns,
                        -1,
                        Kind::Impact { damage, outcome },
                    );
                    if request.spell.cast_ns >= request.spell.gcd_ns {
                        enqueue(&mut queue, &mut sequence, event.time, 0, Kind::Ready);
                    }
                    ("cast_complete", 0.0)
                }
                Kind::Impact { damage, outcome } => {
                    work.impacts += 1;
                    total_damage += damage;
                    (
                        match outcome {
                            Outcome::Hit => "hit",
                            Outcome::Crit => "crit",
                            Outcome::Miss => "miss",
                        },
                        damage,
                    )
                }
            };
            if capture_trace && iteration == 0 {
                trace.push(TraceEvent {
                    time_ns: event.time,
                    event: name.into(),
                    mana,
                    damage,
                });
            }
        }
        let dps = total_damage / (request.duration_ns as f64 / SECOND as f64);
        let n = f64::from(iteration + 1);
        let delta = dps - mean;
        mean += delta / n;
        m2 += delta * (dps - mean);
        mana_mean += (mana - mana_mean) / n;
    }
    let stdev = (m2 / f64::from(request.iterations)).sqrt();
    let elapsed_ns = started.elapsed().as_nanos() as u64;
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
