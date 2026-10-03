//! Go spell_result.go, spell_outcome.go and spell_resistances.go for magic spells.

use crate::mechanics::damage::{
    crit_damage_multiplier, partial_resist_thresholds, resist_coefficient, spell_chance_to_miss,
};

use super::{log::action_string, Action, Agent, Fight, Side, SpellId, PRIORITY_GCD};

pub(crate) const OUTCOME_MISS: u16 = 1;
pub(crate) const OUTCOME_HIT: u16 = 1 << 1;
pub(crate) const OUTCOME_CRIT: u16 = 1 << 2;
pub(crate) const OUTCOME_PARTIAL_1_4: u16 = 1 << 3;
pub(crate) const OUTCOME_PARTIAL_2_4: u16 = 1 << 4;
pub(crate) const OUTCOME_PARTIAL_3_4: u16 = 1 << 5;
pub(crate) const OUTCOME_PARTIAL: u16 =
    OUTCOME_PARTIAL_1_4 | OUTCOME_PARTIAL_2_4 | OUTCOME_PARTIAL_3_4;
pub(crate) const OUTCOME_DODGE: u16 = 1 << 6;
pub(crate) const OUTCOME_GLANCE: u16 = 1 << 7;
pub(crate) const OUTCOME_PARRY: u16 = 1 << 8;
pub(crate) const OUTCOME_BLOCK: u16 = 1 << 9;
/// Go outcome appliers the runtime implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    MagicHitAndCrit,
    MagicHit,
    Tick,
    TickMagicCrit,
    /// Go `OutcomeAlwaysHitNoHitCounter`.
    AlwaysHitNoHitCounter,
    /// Go `OutcomeMagicHitNoHitCounter`: a miss still counts.
    MagicHitNoHitCounter,
}

/// Go `OutcomeLanded`; the runtime has no crushing blows.
pub(crate) const OUTCOME_LANDED: u16 = OUTCOME_HIT | OUTCOME_CRIT | OUTCOME_GLANCE | OUTCOME_BLOCK;

/// Go `SpellResult`, carried by value until its damage is dealt.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpellResult {
    pub(crate) target: Side,
    pub(crate) outcome: u16,
    pub(crate) damage: f64,
    pub(crate) threat: f64,
}

impl SpellResult {
    pub(crate) fn landed(&self) -> bool {
        self.outcome & OUTCOME_LANDED != 0
    }

    pub(crate) fn crit(&self) -> bool {
        self.outcome & OUTCOME_CRIT != 0
    }

    /// Go `HitOutcome.String` for spell outcomes.
    fn outcome_string(&self) -> String {
        let partial = if self.outcome & OUTCOME_PARTIAL_1_4 != 0 {
            " (25% Resist)"
        } else if self.outcome & OUTCOME_PARTIAL_2_4 != 0 {
            " (50% Resist)"
        } else if self.outcome & OUTCOME_PARTIAL_3_4 != 0 {
            " (75% Resist)"
        } else {
            ""
        };
        if self.outcome & OUTCOME_MISS != 0 {
            "Miss".into()
        } else if self.outcome & OUTCOME_DODGE != 0 {
            "Dodge".into()
        } else if self.outcome & OUTCOME_PARRY != 0 {
            "Parry".into()
        } else if self.outcome & OUTCOME_BLOCK != 0 && self.outcome & OUTCOME_CRIT != 0 {
            "BlockedCrit".into()
        } else if self.outcome & OUTCOME_BLOCK != 0 {
            "Block".into()
        } else if self.outcome & OUTCOME_GLANCE != 0 {
            format!("Glance{partial}")
        } else if self.outcome & OUTCOME_CRIT != 0 {
            format!("Crit{partial}")
        } else if self.outcome & OUTCOME_HIT != 0 {
            format!("Hit{partial}")
        } else {
            "Empty".into()
        }
    }

    /// Go `SpellResult.DamageString`.
    fn damage_string(&self) -> String {
        if self.landed() {
            format!("{} for {:.3} damage", self.outcome_string(), self.damage)
        } else {
            self.outcome_string()
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Go `Spell.schoolValue`: one school's entry, or the larger of Fire and Frost for
    /// Frostfire so an effect on both schools never counts twice.
    pub(crate) fn school_value(&self, spell: SpellId, values: &[f64; 8]) -> f64 {
        let state = &self.spells[spell];
        if state.frostfire {
            values[super::SCHOOL_INDEX_FIRE].max(values[super::SCHOOL_INDEX_FROST])
        } else {
            values[state.school_index]
        }
    }

    /// Go `Unit.GetSpellDamageValue`: generic plus school spell damage.
    pub(crate) fn spell_power(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        self.player.powers.spell_damage
            + state.bonus_spell_damage
            + self.school_value(spell, &self.config.school_damage)
    }

    /// Go `Spell.BonusDamage` for a magic spell.
    pub(crate) fn bonus_damage(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        let mut bonus = state.bonus_base_damage;
        bonus += self.spell_power(spell)
            + 0.0
            + self.school_value(spell, &self.config.target_school_bonus_spell_damage);
        bonus
    }

    /// Go `Spell.AttackerDamageMultiplier` for a direct hit.
    pub(crate) fn attacker_multiplier(&self, spell: SpellId, periodic: bool) -> f64 {
        let state = &self.spells[spell];
        if state.flags.ignore_attacker_modifiers {
            return 1.0;
        }
        let additive = if periodic {
            state.damage_multiplier_additive + self.config.dot_damage_multiplier_additive - 1.0
        } else {
            state.damage_multiplier_additive + state.direct_damage_multiplier_additive
        };
        let internal = self.config.damage_dealt_multiplier
            * self.school_value(spell, &self.player.school_damage_dealt_multiplier)
            * self.config.table.damage_dealt_multiplier;
        internal * state.damage_multiplier * additive
    }

    /// Go `Spell.TargetDamageMultiplier`.
    pub(crate) fn target_multiplier(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        if state.flags.ignore_target_modifiers {
            return 1.0;
        }
        self.config.target_damage_taken_multiplier
            * self.school_value(spell, &self.target.school_damage_taken_multiplier)
            * self.config.table.damage_taken_multiplier
    }

    fn resist(&self, spell: SpellId, binary: bool) -> f64 {
        let state = &self.spells[spell];
        // Go resistCoeff: Frostfire Bolt checks the lower resistance.
        let resistance = if state.frostfire {
            let resistance = &self.target.resistance;
            resistance[super::SCHOOL_INDEX_FIRE].min(resistance[super::SCHOOL_INDEX_FROST])
        } else {
            self.target.resistance[state.school_index]
        };
        resist_coefficient(
            resistance,
            self.config.spell_piercing,
            self.config.player_level,
            self.config.target_level,
            binary,
        )
    }

    /// Go `Spell.SpellHitChance`.
    pub(crate) fn spell_hit_chance(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        let mut hit = self.config.spell_hit_percent + state.bonus_hit_percent;
        if state.class_spell_mask {
            hit += self.school_value(spell, &self.config.school_bonus_hit_chance);
        }
        hit / 100.0
    }

    /// Go `Spell.SpellCritChance`.
    pub(crate) fn spell_crit_chance(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        let crit = self.player.powers.spell_crit_percent
            + state.bonus_crit_percent
            + self.config.table.bonus_spell_crit_percent
            - self.config.target_reduced_crit_taken_percent;
        (crit / 100.0 - self.config.table.spell_crit_suppression).max(0.0)
    }

    /// Go `CalcDamage` with `OutcomeMagicHitAndCrit`.
    pub(crate) fn calc_damage(
        &mut self,
        spell: SpellId,
        target: Side,
        base_damage: f64,
    ) -> SpellResult {
        let attacker = self.attacker_multiplier(spell, false);
        let mut base = base_damage;
        if self.spells[spell].bonus_coefficient > 0.0 {
            base += self.spells[spell].bonus_coefficient * self.bonus_damage(spell);
        }
        self.calc_damage_internal(spell, target, base, attacker, Outcome::MagicHitAndCrit)
    }

    /// Go `CalcDamage` with `OutcomeMagicHit`: no crit roll.
    pub(crate) fn calc_damage_hit_only(
        &mut self,
        spell: SpellId,
        target: Side,
        base_damage: f64,
    ) -> SpellResult {
        let attacker = self.attacker_multiplier(spell, false);
        let mut base = base_damage;
        if self.spells[spell].bonus_coefficient > 0.0 {
            base += self.spells[spell].bonus_coefficient * self.bonus_damage(spell);
        }
        self.calc_damage_internal(spell, target, base, attacker, Outcome::MagicHit)
    }

    /// Go `calcDamageInternal` for a direct magic spell.
    fn calc_damage_internal(
        &mut self,
        spell: SpellId,
        target: Side,
        base: f64,
        attacker: f64,
        outcome: Outcome,
    ) -> SpellResult {
        let mut result = SpellResult {
            target,
            outcome: 0,
            damage: base,
            threat: 0.0,
        };
        result.damage *= attacker;
        let after_attacker = result.damage;

        // Go applyResistances: binary spells and ignored resists take no partial roll.
        let binary = self.spells[spell].flags.binary;
        if !self.spells[spell].flags.ignore_resists && !binary {
            let roll = self.random("Partial Resist");
            let (none, quarter, half) = partial_resist_thresholds(self.resist(spell, false));
            let (multiplier, outcome) = if roll > none {
                (1.0, 0)
            } else if roll > quarter {
                (0.75, OUTCOME_PARTIAL_1_4)
            } else if roll > half {
                (0.5, OUTCOME_PARTIAL_2_4)
            } else {
                (0.25, OUTCOME_PARTIAL_3_4)
            };
            result.damage *= multiplier;
            result.outcome |= outcome;
        }
        let after_resistances = result.damage;

        // Go applyTargetModifiers.
        if !self.spells[spell].flags.ignore_target_modifiers {
            if self.spells[spell].school_index > 1 {
                result.damage += self.config.target_bonus_spell_damage_taken;
            }
            result.damage *= self.target_multiplier(spell);
        }
        let after_target = result.damage;

        let partial = result.outcome & super::damage::OUTCOME_PARTIAL;
        self.apply_outcome(spell, &mut result, binary, outcome);
        if partial != 0 {
            result.outcome |= partial;
        }
        let after_outcome = result.damage;
        self.apply_post_outcome_modifiers(spell, &mut result);

        if self.log.is_some() {
            self.log_damage_debug(
                spell,
                base,
                [
                    after_attacker,
                    after_resistances,
                    after_target,
                    after_outcome,
                ],
                result.damage,
            );
        }

        result.threat = if result.landed() {
            let state = &self.spells[spell];
            (result.damage * state.threat_multiplier + state.flat_threat_bonus)
                * self.player.threat_multiplier
        } else {
            0.0
        };
        result
    }

    /// Go `ApplyPostOutcomeDamageModifiers`: the target's dynamic modifiers in order.
    pub(crate) fn apply_post_outcome_modifiers(&self, spell: SpellId, result: &mut SpellResult) {
        for modifier in &self.damage_taken_modifiers {
            if self.spells[spell].school & modifier.school_mask != 0
                && self.aura(modifier.aura).active
            {
                result.damage *= modifier.multiplier;
            }
        }
        for modifier in &self.spell_damage_taken_modifiers {
            if modifier.spells[spell]
                && modifier
                    .auras
                    .iter()
                    .any(|&aura| self.trackers[aura.side.index()].auras[aura.index].active)
            {
                result.damage *= modifier.multiplier;
            }
        }
        result.damage = result.damage.max(0.0);
    }

    /// Go `calcDamageInternal`'s debug line.
    pub(crate) fn log_damage_debug(
        &mut self,
        spell: SpellId,
        base: f64,
        stages: [f64; 4],
        damage: f64,
    ) {
        let line = format!(
            "[{}] {} [DEBUG] MAP: {:.1}, RAP: {:.1}, SP: {:.1}, BaseDamage:{:.1}, AfterAttackerMods:{:.1}, AfterResistances:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
            self.config.target_label,
            action_string(&self.spells[spell].id),
            self.player.powers.attack_power,
            self.player.powers.ranged_attack_power,
            self.spell_power(spell),
            base,
            stages[0],
            stages[1],
            stages[2],
            stages[3],
            damage
        );
        self.player_log(&line);
    }

    /// Go `Spell.CritDamageMultiplier`.
    pub(crate) fn crit_multiplier(&self, spell: SpellId) -> f64 {
        let state = &self.spells[spell];
        crit_damage_multiplier(
            state.magic_defense,
            state.crit_multiplier_pct,
            self.config.crit_damage_multiplier,
            self.config.table.crit_multiplier,
            state.crit_multiplier_additive,
        )
    }

    /// Go `CalcOutcome`: an outcome with no damage, no modifiers and no debug line.
    pub(crate) fn calc_outcome(
        &mut self,
        spell: SpellId,
        target: Side,
        outcome: Outcome,
    ) -> SpellResult {
        let mut result = SpellResult {
            target,
            outcome: 0,
            damage: 0.0,
            threat: 0.0,
        };
        let binary = self.spells[spell].flags.binary;
        self.apply_outcome(spell, &mut result, binary, outcome);
        result.threat = if result.landed() {
            let state = &self.spells[spell];
            (result.damage * state.threat_multiplier + state.flat_threat_bonus)
                * self.player.threat_multiplier
        } else {
            0.0
        };
        result
    }

    /// Run an outcome applier on a result.
    fn apply_outcome(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        binary: bool,
        outcome: Outcome,
    ) {
        match outcome {
            Outcome::MagicHitAndCrit => {
                self.outcome_magic_hit_and_crit(spell, result, binary, true, true)
            }
            Outcome::MagicHit => {
                self.outcome_magic_hit_and_crit(spell, result, binary, false, true)
            }
            Outcome::MagicHitNoHitCounter => {
                self.outcome_magic_hit_and_crit(spell, result, binary, false, false)
            }
            Outcome::Tick => self.outcome_tick(spell, result, false),
            Outcome::TickMagicCrit => self.outcome_tick(spell, result, true),
            Outcome::AlwaysHitNoHitCounter => result.outcome = OUTCOME_HIT,
        }
    }

    /// Go `outcomeMagicHitAndCrit` with hit counters, or `outcomeMagicHit` without the crit
    /// roll, and without the hit counter when `count_hits` is false.
    fn outcome_magic_hit_and_crit(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        binary: bool,
        can_crit: bool,
        count_hits: bool,
    ) {
        let binary_hit = binary.then(|| 1.0 - 0.75 * self.resist(spell, true));
        let miss = spell_chance_to_miss(
            self.config.table.base_spell_miss_chance,
            binary_hit,
            self.spell_hit_chance(spell),
        );
        let target = result.target.index();
        if self.proc(1.0 - miss, "Magical Hit Roll") {
            let partial = result.outcome & OUTCOME_PARTIAL != 0;
            if can_crit && self.random("Magical Crit Roll") < self.spell_crit_chance(spell) {
                result.outcome = OUTCOME_CRIT;
                let state = &self.spells[spell];
                result.damage *= crit_damage_multiplier(
                    state.magic_defense,
                    state.crit_multiplier_pct,
                    self.config.crit_damage_multiplier,
                    self.config.table.crit_multiplier,
                    state.crit_multiplier_additive,
                );
                let metrics = &mut self.spells[spell].metrics[target];
                metrics.crits += 1;
                if partial {
                    metrics.resisted_crits += 1;
                }
            } else {
                result.outcome = OUTCOME_HIT;
                if count_hits {
                    let metrics = &mut self.spells[spell].metrics[target];
                    metrics.hits += 1;
                    if partial {
                        metrics.resisted_hits += 1;
                    }
                }
            }
        } else {
            result.outcome = OUTCOME_MISS;
            result.damage = 0.0;
            self.spells[spell].metrics[target].misses += 1;
        }
    }

    /// Go `Dot.OutcomeTick`, or `OutcomeTickMagicCrit` when the tick can crit: a tick never
    /// rolls to hit, since the dot did that when it landed.
    fn outcome_tick(&mut self, spell: SpellId, result: &mut SpellResult, can_crit: bool) {
        let partial = result.outcome & OUTCOME_PARTIAL != 0;
        let target = result.target.index();
        if can_crit && self.random("Magical Crit Roll") < self.spell_crit_chance(spell) {
            result.outcome = OUTCOME_CRIT;
            let state = &self.spells[spell];
            result.damage *= crit_damage_multiplier(
                state.magic_defense,
                state.crit_multiplier_pct,
                self.config.crit_damage_multiplier,
                self.config.table.crit_multiplier,
                state.crit_multiplier_additive,
            );
            let metrics = &mut self.spells[spell].metrics[target];
            metrics.crit_ticks += 1;
            if partial {
                metrics.resisted_crit_ticks += 1;
            }
        } else {
            result.outcome = OUTCOME_HIT;
            let metrics = &mut self.spells[spell].metrics[target];
            metrics.ticks += 1;
            if partial {
                metrics.resisted_ticks += 1;
            }
        }
    }

    /// Go `Spell.CalcAndDealPeriodicDamage` for a dot's tick on a base amount.
    pub(crate) fn periodic_damage_tick(&mut self, dot: super::DotId, base: f64) {
        let state = &self.dots[dot];
        let (spell, side, can_crit) = (state.spell, state.side, state.tick_can_crit);
        let mut base = base;
        if state.bonus_coefficient > 0.0 {
            base += state.bonus_coefficient * self.bonus_damage(spell);
        }
        let attacker =
            self.attacker_multiplier(spell, true) * self.dots[dot].periodic_damage_multiplier;
        let outcome = if can_crit {
            Outcome::TickMagicCrit
        } else {
            Outcome::Tick
        };
        let result = self.calc_damage_internal(spell, side, base, attacker, outcome);
        self.deal_damage(spell, result, true);
    }

    /// Go `Dot.CalcAndDealPeriodicSnapshotDamage` for a dot built by `Snapshot`, which ticks
    /// on the caster's current spell power and attacker multiplier.
    pub(crate) fn snapshot_dot_tick(&mut self, dot: super::DotId) {
        self.snapshot_dot_tick_result(dot);
    }

    /// [`Self::snapshot_dot_tick`], returning the tick's result as Go does.
    pub(crate) fn snapshot_dot_tick_result(&mut self, dot: super::DotId) -> SpellResult {
        let state = &self.dots[dot];
        let (spell, side, can_crit) = (state.spell, state.side, state.tick_can_crit);
        let mut base = state.snapshot_base;
        if state.reads_spell_power {
            base += state.bonus_coefficient * self.bonus_damage(spell) - state.snapshot_spell_power;
        }
        let attacker =
            self.attacker_multiplier(spell, true) * self.dots[dot].periodic_damage_multiplier;
        let outcome = if can_crit {
            Outcome::TickMagicCrit
        } else {
            Outcome::Tick
        };
        let result = self.calc_damage_internal(spell, side, base, attacker, outcome);
        self.deal_damage(spell, result, true);
        result
    }

    /// Go `Spell.TravelTime`.
    pub(crate) fn travel_time(&self, spell: SpellId) -> i64 {
        let speed = self.spells[spell].missile_speed;
        if speed == 0.0 {
            0
        } else {
            (crate::core::time::NS_PER_SECOND as f64 * self.config.distance / speed) as i64
        }
    }

    /// Go `WaitTravelTime` followed by `DealDamage` on arrival.
    pub(crate) fn deal_damage_after_travel(&mut self, spell: SpellId, result: SpellResult) {
        let at = self.now + self.travel_time(spell);
        self.schedule(
            at,
            PRIORITY_GCD,
            Action::Travel {
                spell,
                result,
                dot: None,
            },
        );
    }

    /// Go `WaitTravelTime` with a class callback, run by [`Agent::on_travel`] on arrival.
    pub(crate) fn class_after_travel(&mut self, spell: SpellId, result: SpellResult) {
        let at = self.now + self.travel_time(spell);
        self.schedule(at, PRIORITY_GCD, Action::ClassTravel { spell, result });
    }

    /// The same, then `Dot.Apply` when the hit landed.
    pub(crate) fn deal_damage_after_travel_then_dot(
        &mut self,
        spell: SpellId,
        result: SpellResult,
        dot: super::DotId,
    ) {
        let at = self.now + self.travel_time(spell);
        self.schedule(
            at,
            PRIORITY_GCD,
            Action::Travel {
                spell,
                result,
                dot: Some(dot),
            },
        );
    }

    /// Go `dealDamageInternal`.
    pub(crate) fn deal_damage(&mut self, spell: SpellId, result: SpellResult, periodic: bool) {
        let partial = result.outcome & OUTCOME_PARTIAL != 0;
        if self.now >= 0 {
            let metrics = &mut self.spells[spell].metrics[result.target.index()];
            metrics.total_damage += result.damage;
            if partial {
                metrics.total_resisted_damage += result.damage;
            }
            if periodic {
                metrics.total_tick_damage += result.damage;
                if partial {
                    metrics.total_resisted_tick_damage += result.damage;
                }
            }
            let blocked = result.outcome & OUTCOME_BLOCK != 0;
            if blocked && result.crit() {
                metrics.total_blocked_crit_damage += result.damage;
            } else if result.crit() {
                metrics.total_crit_damage += result.damage;
                if partial {
                    metrics.total_resisted_crit_damage += result.damage;
                }
                if periodic {
                    metrics.total_crit_tick_damage += result.damage;
                    if partial {
                        metrics.total_resisted_crit_tick_damage += result.damage;
                    }
                }
            } else if result.outcome & OUTCOME_GLANCE != 0 {
                metrics.total_glance_damage += result.damage;
            } else if blocked {
                metrics.total_block_damage += result.damage;
            }
            metrics.total_threat += result.threat;
        }
        if result.target == Side::Target {
            self.encounter_damage_taken += result.damage;
        }
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let label = match result.target {
                Side::Target => &self.config.target_label,
                Side::Player => &self.config.player_label,
            };
            let line = format!(
                "[{}] {}{} {} (SpellSchool: {}). (Threat: {:.3})",
                label,
                action_string(&self.spells[spell].id),
                if periodic { " tick" } else { "" },
                result.damage_string(),
                self.spells[spell].school,
                result.threat
            );
            self.player_log(&line);
        }
        if !self.spells[spell].flags.no_on_damage_dealt {
            if periodic {
                self.on_periodic_damage(spell, &result);
            } else {
                self.on_spell_hit(spell, &result);
            }
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Go `spelldata.Effect.Roll` for a spell's exported client damage effect.
    pub(crate) fn roll_damage_effect(&mut self, spell: SpellId) -> f64 {
        let (average, variance) = self.spells[spell]
            .damage_effect
            .expect("spell has a damage effect");
        if variance == 0.0 {
            return average;
        }
        let (low, high) = (
            average * (1.0 - variance / 2.0),
            average * (1.0 + variance / 2.0),
        );
        low + (high - low) * self.random("Damage Roll")
    }
}
