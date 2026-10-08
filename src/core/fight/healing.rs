//! Go spell_result.go healing: `CalcAndDealHealing` with `OutcomeHealing`, and
//! `CalcAndDealPeriodicHealing` with `Dot.OutcomeTick`, on the casting player. The unit's
//! healing pseudo stats and its attack table's healing multiplier, which nothing changes in
//! scope, come with the spell's class effect.

use crate::mechanics::damage::crit_damage_multiplier;

use super::{
    damage::{OUTCOME_CRIT, OUTCOME_HIT, OUTCOME_LANDED},
    log::action_string,
    Agent, DotId, Fight, Side, SpellId, SpellResult,
};

/// The healing modifiers Go reads off the units for a heal on the player.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Healing {
    /// Go `PseudoStats.HealingDealtMultiplier` of the caster.
    pub(crate) dealt_multiplier: f64,
    /// Go `PseudoStats.HealingTakenMultiplier` of the player healed.
    pub(crate) taken_multiplier: f64,
    /// Go `AttackTable.HealingDealtMultiplier` of the caster against the player.
    pub(crate) table_multiplier: f64,
    /// Go `Spell.HealingPower` for the debug line.
    pub(crate) healing_power: f64,
}

impl<A: Agent> Fight<A> {
    /// Go `CalcAndDealHealing` with `OutcomeHealingCrit` on the player or the target: the
    /// bonus coefficient on the caster's healing power and the unit's bonus healing taken,
    /// the caster and unit multipliers, the healing crit roll on spell crit, then
    /// `DealHealing`'s metrics, the health a unit with a health bar gains, the log line and
    /// the caster's `OnHealDealt`. Only the player has a health bar in scope.
    pub(crate) fn calc_and_deal_healing(
        &mut self,
        spell: SpellId,
        target: Side,
        base_healing: f64,
        healing: Healing,
        bonus_healing_taken: f64,
    ) -> SpellResult {
        let state = &self.spells[spell];
        let caster = state.caster;
        let healing_power = self.unit(caster).powers.healing_power + bonus_healing_taken;
        let mut base = base_healing;
        if state.bonus_coefficient > 0.0 {
            base += state.bonus_coefficient * healing_power;
        }
        let multiplier = if state.flags.ignore_attacker_modifiers {
            1.0
        } else {
            state.damage_multiplier
                * state.damage_multiplier_additive
                * (healing.dealt_multiplier * self.healing_dealt_factor)
        };
        let mut amount = base * multiplier;
        let after_caster = amount;
        if !state.flags.ignore_target_modifiers {
            amount = amount * healing.taken_multiplier * healing.table_multiplier;
        }
        let after_target = amount;
        // Go outcomeHealingCrit: spell crit and the spell's bonus, without an attack table.
        let chance = (self.unit(caster).powers.spell_crit_percent
            + self.spells[spell].bonus_crit_percent)
            / 100.0;
        let index = target.index();
        let outcome = if self.random("Healing Crit Roll") < chance {
            let state = &self.spells[spell];
            amount *= crit_damage_multiplier(
                state.magic_defense,
                state.crit_multiplier_pct,
                self.unit_config(caster).crit_damage_multiplier,
                1.0,
                state.crit_multiplier_additive,
            );
            self.spells[spell].metrics[index].crits += 1;
            OUTCOME_CRIT
        } else {
            self.spells[spell].metrics[index].hits += 1;
            OUTCOME_HIT
        };
        let after_outcome = amount;
        let amount = amount.max(0.0);
        if self.log.is_some() {
            let line = format!(
                "[{}] {} [DEBUG] HealingPower: {:.1}, BaseHealing:{:.1}, AfterCasterMods:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
                self.label_of(target),
                action_string(&self.spells[spell].id),
                healing_power,
                base,
                after_caster,
                after_target,
                after_outcome,
                amount
            );
            self.unit_log(caster, &line);
        }
        let mut result = SpellResult {
            armor_multiplier: 0.0,
            target,
            attacker: self.spells[spell].caster,
            outcome,
            damage: amount,
            threat: 0.0,
        };
        result.threat = self.threat_of(spell, &result);
        // Go dealHealingInternal.
        let metrics = &mut self.spells[spell].metrics[index];
        if outcome == OUTCOME_CRIT {
            metrics.total_crit_healing += amount;
        }
        metrics.total_healing += amount;
        metrics.total_threat += result.threat;
        if target == Side::Player {
            let health = match self.spells[spell].self_health_metrics {
                Some(index) => index,
                None => {
                    let index = self.new_health_metrics(self.spells[spell].id.clone());
                    self.spells[spell].self_health_metrics = Some(index);
                    index
                }
            };
            self.gain_health(amount, health);
        }
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let line = format!(
                "[{}] {} {} for {amount:.3} healing. (Threat: {:.3})",
                self.label_of(target),
                action_string(&self.spells[spell].id),
                if outcome == OUTCOME_CRIT {
                    "Crit"
                } else {
                    "Hit"
                },
                result.threat
            );
            self.unit_log(caster, &line);
        }
        self.on_heal_dealt(spell, &result);
        result
    }

    /// Go `CalcAndDealHealing` on the player with `OutcomeHealingCrit`: the spell's coefficient
    /// on healing power, the caster and target multipliers, a crit roll on spell crit with a
    /// magic crit's multiplier and no attack table, then `DealHealing`'s metrics, health gain on
    /// the spell's own health metrics and log line.
    pub(crate) fn calc_and_deal_self_healing_crit(
        &mut self,
        spell: SpellId,
        base: f64,
        healing: Healing,
    ) {
        let state = &self.spells[spell];
        let mut base = base;
        if state.bonus_coefficient > 0.0 {
            base += state.bonus_coefficient * healing.healing_power;
        }
        let caster = if state.flags.ignore_attacker_modifiers {
            1.0
        } else {
            state.damage_multiplier
                * state.damage_multiplier_additive
                * (healing.dealt_multiplier * self.healing_dealt_factor)
        };
        let mut amount = base * caster;
        let after_caster = amount;
        if !state.flags.ignore_target_modifiers {
            amount = amount * healing.taken_multiplier * healing.table_multiplier;
        }
        let after_target = amount;
        // Go OutcomeHealingCrit: spell crit and the spell's bonus, no suppression or table.
        let target = Side::Player.index();
        let chance =
            (self.unit(state.caster).powers.spell_crit_percent + state.bonus_crit_percent) / 100.0;
        let crit = self.random("Healing Crit Roll") < chance;
        let outcome = if crit {
            let state = &self.spells[spell];
            let base_multiplier = if state.magic_defense { 1.5 } else { 2.0 };
            let unit = self.unit_config(state.caster).crit_damage_multiplier;
            amount *= (base_multiplier * state.crit_multiplier_pct * unit - 1.0)
                * (state.crit_multiplier_additive + 1.0)
                + 1.0;
            self.spells[spell].metrics[target].crits += 1;
            OUTCOME_CRIT
        } else {
            self.spells[spell].metrics[target].hits += 1;
            OUTCOME_HIT
        };
        let after_outcome = amount;
        let amount = amount.max(0.0);
        if self.log.is_some() {
            let line = format!(
                "[{}] {} [DEBUG] HealingPower: {:.1}, BaseHealing:{:.1}, AfterCasterMods:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
                self.config.player_label,
                action_string(&self.spells[spell].id),
                healing.healing_power,
                base,
                after_caster,
                after_target,
                after_outcome,
                amount
            );
            self.player_log(&line);
        }
        let result = SpellResult {
            armor_multiplier: 0.0,
            target: Side::Player,
            attacker: self.spells[spell].caster,
            outcome,
            damage: amount,
            threat: 0.0,
        };
        let threat = self.threat_of(spell, &result);
        // Go dealHealingInternal.
        let metrics = &mut self.spells[spell].metrics[target];
        if crit {
            metrics.total_crit_healing += amount;
        }
        metrics.total_healing += amount;
        metrics.total_threat += threat;
        let health = match self.spells[spell].self_health_metrics {
            Some(index) => index,
            None => {
                let index = self.new_health_metrics(self.spells[spell].id.clone());
                self.spells[spell].self_health_metrics = Some(index);
                index
            }
        };
        self.gain_health(amount, health);
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let line = format!(
                "[{}] {} {} for {amount:.3} healing. (Threat: {threat:.3})",
                self.config.player_label,
                action_string(&self.spells[spell].id),
                if crit { "Crit" } else { "Hit" },
            );
            self.player_log(&line);
        }
    }

    /// Go `CalcAndDealHealing` on the player with `OutcomeHealing`: no bonus coefficient in
    /// scope, the caster and target multipliers, a counted hit, then `DealHealing`'s metrics,
    /// health gain on the spell's own health metrics and log line.
    pub(crate) fn calc_and_deal_self_healing(
        &mut self,
        spell: SpellId,
        base: f64,
        healing: Healing,
    ) {
        let result = self.self_healing(spell, base, healing, None);
        // Go DealHealing: the caster's OnHealDealt.
        self.on_heal_dealt(spell, &result);
    }

    /// Go `CalcAndDealPeriodicHealing` on the player with `Dot.OutcomeTick`: the caster's
    /// multipliers include its periodic healing dealt, the tick counts, and the log line names
    /// the tick.
    pub(crate) fn periodic_self_healing_tick(
        &mut self,
        dot: DotId,
        base: f64,
        healing: Healing,
        periodic_dealt_multiplier: f64,
    ) {
        let spell = self.dots[dot].spell;
        self.self_healing(spell, base, healing, Some(periodic_dealt_multiplier));
    }

    /// The shared heal on the player; `periodic` holds Go
    /// `PseudoStats.PeriodicHealingDealtMultiplier` for a hot's tick.
    fn self_healing(
        &mut self,
        spell: SpellId,
        base: f64,
        healing: Healing,
        periodic: Option<f64>,
    ) -> SpellResult {
        assert!(
            self.spells[spell].bonus_coefficient == 0.0,
            "healing spell power is not supported"
        );
        let state = &self.spells[spell];
        let caster = if state.flags.ignore_attacker_modifiers {
            1.0
        } else {
            let multiplier = state.damage_multiplier
                * state.damage_multiplier_additive
                * (healing.dealt_multiplier * self.healing_dealt_factor);
            match periodic {
                Some(periodic) => multiplier * periodic,
                None => multiplier,
            }
        };
        let mut amount = base * caster;
        let after_caster = amount;
        if !state.flags.ignore_target_modifiers {
            amount = amount * healing.taken_multiplier * healing.table_multiplier;
        }
        let after_target = amount;
        // Go OutcomeHealing, or Dot.OutcomeTick for a hot.
        let target = Side::Player.index();
        if periodic.is_some() {
            self.spells[spell].metrics[target].ticks += 1;
        } else {
            self.spells[spell].metrics[target].hits += 1;
        }
        let after_outcome = amount;
        let amount = amount.max(0.0);
        if self.log.is_some() {
            let line = format!(
                "[{}] {} [DEBUG] HealingPower: {:.1}, BaseHealing:{:.1}, AfterCasterMods:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
                self.config.player_label,
                action_string(&self.spells[spell].id),
                healing.healing_power,
                base,
                after_caster,
                after_target,
                after_outcome,
                amount
            );
            self.player_log(&line);
        }
        let mut result = SpellResult {
            armor_multiplier: 0.0,
            target: Side::Player,
            attacker: self.spells[spell].caster,
            outcome: OUTCOME_HIT,
            damage: amount,
            threat: 0.0,
        };
        let threat = self.threat_of(spell, &result);
        result.threat = threat;
        debug_assert!(result.outcome & OUTCOME_LANDED != 0);
        // Go dealHealingInternal.
        let metrics = &mut self.spells[spell].metrics[target];
        metrics.total_healing += amount;
        metrics.total_threat += threat;
        let health = match self.spells[spell].self_health_metrics {
            Some(index) => index,
            None => {
                let index = self.new_health_metrics(self.spells[spell].id.clone());
                self.spells[spell].self_health_metrics = Some(index);
                index
            }
        };
        self.gain_health(amount, health);
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let line = format!(
                "[{}] {}{} Hit for {amount:.3} healing. (Threat: {threat:.3})",
                self.config.player_label,
                action_string(&self.spells[spell].id),
                if periodic.is_some() { " tick" } else { "" },
            );
            self.player_log(&line);
        }
        result
    }
}
