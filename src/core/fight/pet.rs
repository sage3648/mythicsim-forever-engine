//! Go pet.go: the player's registered pets.
//!
//! Go adds every registered pet to the environment and gives each a unit index after the
//! player's. Rust simulates each pet a reset enables or an effect summons, an [`ActivePet`]
//! acting as the unit [`Side::Pet`] of its position, in unit index order, with its own state,
//! configuration, auras, spells, swings, mana or focus and metrics; the shared runtime reaches
//! it through the caster of each spell. Several may be out at once, as a hunter pet or a demon
//! beside Dragon's Call's whelp: each enabled pet's aura tracker joins Go's tracker list when
//! it is enabled and leaves it by swap removal when disabled, and the resets, mana ticks,
//! focus tasks, encounter start, fight ends and metrics take the pets in unit index order. A
//! pet that is
//! never enabled is an [`InertPet`]: its swing timer reset only rolls an offset for enemies,
//! and only enabled units start the encounter, so no random number is drawn for it. Its
//! metrics report zero, its time to out of mana the hour Go uses for a unit that never spends,
//! and every action metric lists one more unit. Its permanent auras still activate at each
//! reset and fade at each fight's end, so they report a whole fight's uptime. A pet whose agent's Reset dismisses it logs
//! that at each reset; every inert pet logs that none is summoned at each fight's end.

use crate::{
    contracts::prepared_v2::{ActionId, Effect, Pet as ExportedPet, PreparedV2},
    core::fight::log::action_string,
    core::time::{seconds, NEVER_EXPIRES, NS_PER_SECOND, STARTING_CD_TIME},
};

use super::{
    melee,
    metrics::{Aggregator, Distribution},
    unit_config, Action, ActionTotals, Agent, BuildError, Config, Fight, Hardcast, Player, Powers,
    Side, SpellId, UnitSource, PRIORITY_DOT,
};

/// Go `PetUpdateInterval`, the period of the pets' stat inheritance heartbeat.
pub(crate) const PET_UPDATE_INTERVAL: i64 = 5_250 * crate::core::time::NS_PER_MILLISECOND;
/// [`PET_UPDATE_INTERVAL`] in seconds, as Go's `PetUpdateInterval.Seconds()`.
pub(crate) const PET_UPDATE_INTERVAL_SECONDS: f64 = 5.25;

/// The stats a unit's [`Powers`] hold, by the names Go's stats use.
const POWER_STATS: [&str; 5] = [
    "SpellDamage",
    "AttackPower",
    "RangedAttackPower",
    "SpellCritPercent",
    "PhysicalCritPercent",
];

fn power_mut(powers: &mut Powers, index: usize) -> &mut f64 {
    match index {
        0 => &mut powers.spell_damage,
        1 => &mut powers.attack_power,
        2 => &mut powers.ranged_attack_power,
        3 => &mut powers.spell_crit_percent,
        _ => &mut powers.physical_crit_percent,
    }
}

fn power(powers: &Powers, index: usize) -> f64 {
    let mut copy = *powers;
    *power_mut(&mut copy, index)
}

/// The position in [`Powers`] of a stat, if the runtime tracks it there.
pub(crate) fn power_index(stat: &str) -> Option<usize> {
    POWER_STATS.iter().position(|name| *name == stat)
}

/// Go stats.go `flooredGameStats`.
const FLOORED_STATS: [&str; 5] = ["Strength", "Agility", "Stamina", "Intellect", "Spirit"];

/// A dynamic pet's inheritance of its owner's stat changes: Go `pendingStatInheritance`,
/// whether `statInheritanceAction` is pending, and what Go recomputes the pet's stats from.
/// Each stat is a position in one list, the tracked powers first.
pub(crate) struct Inheritance {
    /// (owner power, pet stat, coefficient).
    terms: Vec<(usize, usize, f64)>,
    pending: Powers,
    scheduled: bool,
    /// Go `statsWithoutDeps` at the summon, and now.
    initial_without_deps: Vec<f64>,
    without_deps: Vec<f64>,
    /// Go `inheritedStats` at the summon, and now: what Disable takes away.
    initial_inherited: Vec<f64>,
    inherited: Vec<f64>,
    /// The dismissal line's stats in Go's order: the position of a stat the runtime changes
    /// during a fight, or the value Go logs with no change.
    dismiss: Vec<(String, Option<usize>, f64)>,
    /// What each permanent aura's expiry adds to the stats before dependencies, by label.
    aura_stats: Vec<(String, Vec<(usize, f64)>)>,
    /// (src, dst, amount, step) of each enabled dependency, in order.
    dependencies: Vec<(usize, usize, f64, f64)>,
    floored: Vec<bool>,
    /// The stat behind each of the pet's [`Powers`], if it has one.
    power_stats: [Option<usize>; 5],
}

impl Inheritance {
    fn new(exported: &ExportedPet) -> Self {
        let mut names: Vec<String> = POWER_STATS.iter().map(|name| name.to_string()).collect();
        let mut index = |name: &str| match names.iter().position(|known| known == name) {
            Some(index) => index,
            None => {
                names.push(name.to_string());
                names.len() - 1
            }
        };
        let terms = exported
            .inheritance
            .iter()
            .filter_map(|term| {
                Some((
                    power_index(&term.owner)?,
                    index(&term.pet),
                    term.coefficient,
                ))
            })
            .collect();
        let dependencies = exported
            .stat_dependencies
            .iter()
            .map(|dep| (index(&dep.src), index(&dep.dst), dep.amount, dep.step))
            .collect();
        let without: Vec<(usize, f64)> = exported
            .stats_without_deps
            .iter()
            .map(|(name, value)| (index(name), *value))
            .collect();
        let inherited: Vec<(usize, f64)> = exported
            .inherited_stats
            .iter()
            .map(|(name, value)| (index(name), *value))
            .collect();
        let dismiss: Vec<(String, usize, f64)> = exported
            .dismiss_stats
            .iter()
            .map(|stat| (stat.stat.clone(), index(&stat.stat), stat.value))
            .collect();
        let aura_stats: Vec<(String, Vec<(usize, f64)>)> = exported
            .aura_stats
            .iter()
            .map(|aura| {
                let stats = aura
                    .stats
                    .iter()
                    .map(|(name, value)| (index(name), *value))
                    .collect();
                (aura.aura.clone(), stats)
            })
            .collect();
        let mut initial_without_deps = vec![0.0; names.len()];
        for (position, value) in without {
            initial_without_deps[position] = value;
        }
        let mut initial_inherited = vec![0.0; names.len()];
        for (position, value) in inherited {
            initial_inherited[position] = value;
        }
        let floored = names
            .iter()
            .map(|name| FLOORED_STATS.contains(&name.as_str()))
            .collect();
        // The stats the runtime changes during a fight: the tracked powers, what they pass to
        // the pet, and the stats depending on those.
        let mut changing = vec![false; names.len()];
        changing[..POWER_STATS.len()].fill(true);
        for &(_, stat, _) in &terms {
            changing[stat] = true;
        }
        for &(src, dst, _, _) in &dependencies {
            if changing[src] {
                changing[dst] = true;
            }
        }
        let dismiss = dismiss
            .into_iter()
            .map(|(name, position, value)| (name, changing[position].then_some(position), value))
            .collect();
        Inheritance {
            terms,
            pending: Powers::default(),
            scheduled: false,
            without_deps: initial_without_deps.clone(),
            initial_without_deps,
            inherited: initial_inherited.clone(),
            initial_inherited,
            dismiss,
            aura_stats,
            dependencies,
            floored,
            power_stats: [Some(0), Some(1), Some(2), Some(3), Some(4)],
        }
    }

    /// Go `AttachStatsBuff`'s expiry of a permanent aura: its stats inverted join the stats
    /// before dependencies.
    fn aura_expired(&mut self, position: usize) {
        for &(stat, amount) in &self.aura_stats[position].1 {
            self.without_deps[stat] += amount;
        }
    }

    /// Go `resetInheritedOwnerStats`: `AddStatsDynamic` of the inherited stats inverted.
    fn remove_inherited(&mut self) {
        for (stat, inherited) in self.without_deps.iter_mut().zip(&self.inherited) {
            *stat += -inherited;
        }
    }

    /// Go `pet.GetStats().FlatString()` on dismissal, with the stats the fight changed taken
    /// from the runtime, float residue included; `None` without the exported stat list.
    fn dismiss_line(&self) -> Option<String> {
        if self.dismiss.is_empty() {
            return None;
        }
        let stats = self.stats();
        let mut line = String::from("{");
        for (name, position, value) in &self.dismiss {
            let value = position.map_or(*value, |position| stats[position]);
            if value != 0.0 {
                line.push_str(&format!("\"{name}\": {value:.3},"));
            }
        }
        line.push('}');
        Some(line)
    }

    /// Go deps.go `ApplyStatDependencies`, then `FloorGameStats`.
    fn stats(&self) -> Vec<f64> {
        let mut stats = self.without_deps.clone();
        for &(src, dst, amount, step) in &self.dependencies {
            if src == dst {
                stats[dst] *= amount;
            } else if step != 0.0 {
                stats[dst] += (stats[src] / step).floor() * step * amount;
            } else if self.floored[src] {
                stats[dst] += stats[src].floor() * amount;
            } else {
                stats[dst] += stats[src] * amount;
            }
        }
        for (stat, floored) in stats.iter_mut().zip(&self.floored) {
            if *floored {
                *stat = stat.floor();
            }
        }
        stats
    }
}

/// A pet Go registers but never enables.
pub(crate) struct InertPet {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) unit_index: i32,
    /// The actions its metrics list, with zero results.
    pub(crate) actions: Vec<ActionTotals>,
    /// The auras its metrics list.
    pub(crate) auras: Vec<ActionId>,
    /// Positions in `auras` of the permanent auras, in registration order.
    pub(crate) permanent: Vec<usize>,
    /// When the permanent auras last activated.
    aura_start: i64,
    /// Each aura's uptime across iterations, in seconds, and its activations.
    pub(crate) aura_uptime: Vec<Aggregator>,
    pub(crate) aura_procs: Vec<i64>,
    /// Go `pet.GetStats().FlatString()`, logged when the reset dismisses it.
    pub(crate) dismissed_log: String,
    /// Whether each reset logs its dismissal.
    pub(crate) dismissed_at_reset: bool,
    /// Go's time to out of mana for a pet with a mana bar: the hour of a unit that never
    /// spends mana.
    pub(crate) tto: Option<Distribution>,
}

/// The inert pets a prepared input describes, in Go's order.
pub(crate) fn inert_pets(effects: &[Effect]) -> Result<Vec<InertPet>, BuildError> {
    let mut pets = Vec::new();
    for effect in effects {
        let Effect::InertPet {
            name,
            label,
            unit_index,
            metrics_actions,
            auras,
            permanent_auras,
            dismissed_log,
            dismissed_at_reset,
            mana_bar,
            ..
        } = effect
        else {
            continue;
        };
        let permanent = permanent_auras
            .iter()
            .map(|id| {
                auras
                    .iter()
                    .position(|aura| aura == id)
                    .ok_or_else(|| format!("{label}'s permanent aura {id:?} is unlisted"))
            })
            .collect::<Result<_, _>>()?;
        pets.push(InertPet {
            name: name.clone(),
            label: label.clone(),
            unit_index: *unit_index,
            actions: metrics_actions
                .iter()
                .map(|action| ActionTotals {
                    id: action.action_id.clone(),
                    melee: action.melee_metrics,
                    passive: action.passive,
                    school: action.school,
                    targets: super::metrics::defender_reports(),
                })
                .collect(),
            auras: auras.clone(),
            permanent,
            aura_start: 0,
            aura_uptime: vec![Aggregator::default(); auras.len()],
            aura_procs: vec![0; auras.len()],
            dismissed_log: dismissed_log.clone(),
            dismissed_at_reset: *dismissed_at_reset,
            tto: mana_bar.then(Distribution::default),
        });
    }
    Ok(pets)
}

/// A pet's distributions across iterations.
#[derive(Default)]
pub(crate) struct PetTotals {
    pub(crate) dps: Distribution,
    pub(crate) threat: Distribution,
    pub(crate) tto: Distribution,
    pub(crate) oom_seconds: f64,
}

/// A pet Go enables at each reset or an effect summons, simulated as a [`Side::Pet`].
pub(crate) struct ActivePet {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) unit_index: i32,
    pub(crate) config: Config,
    pub(crate) state: Player,
    /// Go `Unit.CastSpeed`.
    pub(crate) cast_speed: f64,
    pub(crate) autos: melee::AutoAttacks,
    /// Go `UnitMetrics.actions` of the pet, in its spellbook order.
    pub(crate) actions: Vec<ActionTotals>,
    pub(crate) mana_regen_casting: usize,
    pub(crate) mana_regen_not_casting: usize,
    pub(crate) mana_gain_spell: Option<SpellId>,
    /// Go `Unit.JowManaMetrics`, registered at the first Judgement of Wisdom proc.
    pub(crate) jow_metrics: Option<usize>,
    pub(crate) totals: PetTotals,
    pub(crate) summon_log: Vec<String>,
    pub(crate) dismiss_log: String,
    /// Go `APLActionCustomRotation.lastExecutedAt`.
    pub(crate) last_custom: i64,
    /// Go `APLRotation.inLoop`.
    pub(crate) in_rotation: bool,
    /// Go `focusBar`, for a pet with one.
    pub(crate) focus: Option<super::focus::FocusBar>,
    /// The distance the pet starts each fight at and its movement speed.
    pub(crate) start_distance: f64,
    pub(crate) movement_speed: f64,
    /// Go `isDynamic` with its pending inheritance; `None` for a pet that only inherits at
    /// its summon.
    pub(crate) inheritance: Option<Inheritance>,
    /// Go `Pet.enabled`, false once the fight's end dismisses it.
    pub(crate) enabled: bool,
    /// A guardian an effect summons during a fight: each reset dismisses it instead.
    pub(crate) summoned: bool,
    /// Go `timeoutAction` while pending, and when the summon's timeout disables the pet.
    pub(crate) timeout: Option<crate::core::queue::Handle>,
    pub(crate) disabled_at: i64,
}

impl ActivePet {
    pub(crate) fn new(
        prepared: &PreparedV2,
        exported: &ExportedPet,
        regen: (usize, usize),
        mana_gain_spell: Option<SpellId>,
        actions: Vec<ActionTotals>,
    ) -> Result<Self, BuildError> {
        let config = unit_config(
            prepared,
            &UnitSource {
                label: &exported.label,
                name: &exported.name,
                level: exported.level,
                reaction_ns: exported.reaction_ns,
                channel_clip_delay_ns: 0,
                distance: exported.distance_yards,
                cast_speed: exported.cast_speed,
                stats: &exported.stats,
                pseudo: &exported.pseudo_stats,
                table: &exported.attack_table,
                melee: &exported.melee,
                max_mana: exported.mana.max,
                teardown_max: exported.mana.max,
                spirit_regen_per_second: 0.0,
                fixed_regen: Some((
                    exported.mana.regen_per_second_casting,
                    exported.mana.regen_per_second_not_casting,
                )),
            },
        )?;
        let inheritance = if exported.dynamic_stats {
            // The gate refuses a term whose owner stat can change outside the tracked stats;
            // the others never see a change.
            Some(Inheritance::new(exported))
        } else {
            None
        };
        Ok(ActivePet {
            name: exported.name.clone(),
            label: exported.label.clone(),
            unit_index: exported.index,
            state: Player::initial(&config),
            cast_speed: exported.cast_speed,
            config,
            autos: melee::AutoAttacks::default(),
            actions,
            mana_regen_casting: regen.0,
            mana_regen_not_casting: regen.1,
            mana_gain_spell,
            jow_metrics: None,
            totals: PetTotals::default(),
            summon_log: exported.summon_log.clone(),
            dismiss_log: exported.dismiss_log.clone(),
            last_custom: NEVER_EXPIRES,
            in_rotation: false,
            focus: exported.focus.as_ref().map(super::focus::FocusBar::new),
            start_distance: exported.distance_yards,
            movement_speed: exported.movement_speed,
            inheritance,
            enabled: false,
            summoned: exported.summoned,
            timeout: None,
            disabled_at: 0,
        })
    }
}

impl<A: Agent> Fight<A> {
    /// The simulated pet a side names.
    pub(crate) fn active_pet(&self, side: Side) -> &ActivePet {
        match side {
            Side::Pet(pet) => &self.active_pets[pet as usize],
            _ => panic!("{side:?} is not a pet"),
        }
    }

    /// [`Self::active_pet`], mutable.
    pub(crate) fn active_pet_mut(&mut self, side: Side) -> &mut ActivePet {
        match side {
            Side::Pet(pet) => &mut self.active_pets[pet as usize],
            _ => panic!("{side:?} is not a pet"),
        }
    }

    /// The simulated pet with a unit label, if one has it.
    pub(crate) fn pet_side(&self, label: &str) -> Option<Side> {
        self.active_pets
            .iter()
            .position(|pet| pet.label == label)
            .map(|pet| Side::Pet(pet as u8))
    }

    /// Every simulated pet, in unit index order.
    pub(crate) fn pet_sides(&self) -> Vec<Side> {
        (0..self.active_pets.len())
            .map(|pet| Side::Pet(pet as u8))
            .collect()
    }

    /// The simulated pets with unit indexes below `unit`, from position `next` on.
    fn active_pets_before(&self, next: &mut usize, unit: i32) -> Vec<Side> {
        let mut sides = Vec::new();
        while *next < self.active_pets.len() && self.active_pets[*next].unit_index < unit {
            sides.push(Side::Pet(*next as u8));
            *next += 1;
        }
        sides
    }

    /// Go `addTracker` for a pet its Enable enables.
    fn add_pet_tracker(&mut self, side: Side) {
        let Side::Pet(pet) = side else {
            panic!("{side:?} is not a pet");
        };
        self.pet_trackers.push(pet);
        let tracker_min = self.trackers[side.index()].min_expires;
        self.reschedule_tracker(tracker_min);
    }

    /// Go `removeTracker`: swap removal, as Go's slice helper does.
    fn remove_pet_tracker(&mut self, side: Side) {
        let Side::Pet(pet) = side else {
            panic!("{side:?} is not a pet");
        };
        if let Some(index) = self.pet_trackers.iter().position(|&entry| entry == pet) {
            self.pet_trackers.swap_remove(index);
        }
    }

    /// Go `Character.reset`'s pet loop, after the owner's agent: every registered pet in
    /// registration order, which is unit index order.
    pub(crate) fn reset_pets(&mut self) {
        self.pet_trackers.clear();
        let mut next = 0;
        for index in 0..self.pets.len() {
            for side in self.active_pets_before(&mut next, self.pets[index].unit_index) {
                self.reset_active_pet(side);
            }
            // Go `auraTracker.reset` of the pet's unit: its permanent auras activate.
            self.pets[index].aura_start = self.now;
            if self.log.is_some() {
                let label = self.pets[index].label.clone();
                for position in self.pets[index].permanent.clone() {
                    let id = action_string(&self.pets[index].auras[position]);
                    self.log_at(self.now, &label, &format!("Aura gained: {id}"));
                }
            }
            if self.log.is_some() && self.pets[index].dismissed_at_reset {
                let (label, stats) = (
                    self.pets[index].label.clone(),
                    self.pets[index].dismissed_log.clone(),
                );
                self.log_at(self.now, &label, "Pet dismissed");
                self.log_at(self.now, &label, &stats);
            }
        }
        for side in self.active_pets_before(&mut next, i32::MAX) {
            self.reset_active_pet(side);
        }
    }

    /// Go `Pet.reset` and `Enable` for the pet enabled at reset: its unit resets, then it is
    /// summoned at the reset's time with a full mana bar, and its rotation is due at once.
    fn reset_active_pet(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        let config = &pet.config;
        let state = &mut pet.state;
        state.gcd = STARTING_CD_TIME;
        state.rotation_timer = STARTING_CD_TIME;
        state.hardcast = Hardcast::idle(STARTING_CD_TIME);
        state.hardcast_action = None;
        state.rotation_action = None;
        state.channeled_dot = None;
        state.queued = None;
        state.mana_spent = 0.0;
        state.mana_gained = 0.0;
        state.oom_time = 0;
        state.went_oom = false;
        state.first_oom = 0;
        state.spell_cost_percent_modifier = config.initial.spell_cost_percent_modifier;
        state.school_damage_dealt_multiplier = config.school_damage_dealt_multiplier;
        state.damage_dealt_multiplier = config.damage_dealt_multiplier;
        state.disable_dw_miss_penalty = config.melee.disable_dw_miss_penalty;
        state.cast_speed_multiplier = config.initial.cast_speed_multiplier;
        state.attack_speed_multiplier = config.melee.attack_speed_multiplier;
        state.melee_speed_multiplier = config.melee.melee_speed_multiplier;
        state.powers = config.powers;
        state.spirit_regen_rate_casting = config.initial.spirit_regen_rate_casting;
        state.spirit_regen_multiplier = config.initial.spirit_regen_multiplier;
        state.force_full_spirit_regen = config.initial.force_full_spirit_regen;
        state.five_second_rule_refresh = 0;
        state.spirit_attribution = None;
        state.mana = config.max_mana;
        state.health = config.max_health;
        state.mana_regen_multiplier = 1.0;
        state.waiting_for_mana = 0.0;
        state.waiting_for_mana_start = 0;
        let (casting, not_casting) = config.fixed_regen.expect("a pet's regeneration is fixed");
        state.mana_tick_casting = casting * 2.0;
        state.mana_tick_not_casting = not_casting * 2.0;
        pet.last_custom = NEVER_EXPIRES;
        pet.in_rotation = false;
        pet.config.distance = pet.start_distance;
        pet.state.moving = false;
        pet.state.movement = None;
        // Go enableDynamicStats: a fresh inheritance action with nothing pending.
        if let Some(inheritance) = pet.inheritance.as_mut() {
            inheritance.pending = Powers::default();
            inheritance.scheduled = false;
            inheritance.without_deps = inheritance.initial_without_deps.clone();
            inheritance.inherited = inheritance.initial_inherited.clone();
        }
        pet.enabled = true;
        pet.timeout = None;
        let summoned = pet.summoned;
        self.reset_auras(side);
        self.reset_auto_attacks_of(side);
        if summoned {
            // Go: the unit's reset enables it, then its agent's Reset dismisses it.
            self.active_pet_mut(side).enabled = false;
            self.log_pet_dismissed(side);
            return;
        }
        // Enable.
        if self.log.is_some() {
            let pet = self.active_pet(side);
            let (label, lines) = (pet.label.clone(), pet.summon_log.clone());
            for line in lines {
                self.log_at(self.now, &label, &line);
            }
            self.log_at(self.now, &label, "Pet summoned");
        }
        let ready = self.now.max(0);
        self.set_gcd_timer_of(side, ready);
        self.add_pet_tracker(side);
        self.enable_pet_focus(side);
    }

    /// A pet aura's expiry: a permanent aura's stat buff leaves the dynamic pet's stats.
    pub(crate) fn pet_aura_expired(&mut self, aura: super::AuraRef) {
        let Some(inheritance) = self.active_pet(aura.side).inheritance.as_ref() else {
            return;
        };
        if inheritance.aura_stats.is_empty() {
            return;
        }
        let label = &self.aura(aura).label;
        let Some(position) = inheritance
            .aura_stats
            .iter()
            .position(|(known, _)| known == label)
        else {
            return;
        };
        if let Some(inheritance) = self.active_pet_mut(aura.side).inheritance.as_mut() {
            inheritance.aura_expired(position);
        }
    }

    /// Go `Disable`'s `resetDynamicStats` for a dynamic pet enabled at reset; a summoned
    /// guardian's dismissal line is Go's own.
    fn remove_pet_inheritance(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        if pet.summoned {
            return;
        }
        if let Some(inheritance) = pet.inheritance.as_mut() {
            inheritance.remove_inherited();
        }
    }

    /// Go `Disable`'s closing lines for a pet that was out.
    fn log_pet_dismissed(&mut self, side: Side) {
        if self.log.is_some() {
            let pet = self.active_pet(side);
            let stats = pet
                .inheritance
                .as_ref()
                .and_then(Inheritance::dismiss_line)
                .unwrap_or_else(|| pet.dismiss_log.clone());
            let label = pet.label.clone();
            self.log_at(self.now, &label, "Pet dismissed");
            self.log_at(self.now, &label, &stats);
        }
    }

    /// Go `Pet.EnableWithTimeout` for a summoned guardian: a pet already out only logs so,
    /// otherwise `Enable` fills its mana, starts its rotation and swing, and logs its stats; in
    /// both cases the timeout restarts.
    pub(crate) fn summon_pet(&mut self, side: Side, duration: i64) {
        let now = self.now;
        let pet = self.active_pet_mut(side);
        if pet.enabled {
            if self.log.is_some() {
                self.unit_log(side, "Pet already summoned");
            }
        } else {
            pet.state.mana = pet.config.max_mana;
            pet.state.health = pet.config.max_health;
            pet.enabled = true;
            self.set_gcd_timer_of(side, now.max(0));
            self.enable_melee_swing(side);
            if self.log.is_some() {
                let pet = self.active_pet(side);
                let (label, lines) = (pet.label.clone(), pet.summon_log.clone());
                for line in lines {
                    self.log_at(now, &label, &line);
                }
                self.log_at(now, &label, "Pet summoned");
            }
            self.add_pet_tracker(side);
        }
        // Go SetTimeoutAction.
        if let Some(handle) = self.active_pet_mut(side).timeout.take() {
            self.queue.cancel(handle);
        }
        let handle = self.schedule(
            now + duration,
            super::PRIORITY_GCD,
            Action::PetTimeout(side),
        );
        let pet = self.active_pet_mut(side);
        pet.timeout = Some(handle);
        pet.disabled_at = now + duration;
    }

    /// Go `Pet.Disable` during a fight: its rotation and swing stop, its cast clears, its
    /// auras fade and it is dismissed; a pet that is not out only logs so.
    pub(crate) fn disable_pet(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        if !pet.enabled {
            if self.log.is_some() {
                self.unit_log(side, "No pet summoned");
            }
            return;
        }
        if let Some(handle) = pet.state.rotation_action.take() {
            self.queue.cancel(handle);
        }
        self.disable_pet_focus(side);
        self.cancel_melee_swing(side);
        let pet = self.active_pet_mut(side);
        pet.enabled = false;
        pet.state.hardcast = Hardcast::idle(0);
        if let Some(handle) = pet.timeout.take() {
            self.queue.cancel(handle);
        }
        self.remove_pet_inheritance(side);
        self.expire_all_auras(side);
        self.remove_pet_tracker(side);
        self.log_pet_dismissed(side);
    }

    /// Go `APLRotation.DoNextAction` of an acting unit.
    pub(crate) fn do_next_action_of(&mut self, side: Side) {
        match side {
            Side::Pet(_) => self.pet_do_next_action(side),
            _ => self.do_next_action(),
        }
    }

    /// Go `APLRotation.DoNextAction` for a pet's rotation: one custom rotation action, which
    /// runs at most once per timestep.
    fn pet_do_next_action(&mut self, side: Side) {
        if self.now < 0 {
            return;
        }
        let pet = self.active_pet(side);
        // Go evaluates a disabled pet's rotation too, as a hunter pet past its uptime is.
        if pet.in_rotation || pet.state.rotation_timer > self.now {
            return;
        }
        assert!(pet.state.channeled_dot.is_none(), "pets do not channel");
        self.active_pet_mut(side).in_rotation = true;
        // Go DoNextAction brings a moving unit's position up to date first.
        self.update_position(side, false);
        let mut executed = 0;
        loop {
            let now = self.now;
            let pet = self.active_pet_mut(side);
            if pet.last_custom == now {
                break;
            }
            pet.last_custom = now;
            if self.whelp.as_ref().is_some_and(|whelp| whelp.pet == side) {
                self.whelp_rotation(side);
            } else {
                A::pet_rotation(self, side);
            }
            executed += 1;
        }
        self.active_pet_mut(side).in_rotation = false;
        if executed == 0 && self.log.is_some() {
            self.unit_log(side, "No available actions!");
        }
        let state = self.unit(side);
        if state.rotation_timer <= self.now {
            // A moving unit evaluates again after its reaction time, GCD or not.
            let mut next = self.now + self.unit_config(side).reaction;
            if !state.moving {
                next = next.max(state.gcd);
            }
            self.wait_until_of(side, next);
        }
    }

    /// Go `Character.doneIteration`'s pet loop, before the owner's own: every registered
    /// pet in registration order, then each pet's damage counts for its owner.
    pub(crate) fn pets_done_iteration(&mut self) {
        let mut next = 0;
        for index in 0..self.pets.len() {
            for side in self.active_pets_before(&mut next, self.pets[index].unit_index) {
                self.active_pet_done_iteration(side);
            }
            // Go `auraTracker.doneIteration`: the permanent auras fade, then every aura folds
            // its uptime.
            let label = self.pets[index].label.clone();
            let uptime = seconds((self.now - self.pets[index].aura_start.max(0)).max(0));
            for position in self.pets[index].permanent.clone() {
                if self.log.is_some() {
                    let id = action_string(&self.pets[index].auras[position]);
                    self.log_at(self.now, &label, &format!("Aura faded: {id}"));
                }
            }
            let pet = &mut self.pets[index];
            for position in 0..pet.auras.len() {
                if pet.permanent.contains(&position) {
                    pet.aura_uptime[position].add(uptime);
                    pet.aura_procs[position] += 1;
                } else {
                    pet.aura_uptime[position].add(0.0);
                }
            }
            if self.log.is_some() {
                self.log_at(self.now, &label, "No pet summoned");
            }
        }
        for side in self.active_pets_before(&mut next, i32::MAX) {
            self.active_pet_done_iteration(side);
        }
    }

    /// Go `Pet.doneIteration`: the unit's own end of fight, then `Disable`, then the owner's
    /// `AddFinalPetMetrics`.
    fn active_pet_done_iteration(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        let was_enabled = pet.enabled;
        pet.enabled = false;
        self.unit_mut(side).hardcast = Hardcast::idle(0);
        self.mana_done_iteration(side);
        self.aura_done_iteration(side);
        for spell in 0..self.spells.len() {
            if self.spells[spell].caster == side {
                self.spell_done_iteration(spell);
            }
        }
        if was_enabled {
            self.remove_pet_inheritance(side);
            self.remove_pet_tracker(side);
            self.log_pet_dismissed(side);
        } else if self.log.is_some() {
            let label = self.active_pet(side).label.clone();
            self.log_at(self.now, &label, "No pet summoned");
        }
        let damage = self.active_pet(side).totals.dps.total;
        self.totals.dps.total += damage;
    }

    /// Go `UnitMetrics.doneIteration` for every pet, after the owner's.
    pub(crate) fn pets_metrics_done_iteration(&mut self, seed: i64) {
        let duration = self.duration;
        let duration_seconds = seconds(duration);
        let hour = 60 * 60 * NS_PER_SECOND;
        for pet in &mut self.pets {
            if let Some(tto) = pet.tto.as_mut() {
                tto.total = seconds(hour) * duration_seconds;
                tto.done_iteration(duration, seed);
            }
        }
        for side in self.pet_sides() {
            self.active_pet_metrics_done_iteration(side, seed);
        }
    }

    /// [`Self::pets_metrics_done_iteration`] for one simulated pet.
    fn active_pet_metrics_done_iteration(&mut self, side: Side, seed: i64) {
        let duration = self.duration;
        let duration_seconds = seconds(duration);
        let time_to_oom = self.time_to_oom(side, f64::INFINITY);
        let pet = self.active_pet_mut(side);
        // Go reports no time to out of mana for a unit without a mana bar.
        pet.totals.tto.total = if pet.config.max_mana > 0.0 {
            seconds(time_to_oom) * duration_seconds
        } else {
            0.0
        };
        pet.totals.dps.done_iteration(duration, seed);
        pet.totals.threat.done_iteration(duration, seed);
        pet.totals.tto.done_iteration(duration, seed);
        pet.totals.oom_seconds += seconds(pet.state.oom_time);
    }

    /// The unit indexes every action metric lists after the target and the player.
    pub(crate) fn extra_unit_indexes(&self) -> Vec<i32> {
        let mut units: Vec<i32> = self.pets.iter().map(|pet| pet.unit_index).collect();
        units.extend(self.active_pets.iter().map(|pet| pet.unit_index));
        units.sort_unstable();
        units
    }
}

impl<A: Agent> Fight<A> {
    /// Go `AddStatsDynamic` on the player for the stats the runtime tracks, then
    /// `processDynamicBonus`'s pet loop: an enabled dynamic pet adds the change to its pending
    /// inheritance and, unless its action is already pending, takes it at the next heartbeat.
    pub(crate) fn set_player_powers(&mut self, powers: Powers) {
        let old = self.player.powers;
        self.player.powers = powers;
        let (now, offset) = (self.now, self.heartbeat_offset);
        for index in 0..self.active_pets.len() {
            let side = Side::Pet(index as u8);
            let pet = self.active_pet_mut(side);
            if !pet.enabled {
                continue;
            }
            let Some(inheritance) = pet.inheritance.as_mut() else {
                continue;
            };
            for index in 0..POWER_STATS.len() {
                *power_mut(&mut inheritance.pending, index) +=
                    power(&powers, index) - power(&old, index);
            }
            if !inheritance.scheduled {
                inheritance.scheduled = true;
                let heartbeats = (now - offset) / PET_UPDATE_INTERVAL;
                let at = PET_UPDATE_INTERVAL * (heartbeats + 1) + offset;
                self.schedule(at, PRIORITY_DOT, Action::PetInheritance(side));
            }
        }
    }

    /// Go `AddStatDynamic` on a dynamic pet: the amount joins the stat before dependencies,
    /// from which the pet's stats are recomputed. Returns whether the pet could take it.
    pub(crate) fn add_pet_stat_dynamic(&mut self, side: Side, stat: &str, amount: f64) -> bool {
        let pet = self.active_pet_mut(side);
        let Some(inheritance) = pet.inheritance.as_mut() else {
            return false;
        };
        let Some(index) = power_index(stat) else {
            return false;
        };
        inheritance.without_deps[index] += amount;
        let stats = inheritance.stats();
        for (power, slot) in inheritance.power_stats.iter().enumerate() {
            if let Some(stat) = slot {
                *power_mut(&mut pet.state.powers, power) = stats[*stat];
            }
        }
        true
    }

    /// Go `consumed` on the heartbeat once it is popped, before the advance to its time.
    pub(crate) fn pet_inheritance_popped(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        pet.inheritance
            .as_mut()
            .expect("the pet is dynamic")
            .scheduled = false;
    }

    /// Go `statInheritanceAction`: the enabled pet takes its pending inheritance, Go
    /// `AddOwnerStats`. A stat change during the advance to it scheduled the next heartbeat,
    /// and its pending change is taken now too.
    pub(crate) fn pet_inheritance(&mut self, side: Side) {
        let pet = self.active_pet_mut(side);
        let enabled = pet.enabled;
        let inheritance = pet.inheritance.as_mut().expect("the pet is dynamic");
        let pending = std::mem::take(&mut inheritance.pending);
        if !enabled {
            return;
        }
        // Go AddStatsDynamic: the change joins the stats before dependencies, from which the
        // stats are recomputed.
        for &(owner, stat, coefficient) in &inheritance.terms {
            let change = power(&pending, owner) * coefficient;
            inheritance.without_deps[stat] += change;
            inheritance.inherited[stat] += change;
        }
        let stats = inheritance.stats();
        for (index, slot) in inheritance.power_stats.iter().enumerate() {
            if let Some(stat) = slot {
                *power_mut(&mut pet.state.powers, index) = stats[*stat];
            }
        }
    }
}

/// The aura actions of each of the player's pets, simulated or inert, in Go registration
/// order, which is unit index order: Go `PetAgents`, read by a rotation's pet source unit.
pub(crate) fn pet_agent_auras(prepared: &PreparedV2) -> Vec<Vec<ActionId>> {
    let mut pets: Vec<(i32, Vec<ActionId>)> = prepared
        .pets
        .iter()
        .map(|pet| {
            let auras = pet
                .auras
                .iter()
                .filter_map(|aura| aura.action_id.clone())
                .collect();
            (pet.index, auras)
        })
        .collect();
    for effect in &prepared.effects {
        if let Effect::InertPet {
            unit_index, auras, ..
        } = effect
        {
            pets.push((*unit_index, auras.clone()));
        }
    }
    pets.sort_by_key(|(index, _)| *index);
    pets.into_iter().map(|(_, auras)| auras).collect()
}
