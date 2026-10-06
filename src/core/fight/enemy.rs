//! The target swinging at the player when the player tanks it: Go attack.go's enemy auto
//! attack `ApplyEffects`, `CalcDamage` and spell_outcome.go `outcomeEnemyMeleeWhite`, and
//! attack.go `applyParryHaste` for both units.
//!
//! Every value the swing reads is exported resolved, since the gate rejects anything in scope
//! that would change one during a fight.

use std::rc::Rc;

use crate::contracts::prepared_v2::{Enemy, EnemyRolls};

use super::{
    damage::{
        OUTCOME_BLOCK, OUTCOME_CRIT, OUTCOME_CRUSH, OUTCOME_DODGE, OUTCOME_HIT, OUTCOME_MISS,
        OUTCOME_PARRY,
    },
    log::action_string,
    melee::Hand,
    Agent, AuraBehavior, Fight, Side, SpellMetrics, SpellResult,
};
use crate::core::time::go_string;

/// The target's main hand swing and its metrics against the player.
#[derive(Clone, Debug)]
pub(crate) struct EnemyAttack {
    /// Shared, so each swing reads them without a copy.
    pub(crate) values: Rc<Enemy>,
    /// The target action the swing's metrics belong to.
    pub(crate) action: usize,
    pub(crate) metrics: SpellMetrics,
    /// Go `PseudoStats.MeleeSpeedMultiplier` of the target, which a slow multiplies and each
    /// reset restores.
    pub(crate) melee_speed_multiplier: f64,
    /// Target auras that change only the swing's attack power, with its attack power and the
    /// debug line's MAP while active. The gate admits at most one an effect can activate.
    pub(crate) attack_power_auras: Vec<(super::AuraRef, f64, f64)>,
}

impl<A: Agent> Fight<A> {
    /// Go `Spell.Cast` of the target's main hand auto: the cast lines, then `ApplyEffects`.
    pub(crate) fn enemy_swing(&mut self) {
        let enemy = self.enemy.as_mut().expect("the target swings");
        enemy.metrics.casts += 1;
        if self.log.is_some() {
            let action = action_string(&enemy.values.action_id);
            self.unit_log(
                Side::Target,
                &format!(
                    "Casting {action} (Cost = 0.000, Cast Time = 0s, GCD = 0s, Effective Time = 0s)"
                ),
            );
            self.unit_log(Side::Target, &format!("Completed cast {action}"));
        }
        let enemy = self.enemy.as_ref().expect("the target swings");
        let values = Rc::clone(&enemy.values);
        let (mut attack_power, mut log_attack_power) =
            (values.attack_power, values.log_attack_power);
        if let Some(&(_, aura_attack_power, aura_log_attack_power)) = enemy
            .attack_power_auras
            .iter()
            .find(|(aura, _, _)| self.aura(*aura).active)
        {
            attack_power = aura_attack_power;
            log_attack_power = aura_log_attack_power;
        }
        // The stat aura combination picks the rolls, as it picks the player's powers.
        // A hardcast holds the tank's reduced avoidance aura.
        let table = if self.player.reduced_avoidance && !values.reduced_avoidance_rolls.is_empty() {
            &values.reduced_avoidance_rolls
        } else {
            &values.rolls
        };
        let rolls = table[self.stat_mask as usize % table.len()].clone();
        // Go Weapon.EnemyWeaponDamage.
        // The arm64 build fuses 1 + spread * roll.
        let spread = values
            .damage_spread
            .mul_add(self.random("Enemy Weapon Damage"), 1.0);
        let weapon = values.base_damage_min
            * (spread + (attack_power * values.attack_power_coefficient).max(0.0));
        let base = weapon + values.bonus_damage;
        let mut result = SpellResult {
            armor_multiplier: 0.0,
            target: Side::Player,
            outcome: 0,
            damage: base * values.attacker_multiplier,
            threat: 0.0,
        };
        let after_attacker = result.damage;
        result.damage *= rolls.armor_multiplier;
        let after_resistances = result.damage;
        // Go SpellResult's PostArmorAndResistanceMultiplier and ArmorAndResistanceMultiplier,
        // which rage from damage taken reads.
        self.player_hit_resistance = (after_resistances, rolls.armor_multiplier);
        result.damage += rolls.bonus_damage_taken;
        // Go TargetDamageMultiplier: the player's live damage taken multiplier, then the
        // school's and the attack table's. The physical school's is the player's live one,
        // which racial survival auras multiply, while the rolls carry the reset's value.
        let physical = super::school_index(1);
        let target_multiplier = match (
            rolls.school_damage_taken_multiplier,
            rolls.table_damage_taken_multiplier,
        ) {
            (Some(school), Some(table)) => {
                let school = if school == self.config.school_damage_taken_multiplier[physical] {
                    self.player.school_damage_taken_multiplier[physical]
                } else {
                    school
                };
                self.player.damage_taken_multiplier * school * table
            }
            _ => rolls.target_multiplier,
        };
        result.damage *= target_multiplier;
        let after_target = result.damage;
        self.enemy_outcome(&rolls, &mut result);
        let after_outcome = result.damage;
        // Go ApplyPostOutcomeDamageModifiers: the player's dynamic damage taken modifiers, as
        // an absorb shield, then the floor.
        // Go registers the item shields before the class's modifiers.
        self.item_absorbs_taken(values.school, &mut result);
        A::player_damage_taken_modifiers(self, &mut result);
        result.damage = result.damage.max(0.0);
        if self.log.is_some() {
            let action = action_string(&values.action_id);
            let line = format!(
                "[{}] {action} [DEBUG] MAP: {:.1}, RAP: {:.1}, SP: {:.1}, BaseDamage:{:.1}, AfterAttackerMods:{:.1}, AfterResistances:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
                self.config.player_label,
                log_attack_power,
                values.log_ranged_attack_power,
                values.log_spell_power,
                base,
                after_attacker,
                after_resistances,
                after_target,
                after_outcome,
                result.damage
            );
            self.unit_log(Side::Target, &line);
        }
        result.threat = if result.landed() {
            result
                .damage
                .mul_add(values.threat_multiplier, values.flat_threat_bonus)
                * values.unit_threat_multiplier
        } else {
            0.0
        };
        self.enemy_deal_damage(&values, result);
    }

    /// Go `outcomeEnemyMeleeWhite`: one roll against the running sum of the table's steps.
    fn enemy_outcome(&mut self, values: &EnemyRolls, result: &mut SpellResult) {
        let roll = self.random("Enemy White Hit Table");
        let metrics = &mut self.enemy.as_mut().expect("the target swings").metrics;
        let mut chance = values.miss_chance;
        if roll < chance {
            result.outcome = OUTCOME_MISS;
            metrics.misses += 1;
            result.damage = 0.0;
            return;
        }
        chance += values.dodge_chance;
        if roll < chance {
            result.outcome = OUTCOME_DODGE;
            metrics.dodges += 1;
            result.damage = 0.0;
            return;
        }
        chance += values.parry_chance;
        if roll < chance {
            result.outcome = OUTCOME_PARRY;
            metrics.parries += 1;
            result.damage = 0.0;
            return;
        }
        chance += values.block_chance;
        if roll < chance {
            result.outcome |= OUTCOME_BLOCK;
            metrics.blocks += 1;
            // Go fuses the block value times its multiplier into the subtraction.
            result.damage = match (values.block_value, values.block_value_multiplier) {
                (Some(value), Some(multiplier)) => (-value).mul_add(multiplier, result.damage),
                _ => result.damage - values.block_reduction,
            }
            .max(0.0);
            return;
        }
        chance += values.crit_chance;
        if roll < chance {
            result.outcome = OUTCOME_CRIT;
            metrics.crits += 1;
            result.damage *= 2.0;
            return;
        }
        chance += values.crush_chance;
        if roll < chance {
            result.outcome = OUTCOME_CRUSH;
            metrics.crushes += 1;
            result.damage *= 1.5;
            return;
        }
        result.outcome = OUTCOME_HIT;
        metrics.hits += 1;
    }

    /// Go `dealDamageInternal` for the target's swing, then the player's `OnSpellHitTaken`.
    /// The gate rejects any target listener of the target's own hits.
    fn enemy_deal_damage(&mut self, values: &Enemy, result: SpellResult) {
        let metrics = &mut self.enemy.as_mut().expect("the target swings").metrics;
        metrics.total_damage += result.damage;
        let blocked = result.outcome & OUTCOME_BLOCK != 0;
        if blocked && result.crit() {
            metrics.total_blocked_crit_damage += result.damage;
        } else if result.crit() {
            metrics.total_crit_damage += result.damage;
        } else if blocked {
            metrics.total_block_damage += result.damage;
        } else if result.outcome & OUTCOME_CRUSH != 0 {
            metrics.total_crush_damage += result.damage;
        }
        metrics.total_threat += result.threat;
        if self.log.is_some() {
            let line = format!(
                "[{}] {} {} (SpellSchool: {}). (Threat: {:.3})",
                self.config.player_label,
                action_string(&values.action_id),
                result.damage_string(),
                values.school,
                result.threat
            );
            self.unit_log(Side::Target, &line);
        }
        self.on_enemy_hit_taken(&result);
    }

    /// The player's `BlockDamageReduction`, which the target's rolls carry for the current
    /// stat aura combination.
    pub(crate) fn player_block_damage_reduction(&self) -> f64 {
        let enemy = self.enemy.as_ref().expect("the target swings");
        let rolls = &enemy.values.rolls;
        rolls[self.stat_mask as usize % rolls.len()].block_reduction
    }

    /// Go `TotalMeleeHasteMultiplier` of the target: its attack speed, its live melee speed
    /// and its haste rating term, or the value at reset when the factors are not exported.
    pub(crate) fn enemy_melee_haste(&self) -> f64 {
        let enemy = self.enemy.as_ref().expect("the target swings");
        let values = &enemy.values;
        match (
            values.attack_speed_multiplier,
            values.melee_haste_rating_multiplier,
        ) {
            (Some(attack), Some(rating)) => attack * enemy.melee_speed_multiplier * rating,
            _ => values.melee_haste_multiplier,
        }
    }

    /// Go `Unit.MultiplyMeleeSpeed` on the target, then its `AutoAttacks.UpdateSwingTimers`:
    /// the rest of a pending swing scales with the change in speed.
    pub(crate) fn multiply_enemy_melee_speed(&mut self, amount: f64) {
        // A target that does not swing at the player has no swing to slow.
        let Some(enemy) = self.enemy.as_mut() else {
            return;
        };
        enemy.melee_speed_multiplier *= amount;
        if !self.autos.enemy.enabled {
            return;
        }
        let haste = self.enemy_melee_haste();
        let now = self.now;
        let attack = &mut self.autos.enemy;
        let old = attack.cur_swing_speed();
        attack.set_swing_speed(haste);
        let factor = old / attack.cur_swing_speed();
        let remaining = attack.swing_at - now;
        if remaining > 0 {
            attack.swing_at = now + (remaining as f64 * factor) as i64;
        }
        let swing_at = attack.swing_at;
        self.autos.min_time = self.autos.min_time.min(swing_at);
    }

    /// Go `AutoAttacks.PauseMeleeBy` on the target that swings at the player: no swing lands
    /// before the pause ends.
    pub(crate) fn pause_enemy_melee_by(&mut self, pause: i64) {
        if self.enemy.is_none() {
            return;
        }
        let resume = self.now + pause;
        let attack = &mut self.autos.enemy;
        if attack.swing_at < resume {
            attack.swing_at = resume;
            self.autos.min_time = self.autos.min_time.min(resume);
        }
    }

    /// Go `AutoAttacks.ResumeMeleeAt` on the target: the unused part of a pause is undone, the
    /// swing moving back to the time it had, never into the past.
    pub(crate) fn resume_enemy_melee_at(&mut self, swing_at: i64) {
        if self.enemy.is_none() {
            return;
        }
        let resume = self.now.max(swing_at);
        let attack = &mut self.autos.enemy;
        if resume < attack.swing_at {
            attack.swing_at = resume;
            self.autos.min_time = self.autos.min_time.min(resume);
        }
    }

    /// Go `auraTracker.OnSpellHitTaken` on the player for the target's swing.
    fn on_enemy_hit_taken(&mut self, result: &SpellResult) {
        let side = Side::Player;
        let list = super::aura::List::SpellHitTaken as usize;
        let snapshot = self.trackers[side.index()].lists[list].snapshot();
        for position in 0..snapshot.len {
            let index = self.trackers[side.index()].lists[list].read(snapshot, position);
            let aura = super::AuraRef { side, index };
            if !self.aura(aura).active {
                continue;
            }
            match self.aura(aura).behavior {
                AuraBehavior::ChanceOfDeath => self.chance_of_death_hit_taken(result),
                AuraBehavior::ParryHaste => self.parry_haste(side, result),
                AuraBehavior::PushbackTrigger { chance } => self.pushback_hit_taken(chance, result),
                AuraBehavior::Class(kind) => A::on_enemy_hit_taken(self, aura, kind, result),
                AuraBehavior::RageBar => self.rage_bar_hit_taken(result),
                // The target's swing is a melee hit, which Immolation hears.
                AuraBehavior::SulfurasImmolation => self.sulfuras_immolation(aura, result),
                // The swing is a melee auto attack without the proc flag, which a struck proc
                // hears.
                AuraBehavior::SpellDataDamageProc(proc) if self.damage_procs[proc].struck => {
                    self.damage_proc_callback(aura, proc, None, result)
                }
                AuraBehavior::SpellDataStatProc(proc) if self.spell_stat_procs[proc].struck => {
                    self.spell_stat_proc_callback(aura, proc, None, Some(result))
                }
                // And a melee auto attack, which an absorb proc's melee mask hears.
                AuraBehavior::AbsorbProc(proc) => self.absorb_proc_callback(aura, proc, result),
                _ => {}
            }
        }
    }

    /// The "Pushback trigger" proc's `OnSpellHitTaken` for the target's swing: a landed hit that
    /// deals damage during a hardcast with the pushback flag queues the handler a spell batch
    /// window later. The swing is neither a channel's hit nor a dot's.
    fn pushback_hit_taken(&mut self, chance: f64, result: &SpellResult) {
        let hardcast = self.player.hardcast;
        if !result.landed()
            || result.damage == 0.0
            || hardcast.expires <= self.now
            || !hardcast.pushback
        {
            return;
        }
        self.schedule(
            self.now + super::SPELL_BATCH_WINDOW,
            super::PRIORITY_DOT,
            super::Action::Pushback { chance },
        );
    }

    /// Go `applyParryHaste`'s `OnSpellHitTaken`: a parry pulls the parrying unit's next main
    /// hand swing in by up to 40% of its swing time, leaving at least 20% of it.
    pub(crate) fn parry_haste(&mut self, side: Side, result: &SpellResult) {
        if result.outcome & OUTCOME_PARRY == 0 {
            return;
        }
        let hand = match side {
            Side::Player => Hand::Main,
            Side::Target => Hand::Enemy,
            Side::Pet(_) => unreachable!("the target never swings at a pet in scope"),
            Side::Extra(_) => unreachable!("only the first target can be tanked"),
        };
        let now = self.now;
        let attack = self.autos.attack(hand);
        let remaining = attack.swing_at - now;
        let swing_speed = attack.cur_swing_duration;
        let min_remaining = (swing_speed as f64 * 0.2) as i64;
        let default_reduction = min_remaining * 2;
        if remaining <= min_remaining {
            return;
        }
        let reduction = default_reduction.min(remaining - min_remaining);
        let new_ready_at = attack.swing_at - reduction;
        attack.swing_at = new_ready_at;
        if self.log.is_some() {
            self.unit_log(
                side,
                &format!(
                    "MH Swing reduced by {} due to parry haste, will now occur at {}",
                    go_string(reduction),
                    go_string(new_ready_at)
                ),
            );
        }
        // Go rescheduleWeaponAttack.
        self.autos.min_time = self.autos.min_time.min(new_ready_at);
    }

    /// Go `addSpellMetrics` for the target's swing at the end of an iteration.
    pub(crate) fn enemy_done_iteration(&mut self) {
        let Some(enemy) = self.enemy.as_mut() else {
            return;
        };
        let metrics = std::mem::take(&mut enemy.metrics);
        let action = enemy.action;
        let totals = &mut self.target_actions[action].targets[Side::Player.index()];
        totals.casts += metrics.casts;
        totals.misses += metrics.misses;
        totals.dodges += metrics.dodges;
        totals.parries += metrics.parries;
        totals.blocks += metrics.blocks;
        totals.blocked_crits += metrics.blocked_crits;
        totals.hits += metrics.hits;
        totals.crits += metrics.crits;
        totals.crushes += metrics.crushes;
        totals.damage += metrics.total_damage;
        totals.crit_damage += metrics.total_crit_damage;
        totals.block_damage += metrics.total_block_damage;
        totals.blocked_crit_damage += metrics.total_blocked_crit_damage;
        totals.crush_damage += metrics.total_crush_damage;
        totals.threat += metrics.total_threat;
        self.totals.player_dtps.total += metrics.total_damage;
        self.totals.target_dps.total += metrics.total_damage;
        self.totals.target_threat.total += metrics.total_threat;
    }
}
