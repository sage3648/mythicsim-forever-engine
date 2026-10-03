//! The target swinging at the player when the player tanks it: Go attack.go's enemy auto
//! attack `ApplyEffects`, `CalcDamage` and spell_outcome.go `outcomeEnemyMeleeWhite`, and
//! attack.go `applyParryHaste` for both units.
//!
//! Every value the swing reads is exported resolved, since the gate rejects anything in scope
//! that would change one during a fight.

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
    pub(crate) values: Enemy,
    /// The target action the swing's metrics belong to.
    pub(crate) action: usize,
    pub(crate) metrics: SpellMetrics,
}

impl<A: Agent> Fight<A> {
    /// Go `Spell.Cast` of the target's main hand auto: the cast lines, then `ApplyEffects`.
    pub(crate) fn enemy_swing(&mut self) {
        let enemy = self.enemy.as_mut().expect("the target swings");
        enemy.metrics.casts += 1;
        let action = action_string(&enemy.values.action_id);
        if self.log.is_some() {
            self.unit_log(
                Side::Target,
                &format!(
                    "Casting {action} (Cost = 0.000, Cast Time = 0s, GCD = 0s, Effective Time = 0s)"
                ),
            );
            self.unit_log(Side::Target, &format!("Completed cast {action}"));
        }
        let values = self
            .enemy
            .as_ref()
            .expect("the target swings")
            .values
            .clone();
        // The stat aura combination picks the rolls, as it picks the player's powers.
        let rolls = values.rolls[self.stat_mask as usize % values.rolls.len()].clone();
        // Go Weapon.EnemyWeaponDamage.
        let spread = 1.0 + values.damage_spread * self.random("Enemy Weapon Damage");
        let weapon = values.base_damage_min
            * (spread + (values.attack_power * values.attack_power_coefficient).max(0.0));
        let base = weapon + values.bonus_damage;
        let mut result = SpellResult {
            target: Side::Player,
            outcome: 0,
            damage: base * values.attacker_multiplier,
            threat: 0.0,
        };
        let after_attacker = result.damage;
        result.damage *= rolls.armor_multiplier;
        let after_resistances = result.damage;
        result.damage += rolls.bonus_damage_taken;
        result.damage *= rolls.target_multiplier;
        let after_target = result.damage;
        self.enemy_outcome(&rolls, &mut result);
        let after_outcome = result.damage;
        result.damage = result.damage.max(0.0);
        if self.log.is_some() {
            let line = format!(
                "[{}] {action} [DEBUG] MAP: {:.1}, RAP: {:.1}, SP: {:.1}, BaseDamage:{:.1}, AfterAttackerMods:{:.1}, AfterResistances:{:.1}, AfterTargetMods:{:.1}, AfterOutcome:{:.1}, AfterPostOutcome:{:.1}",
                self.config.player_label,
                values.log_attack_power,
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
            (result.damage * values.threat_multiplier + values.flat_threat_bonus)
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
            result.damage = (result.damage - values.block_reduction).max(0.0);
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

    /// Go `auraTracker.OnSpellHitTaken` on the player for the target's swing.
    fn on_enemy_hit_taken(&mut self, result: &SpellResult) {
        let side = Side::Player;
        let list = super::aura::List::SpellHitTaken as usize;
        let length = self.trackers[side.index()].lists[list].snapshot_len();
        for position in 0..length {
            let index = self.trackers[side.index()].lists[list].read(position);
            let aura = super::AuraRef { side, index };
            if !self.aura(aura).active {
                continue;
            }
            match self.aura(aura).behavior {
                AuraBehavior::ChanceOfDeath => self.chance_of_death_hit_taken(result),
                AuraBehavior::ParryHaste => self.parry_haste(side, result),
                _ => {}
            }
        }
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
