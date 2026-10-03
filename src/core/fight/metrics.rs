//! Go metrics_aggregator.go and the `RaidSimResult` subset Rust reports.
//!
//! Field names follow Go's protojson output and zero values are omitted, as protojson
//! does by default, so Rust and Go results can be compared field by field.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    contracts::prepared_v2::ActionId,
    core::time::{milliseconds, seconds, NS_PER_SECOND},
};

use super::{Agent, Fight, Side};

/// Go `aggregator`: count, sum and sum of squares.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Aggregator {
    n: u32,
    sum: f64,
    sum_sq: f64,
}

impl Aggregator {
    pub(crate) fn add(&mut self, value: f64) {
        self.n += 1;
        self.sum += value;
        self.sum_sq += value * value;
    }

    /// Go `meanAndStdDev`.
    fn mean_and_stdev(&self) -> (f64, f64) {
        let mean = self.sum / f64::from(self.n);
        (mean, (self.sum_sq / f64::from(self.n) - mean * mean).sqrt())
    }
}

/// Go `DistributionMetrics`.
#[derive(Clone, Debug)]
pub(crate) struct Distribution {
    pub(crate) total: f64,
    aggregate: Aggregator,
    max: f64,
    min: f64,
    max_seed: i64,
    min_seed: i64,
    hist: BTreeMap<i32, i32>,
}

impl Default for Distribution {
    fn default() -> Self {
        Self {
            total: 0.0,
            aggregate: Aggregator::default(),
            max: 0.0,
            min: -1.0,
            max_seed: 0,
            min_seed: 0,
            hist: BTreeMap::new(),
        }
    }
}

impl Distribution {
    /// Go `DistributionMetrics.doneIteration`.
    fn done_iteration(&mut self, duration: i64, seed: i64) {
        let value = self.total / seconds(duration);
        self.aggregate.add(value);
        if value > self.max {
            self.max = value;
            self.max_seed = seed;
        }
        if value <= self.min || self.min < 0.0 {
            self.min = value;
            self.min_seed = seed;
        }
        let rounded = ((value / 10.0).round() * 10.0) as i32;
        *self.hist.entry(rounded).or_default() += 1;
        self.total = 0.0;
    }

    fn report(&self) -> DistributionReport {
        let (avg, stdev) = self.aggregate.mean_and_stdev();
        DistributionReport {
            avg,
            stdev,
            max: self.max,
            min: self.min,
            max_seed: self.max_seed.to_string(),
            min_seed: self.min_seed.to_string(),
            hist: self.hist.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            aggregator_data: AggregatorData {
                n: self.aggregate.n,
                sum_sq: self.aggregate.sum_sq,
            },
        }
    }
}

/// Per-unit aggregates across iterations.
#[derive(Default)]
pub(crate) struct Totals {
    pub(crate) iteration_damage: f64,
    pub(crate) dps: Distribution,
    pub(crate) threat: Distribution,
    pub(crate) tto: Distribution,
    pub(crate) target_dtps: Distribution,
    /// Damage the player takes from its own spells.
    pub(crate) player_dtps: Distribution,
    /// Every distribution Go keeps that stays zero in scope: healing, damage taken by the
    /// player, TMI, and the target's own output.
    pub(crate) zero: Distribution,
    pub(crate) oom_seconds: f64,
    pub(crate) iterations: u32,
}

fn is_zero_f(value: &f64) -> bool {
    *value == 0.0
}

/// A deviation as protojson writes it: a number, or "NaN" when cancellation left
/// `sqrt(sumSq/n - mean^2)` a negative residue, as Go reports it.
fn deviation<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if value.is_nan() {
        serializer.serialize_str("NaN")
    } else {
        serializer.serialize_f64(*value)
    }
}

fn is_zero_i(value: &i32) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_zero_string(value: &str) -> bool {
    value == "0"
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionIdReport {
    #[serde(skip_serializing_if = "is_zero_i")]
    spell_id: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    item_id: i32,
    #[serde(skip_serializing_if = "String::is_empty")]
    other_id: String,
    #[serde(skip_serializing_if = "is_zero_i")]
    tag: i32,
}

impl From<&ActionId> for ActionIdReport {
    fn from(id: &ActionId) -> Self {
        ActionIdReport {
            spell_id: id.spell_id,
            item_id: id.item_id,
            other_id: id.other_id.clone(),
            tag: id.tag,
        }
    }
}

/// Go `TargetedActionMetrics`, accumulated over every iteration.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionReport {
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) unit_index: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) casts: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) hits: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) resisted_hits: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) crits: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) resisted_crits: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) ticks: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) resisted_ticks: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) crit_ticks: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) resisted_crit_ticks: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) misses: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) dodges: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) parries: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) blocks: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) blocked_crits: i32,
    #[serde(skip_serializing_if = "is_zero_i")]
    pub(crate) glances: i32,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) resisted_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) crit_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) resisted_crit_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) tick_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) resisted_tick_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) crit_tick_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) resisted_crit_tick_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) glance_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) block_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) blocked_crit_damage: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) threat: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub(crate) cast_time_ms: f64,
    #[serde(skip)]
    cast_time: i64,
}

impl ActionReport {
    pub(crate) fn new(unit_index: i32) -> Self {
        ActionReport {
            unit_index,
            ..ActionReport::default()
        }
    }
}

/// Go `ActionMetrics`: one entry per action ID, shared by spells with that ID.
#[derive(Clone, Debug)]
pub(crate) struct ActionTotals {
    pub(crate) id: ActionId,
    pub(crate) melee: bool,
    pub(crate) passive: bool,
    pub(crate) school: u8,
    pub(crate) targets: [ActionReport; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionMetricsReport {
    id: ActionIdReport,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_melee: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_passive: bool,
    targets: Vec<ActionReport>,
    #[serde(skip_serializing_if = "is_zero_u8")]
    spell_school: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AggregatorData {
    #[serde(skip_serializing_if = "is_zero_u32")]
    n: u32,
    #[serde(skip_serializing_if = "is_zero_f")]
    sum_sq: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuraMetricsReport {
    id: ActionIdReport,
    #[serde(skip_serializing_if = "is_zero_f")]
    uptime_seconds_avg: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    #[serde(serialize_with = "deviation")]
    uptime_seconds_stdev: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    procs_avg: f64,
    aggregator_data: AggregatorData,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceMetricsReport {
    id: ActionIdReport,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "is_zero_i")]
    events: i32,
    #[serde(skip_serializing_if = "is_zero_f")]
    gain: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    actual_gain: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DistributionReport {
    #[serde(skip_serializing_if = "is_zero_f")]
    avg: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    #[serde(serialize_with = "deviation")]
    stdev: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    max: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    min: f64,
    #[serde(skip_serializing_if = "is_zero_string")]
    max_seed: String,
    #[serde(skip_serializing_if = "is_zero_string")]
    min_seed: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    hist: BTreeMap<String, i32>,
    aggregator_data: AggregatorData,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnitReport {
    name: String,
    #[serde(skip_serializing_if = "is_zero_i")]
    unit_index: i32,
    dps: DistributionReport,
    threat: DistributionReport,
    dtps: DistributionReport,
    tmi: DistributionReport,
    hps: DistributionReport,
    tto: DistributionReport,
    #[serde(skip_serializing_if = "is_zero_f")]
    seconds_oom_avg: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    chance_of_death: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    death_seeds: Vec<String>,
    actions: Vec<ActionMetricsReport>,
    auras: Vec<AuraMetricsReport>,
    resources: Vec<ResourceMetricsReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pets: Vec<TargetReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetReport {
    name: String,
    #[serde(skip_serializing_if = "is_zero_i")]
    unit_index: i32,
    dps: DistributionReport,
    threat: DistributionReport,
    dtps: DistributionReport,
    tmi: DistributionReport,
    hps: DistributionReport,
    tto: DistributionReport,
    actions: Vec<ActionMetricsReport>,
    auras: Vec<AuraMetricsReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartyReport {
    dps: DistributionReport,
    hps: DistributionReport,
    players: Vec<UnitReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RaidReport {
    dps: DistributionReport,
    hps: DistributionReport,
    parties: Vec<PartyReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EncounterReport {
    targets: Vec<TargetReport>,
}

/// Go `RaidSimResult` for one player and one target.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FightReport {
    raid_metrics: RaidReport,
    encounter_metrics: EncounterReport,
    #[serde(skip_serializing_if = "String::is_empty")]
    logs: String,
    first_iteration_duration: f64,
    avg_iteration_duration: f64,
    iterations_done: u32,
    #[serde(skip)]
    pub(crate) elapsed_ns: u64,
}

impl<A: Agent> Fight<A> {
    fn iteration_seed(&self) -> i64 {
        self.config.seed + i64::from(self.totals.iterations)
    }

    /// Go `manaBar.doneIteration`: out-of-mana time and mana-gain threat.
    pub(crate) fn player_done_iteration(&mut self) {
        if self.player.waiting_for_mana != 0.0 {
            let duration = self.now - self.player.waiting_for_mana_start;
            if duration > 0 {
                self.player.oom_time += duration;
                if !self.player.went_oom {
                    self.player.went_oom = true;
                    self.player.first_oom = self.now;
                }
            }
        }
        let Some(gain_spell) = self.mana_gain_spell else {
            return;
        };
        for index in 0..self.resources.len() {
            let resource = &self.resources[index];
            if resource.kind != super::ResourceKind::Mana
                || resource.is_mana_regen
                || resource.id.other_id == "OtherActionManaRegen"
            {
                continue;
            }
            let actual = resource.actual_gain - resource.previous_actual_gain;
            if actual <= 0.0 {
                continue;
            }
            let events = resource.events - resource.previous_events;
            self.spells[gain_spell].metrics[0].casts += events;
            self.spells[gain_spell].metrics[Side::Target.index()].total_threat += actual * 0.5;
        }
    }

    /// Go `Spell.doneIteration` and `UnitMetrics.addSpellMetrics`.
    pub(crate) fn spell_done_iteration(&mut self, spell: usize) {
        if self.spells[spell].action.is_none() {
            return;
        }
        if self.spells[spell].split_metrics.is_empty() {
            let metrics = self.spells[spell].metrics;
            let action = self.spells[spell].action.expect("checked above");
            self.add_spell_metrics(spell, action, metrics);
            return;
        }
        // Go Spell.doneIteration: each split under its own tagged ID.
        let state = &mut self.spells[spell];
        state.split_metrics[state.split] = state.metrics;
        for split in 0..self.spells[spell].split_metrics.len() {
            let metrics = self.spells[spell].split_metrics[split];
            let action = self.spells[spell].split_actions[split];
            self.add_spell_metrics(spell, action, metrics);
        }
    }

    /// Go `UnitMetrics.addSpellMetrics`.
    fn add_spell_metrics(&mut self, spell: usize, action: usize, all: [super::SpellMetrics; 2]) {
        let passive = self.spells[spell].flags.passive;
        for (target, metrics) in all.into_iter().enumerate() {
            let totals = &mut self.actions[action].targets[target];
            if !passive {
                totals.casts += metrics.casts;
            }
            totals.misses += metrics.misses;
            totals.dodges += metrics.dodges;
            totals.parries += metrics.parries;
            totals.blocks += metrics.blocks;
            totals.blocked_crits += metrics.blocked_crits;
            totals.glances += metrics.glances;
            totals.hits += metrics.hits;
            totals.resisted_hits += metrics.resisted_hits;
            totals.crits += metrics.crits;
            totals.resisted_crits += metrics.resisted_crits;
            totals.ticks += metrics.ticks;
            totals.resisted_ticks += metrics.resisted_ticks;
            totals.crit_ticks += metrics.crit_ticks;
            totals.resisted_crit_ticks += metrics.resisted_crit_ticks;
            totals.damage += metrics.total_damage;
            totals.resisted_damage += metrics.total_resisted_damage;
            totals.crit_damage += metrics.total_crit_damage;
            totals.resisted_crit_damage += metrics.total_resisted_crit_damage;
            totals.tick_damage += metrics.total_tick_damage;
            totals.resisted_tick_damage += metrics.total_resisted_tick_damage;
            totals.crit_tick_damage += metrics.total_crit_tick_damage;
            totals.resisted_crit_tick_damage += metrics.total_resisted_crit_tick_damage;
            totals.glance_damage += metrics.total_glance_damage;
            totals.block_damage += metrics.total_block_damage;
            totals.blocked_crit_damage += metrics.total_blocked_crit_damage;
            totals.threat += metrics.total_threat;
            if !passive {
                totals.cast_time += metrics.total_cast_time;
            }
            if target == Side::Player.index() {
                // Go adds damage on any unit to that unit's damage taken; the player is no
                // opponent of its own, so its healing would go to healing done.
                self.totals.player_dtps.total += metrics.total_damage;
            }
            if target == Side::Target.index() {
                self.totals.target_dtps.total += metrics.total_damage;
                self.totals.dps.total += metrics.total_damage;
                self.totals.threat.total += metrics.total_threat;
            }
        }
    }

    /// Go `UnitMetrics.doneIteration` for the player and the target.
    pub(crate) fn unit_done_iteration(&mut self, _damage: f64) {
        let seed = self.iteration_seed();
        let duration_seconds = seconds(self.duration);
        let time_to_oom = if self.player.went_oom {
            self.player.first_oom
        } else {
            let spent_per_second =
                (self.player.mana_spent - self.player.mana_gained) / duration_seconds;
            if spent_per_second > 0.0 {
                // Go's aura teardown clamps current mana to the falling maximum first.
                let mana = self.player.mana.min(self.config.teardown_max_mana);
                let remaining = crate::core::time::from_seconds(mana / spent_per_second);
                // Go adds durations with int64 wraparound; an overflow turns negative and
                // falls back to 60 minutes below.
                crate::core::time::from_seconds(duration_seconds)
                    .wrapping_add(remaining)
                    .min(60 * 60 * NS_PER_SECOND)
            } else {
                60 * 60 * NS_PER_SECOND
            }
        };
        let time_to_oom = if time_to_oom < 0 {
            60 * 60 * NS_PER_SECOND
        } else {
            time_to_oom
        };
        // Go infers time to out of mana only for a unit with a mana bar.
        self.totals.tto.total = if self.has_mana_bar() {
            seconds(time_to_oom) * duration_seconds
        } else {
            0.0
        };
        let duration = self.duration;
        self.totals.dps.done_iteration(duration, seed);
        self.totals.threat.done_iteration(duration, seed);
        self.totals.tto.done_iteration(duration, seed);
        self.totals.target_dtps.done_iteration(duration, seed);
        self.totals.player_dtps.done_iteration(duration, seed);
        if self.death.died {
            self.death.iterations_dead += 1;
            self.death.seeds.push(seed);
            self.death.seeds.sort_unstable();
        }
        self.totals.zero.done_iteration(duration, seed);
        self.totals.oom_seconds += seconds(self.player.oom_time);
        self.totals.iterations += 1;
    }

    fn aura_reports(&self, side: Side) -> Vec<AuraMetricsReport> {
        self.trackers[side.index()]
            .auras
            .iter()
            .filter_map(|aura| {
                let id = aura.action_id.as_ref()?;
                let n = aura.aggregate.n;
                let (avg, stdev) = aura.aggregate.mean_and_stdev();
                Some(AuraMetricsReport {
                    id: id.into(),
                    uptime_seconds_avg: avg,
                    uptime_seconds_stdev: stdev,
                    procs_avg: aura.procs_sum as f64 / f64::from(n),
                    aggregator_data: AggregatorData {
                        n,
                        sum_sq: aura.aggregate.sum_sq,
                    },
                })
            })
            .collect()
    }

    pub(crate) fn report(
        &self,
        first_duration: i64,
        total_duration: i64,
        logs: String,
        elapsed_ns: u64,
    ) -> FightReport {
        // Go lists every unit in each action's targets; pets never take a hit in scope.
        let extra_units = self.extra_unit_indexes();
        let action_report = |action: &ActionTotals| ActionMetricsReport {
            id: (&action.id).into(),
            is_melee: action.melee,
            is_passive: action.passive,
            targets: action
                .targets
                .iter()
                .cloned()
                .map(|mut target| {
                    target.cast_time_ms = milliseconds(target.cast_time) as f64;
                    target
                })
                .chain(extra_units.iter().map(|&unit| ActionReport::new(unit)))
                .collect(),
            spell_school: action.school,
        };
        let actions = self.actions.iter().map(action_report).collect();
        let resources = self
            .resources
            .iter()
            .filter(|resource| resource.events > 0)
            .map(|resource| ResourceMetricsReport {
                id: (&resource.id).into(),
                kind: resource.kind.proto_name(),
                events: resource.events,
                gain: resource.gain,
                actual_gain: resource.actual_gain,
            })
            .collect();
        let n = f64::from(self.totals.iterations);
        let zero = self.totals.zero.report();
        let player = UnitReport {
            name: self.config.player_name.clone(),
            unit_index: Side::Player.index() as i32,
            dps: self.totals.dps.report(),
            threat: self.totals.threat.report(),
            dtps: self.totals.player_dtps.report(),
            tmi: zero.clone(),
            hps: zero.clone(),
            tto: self.totals.tto.report(),
            seconds_oom_avg: self.totals.oom_seconds / n,
            chance_of_death: f64::from(self.death.iterations_dead) / n,
            death_seeds: self.death.seeds.iter().map(i64::to_string).collect(),
            actions,
            auras: self.aura_reports(Side::Player),
            resources,
            pets: self
                .pets
                .iter()
                .map(|pet| TargetReport {
                    name: pet.name.clone(),
                    unit_index: pet.unit_index,
                    dps: zero.clone(),
                    threat: zero.clone(),
                    dtps: zero.clone(),
                    tmi: zero.clone(),
                    hps: zero.clone(),
                    tto: zero.clone(),
                    actions: pet.actions.iter().map(action_report).collect(),
                    auras: pet
                        .auras
                        .iter()
                        .map(|id| AuraMetricsReport {
                            id: id.into(),
                            uptime_seconds_avg: 0.0,
                            uptime_seconds_stdev: 0.0,
                            procs_avg: 0.0,
                            aggregator_data: AggregatorData {
                                n: self.totals.iterations,
                                sum_sq: 0.0,
                            },
                        })
                        .collect(),
                })
                .collect(),
        };
        let target = TargetReport {
            name: self.config.target_label.clone(),
            unit_index: Side::Target.index() as i32,
            dps: zero.clone(),
            threat: zero.clone(),
            dtps: self.totals.target_dtps.report(),
            tmi: zero.clone(),
            hps: zero.clone(),
            tto: zero.clone(),
            actions: self.target_actions.iter().map(action_report).collect(),
            auras: self.aura_reports(Side::Target),
        };
        let dps = self.totals.dps.report();
        FightReport {
            raid_metrics: RaidReport {
                dps: dps.clone(),
                hps: zero.clone(),
                parties: vec![PartyReport {
                    dps,
                    hps: zero,
                    players: vec![player],
                }],
            },
            encounter_metrics: EncounterReport {
                targets: vec![target],
            },
            logs,
            first_iteration_duration: seconds(first_duration),
            avg_iteration_duration: seconds(total_duration) / n,
            iterations_done: self.totals.iterations,
            elapsed_ns,
        }
    }
}
