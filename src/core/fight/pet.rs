//! Go pet.go: the player's registered pets.
//!
//! Go adds every registered pet to the environment and gives each a unit index after the
//! player's. Rust simulates the pet that each reset enables, an [`ActivePet`] acting as the
//! unit [`Side::Pet`] with its own state, configuration, auras, spells, swings, mana and
//! metrics; the shared runtime reaches it through the caster of each spell. A pet that is
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
    unit_config, ActionReport, ActionTotals, Agent, BuildError, Config, Fight, Hardcast, Player,
    Side, SpellId, UnitSource,
};

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
                    targets: [ActionReport::new(0), ActionReport::new(1)],
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

/// The pet Go enables at each reset, simulated as [`Side::Pet`].
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
        })
    }
}

impl<A: Agent> Fight<A> {
    /// Go `Character.reset`'s pet loop, after the owner's agent: every registered pet in
    /// registration order.
    pub(crate) fn reset_pets(&mut self) {
        let active = self.pet.as_ref().map(|pet| pet.unit_index);
        let mut done_active = false;
        for index in 0..self.pets.len() {
            if let Some(unit) = active {
                if !done_active && unit < self.pets[index].unit_index {
                    self.reset_active_pet();
                    done_active = true;
                }
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
        if active.is_some() && !done_active {
            self.reset_active_pet();
        }
    }

    /// Go `Pet.reset` and `Enable` for the pet enabled at reset: its unit resets, then it is
    /// summoned at the reset's time with a full mana bar, and its rotation is due at once.
    fn reset_active_pet(&mut self) {
        let pet = self.pet.as_mut().expect("the pet is simulated");
        let config = &pet.config;
        let state = &mut pet.state;
        state.gcd = STARTING_CD_TIME;
        state.rotation_timer = STARTING_CD_TIME;
        state.hardcast = Hardcast {
            expires: STARTING_CD_TIME,
            spell: None,
            target: Side::Target,
        };
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
        self.reset_auras(Side::Pet);
        self.reset_auto_attacks_of(Side::Pet);
        // Enable.
        if self.log.is_some() {
            let pet = self.pet.as_ref().expect("the pet is simulated");
            let (label, lines) = (pet.label.clone(), pet.summon_log.clone());
            for line in lines {
                self.log_at(self.now, &label, &line);
            }
            self.log_at(self.now, &label, "Pet summoned");
        }
        let ready = self.now.max(0);
        self.set_gcd_timer_of(Side::Pet, ready);
        let tracker_min = self.trackers[Side::Pet.index()].min_expires;
        self.reschedule_tracker(tracker_min);
    }

    /// Go `APLRotation.DoNextAction` of an acting unit.
    pub(crate) fn do_next_action_of(&mut self, side: Side) {
        match side {
            Side::Pet => self.pet_do_next_action(),
            _ => self.do_next_action(),
        }
    }

    /// Go `APLRotation.DoNextAction` for a pet's rotation: one custom rotation action, which
    /// runs at most once per timestep.
    fn pet_do_next_action(&mut self) {
        if self.now < 0 {
            return;
        }
        let pet = self.pet.as_ref().expect("the pet is simulated");
        if pet.in_rotation || pet.state.rotation_timer > self.now {
            return;
        }
        assert!(pet.state.channeled_dot.is_none(), "pets do not channel");
        self.pet.as_mut().expect("the pet is simulated").in_rotation = true;
        let mut executed = 0;
        loop {
            let pet = self.pet.as_mut().expect("the pet is simulated");
            if pet.last_custom == self.now {
                break;
            }
            pet.last_custom = self.now;
            A::pet_rotation(self);
            executed += 1;
        }
        self.pet.as_mut().expect("the pet is simulated").in_rotation = false;
        if executed == 0 && self.log.is_some() {
            self.unit_log(Side::Pet, "No available actions!");
        }
        let state = self.unit(Side::Pet);
        if state.rotation_timer <= self.now {
            let next = (self.now + self.unit_config(Side::Pet).reaction).max(state.gcd);
            self.wait_until_of(Side::Pet, next);
        }
    }

    /// Go `Character.doneIteration`'s pet loop, before the owner's own: every registered
    /// pet in registration order, then each pet's damage counts for its owner.
    pub(crate) fn pets_done_iteration(&mut self) {
        let active = self.pet.as_ref().map(|pet| pet.unit_index);
        let mut done_active = false;
        for index in 0..self.pets.len() {
            if let Some(unit) = active {
                if !done_active && unit < self.pets[index].unit_index {
                    self.active_pet_done_iteration();
                    done_active = true;
                }
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
        if active.is_some() && !done_active {
            self.active_pet_done_iteration();
        }
    }

    /// Go `Pet.doneIteration`: the unit's own end of fight, then `Disable`, then the owner's
    /// `AddFinalPetMetrics`.
    fn active_pet_done_iteration(&mut self) {
        self.unit_mut(Side::Pet).hardcast = Hardcast {
            expires: 0,
            spell: None,
            target: Side::Target,
        };
        self.mana_done_iteration(Side::Pet);
        self.aura_done_iteration(Side::Pet);
        for spell in 0..self.spells.len() {
            if self.spells[spell].caster == Side::Pet {
                self.spell_done_iteration(spell);
            }
        }
        if self.log.is_some() {
            let pet = self.pet.as_ref().expect("the pet is simulated");
            let (label, stats) = (pet.label.clone(), pet.dismiss_log.clone());
            self.log_at(self.now, &label, "Pet dismissed");
            self.log_at(self.now, &label, &stats);
        }
        let damage = self
            .pet
            .as_ref()
            .expect("the pet is simulated")
            .totals
            .dps
            .total;
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
        if self.pet.is_none() {
            return;
        }
        let time_to_oom = self.time_to_oom(Side::Pet, f64::INFINITY);
        let pet = self.pet.as_mut().expect("the pet is simulated");
        pet.totals.tto.total = seconds(time_to_oom) * duration_seconds;
        pet.totals.dps.done_iteration(duration, seed);
        pet.totals.threat.done_iteration(duration, seed);
        pet.totals.tto.done_iteration(duration, seed);
        pet.totals.oom_seconds += seconds(pet.state.oom_time);
    }

    /// The unit indexes every action metric lists after the target and the player.
    pub(crate) fn extra_unit_indexes(&self) -> Vec<i32> {
        let mut units: Vec<i32> = self.pets.iter().map(|pet| pet.unit_index).collect();
        units.extend(self.pet.as_ref().map(|pet| pet.unit_index));
        units.sort_unstable();
        units
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
