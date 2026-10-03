//! Go attack.go and the physical half of spell_outcome.go and spell_resistances.go: the
//! player's main and off hand swings, weapon damage, armor and the physical attack table.
//!
//! Go runs weapon attacks outside the pending-action queue: before each action, every swing
//! due at or before it fires first. The step loop mirrors that through
//! [`Fight::due_weapon_attack`].

use crate::contracts::prepared_v2::Weapon;

use super::{
    damage::{
        OUTCOME_BLOCK, OUTCOME_CRIT, OUTCOME_DODGE, OUTCOME_GLANCE, OUTCOME_HIT, OUTCOME_MISS,
        OUTCOME_PARRY, OUTCOME_PARTIAL,
    },
    Agent, Fight, Side, SpellId, SpellResult,
};
use crate::core::time::NEVER_EXPIRES;

/// Go `MaxMeleeRange`.
const MAX_MELEE_RANGE: f64 = 5.0;

/// Go `PhysicalHasteRatingPerHastePercent`.
const PHYSICAL_HASTE_RATING_PER_PERCENT: f64 = 10.0;

/// Which weapon a swing uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hand {
    Main,
    Off,
    /// The target's main hand, when it swings at the player.
    Enemy,
}

/// Go `WeaponAttack`.
#[derive(Clone, Debug, Default)]
pub(crate) struct WeaponAttack {
    pub(crate) weapon: Weapon,
    pub(crate) spell: Option<SpellId>,
    pub(crate) swing_at: i64,
    pub(crate) previous_swing: i64,
    pub(crate) natural_ready_at: i64,
    cur_swing_speed: f64,
    pub(crate) cur_swing_duration: i64,
    pub(crate) enabled: bool,
}

impl WeaponAttack {
    /// Go `updateSwingDuration`.
    fn update_swing_duration(&mut self, speed: f64) {
        self.cur_swing_speed = speed;
        self.cur_swing_duration = duration_from_seconds(self.weapon.swing_speed / speed);
    }
}

/// Go `DurationFromSeconds`.
fn duration_from_seconds(seconds: f64) -> i64 {
    (crate::core::time::NS_PER_SECOND as f64 * seconds) as i64
}

/// Go `AutoAttacks` with the simulation's weapon attack list.
#[derive(Clone, Debug, Default)]
pub(crate) struct AutoAttacks {
    pub(crate) melee: bool,
    pub(crate) dual_wielding: bool,
    pub(crate) mh: WeaponAttack,
    pub(crate) oh: WeaponAttack,
    pub(crate) enemy: WeaponAttack,
    /// Go `sim.weaponAttacks`, in the order swings were added.
    attacks: Vec<Hand>,
    /// Go `sim.minWeaponAttackTime`.
    pub(crate) min_time: i64,
}

impl AutoAttacks {
    pub(crate) fn attack(&mut self, hand: Hand) -> &mut WeaponAttack {
        match hand {
            Hand::Main => &mut self.mh,
            Hand::Off => &mut self.oh,
            Hand::Enemy => &mut self.enemy,
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Go `TotalMeleeHasteMultiplier`.
    pub(crate) fn melee_haste_multiplier(&self) -> f64 {
        self.player.attack_speed_multiplier
            * self.player.melee_speed_multiplier
            * (1.0 + self.config.melee_haste_rating / (PHYSICAL_HASTE_RATING_PER_PERCENT * 100.0))
    }

    /// Go `Unit.MultiplyMeleeSpeed`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn multiply_melee_speed(&mut self, amount: f64) {
        self.player.melee_speed_multiplier *= amount;
        self.update_swing_timers();
    }

    /// Go `Unit.MultiplyAttackSpeed`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn multiply_attack_speed(&mut self, amount: f64) {
        self.player.attack_speed_multiplier *= amount;
        self.update_swing_timers();
    }

    /// Go `AutoAttacks.UpdateSwingTimers`: the remaining part of each pending swing scales with
    /// the change in speed, the off hand by the main hand's factor.
    pub(crate) fn update_swing_timers(&mut self) {
        if !(self.autos.mh.enabled || self.autos.oh.enabled) || !self.autos.melee {
            return;
        }
        if !self.autos.mh.enabled {
            return;
        }
        let haste = self.melee_haste_multiplier();
        let now = self.now;
        let old = self.autos.mh.cur_swing_speed;
        self.autos.mh.update_swing_duration(haste);
        let factor = old / self.autos.mh.cur_swing_speed;
        let remaining = self.autos.mh.swing_at - now;
        if remaining > 0 {
            self.autos.mh.swing_at = now + (remaining as f64 * factor) as i64;
        }
        self.autos.min_time = self.autos.min_time.min(self.autos.mh.swing_at);
        if self.autos.dual_wielding && self.autos.oh.enabled {
            self.autos.oh.update_swing_duration(haste);
            let remaining = self.autos.oh.swing_at - now;
            if remaining > 0 {
                self.autos.oh.swing_at = now + (remaining as f64 * factor) as i64;
            }
            self.autos.min_time = self.autos.min_time.min(self.autos.oh.swing_at);
        }
    }

    /// Go `AutoAttacks.reset` and the simulation's weapon attack reset.
    pub(crate) fn reset_auto_attacks(&mut self) {
        self.autos.attacks.clear();
        self.autos.min_time = NEVER_EXPIRES;
        if !self.autos.melee {
            return;
        }
        let haste = self.melee_haste_multiplier();
        let autos = &mut self.autos;
        for attack in [&mut autos.mh, &mut autos.oh] {
            attack.enabled = false;
            attack.previous_swing = -NEVER_EXPIRES;
            attack.swing_at = NEVER_EXPIRES;
        }
        autos.mh.update_swing_duration(haste);
        autos.mh.previous_swing = -autos.mh.cur_swing_duration;
        autos.mh.swing_at = 0;
        autos.mh.natural_ready_at = 0;
        if autos.dual_wielding {
            autos.oh.update_swing_duration(haste);
            autos.oh.previous_swing = -autos.oh.cur_swing_duration;
            autos.oh.swing_at = duration_from_seconds(autos.oh.weapon.swing_speed / 2.0);
        }
    }

    /// Go `RandomizeMeleeTiming` at encounter start: delay the first swings by a random whole
    /// number of milliseconds below the reaction time, unless out of melee range.
    pub(crate) fn randomize_melee_timing(&mut self) {
        if !self.autos.melee || self.config.distance > MAX_MELEE_RANGE {
            return;
        }
        let reaction_ms = self.config.reaction / crate::core::time::NS_PER_MILLISECOND;
        let roll = self.random("Melee Timing");
        let delay = (roll * reaction_ms as f64) as i64 * crate::core::time::NS_PER_MILLISECOND;
        // Go DelayMeleeBy.
        if delay <= 0 {
            return;
        }
        self.autos.mh.swing_at += delay;
        self.autos.min_time = self.autos.min_time.min(self.autos.mh.swing_at);
        if self.autos.dual_wielding {
            self.autos.oh.swing_at += delay;
            self.autos.min_time = self.autos.min_time.min(self.autos.oh.swing_at);
        }
    }

    /// Go `AutoAttacks.startPull` for the target, whose swing is added before the player's.
    pub(crate) fn start_enemy_attack(&mut self) {
        let Some(enemy) = &self.enemy else {
            return;
        };
        let haste = enemy.values.melee_haste_multiplier;
        let attack = &mut self.autos.enemy;
        attack.enabled = true;
        attack.update_swing_duration(haste);
        let swing_at = attack.swing_at;
        self.autos.attacks.push(Hand::Enemy);
        self.autos.min_time = self.autos.min_time.min(swing_at);
    }

    /// Go `AutoAttacks.reset` for the target: its main hand opens at a random point of its
    /// swing timer, from the roll Go draws at the target's reset.
    pub(crate) fn reset_enemy_attack(&mut self, roll: f64) {
        let Some(enemy) = &self.enemy else {
            return;
        };
        let (speed, haste) = (
            enemy.values.swing_speed,
            enemy.values.melee_haste_multiplier,
        );
        let attack = &mut self.autos.enemy;
        attack.weapon.swing_speed = speed;
        attack.enabled = false;
        attack.update_swing_duration(haste);
        attack.previous_swing = -attack.cur_swing_duration;
        attack.swing_at = 0;
        attack.natural_ready_at = 0;
        let offset = (roll * attack.cur_swing_duration as f64) as i64;
        attack.previous_swing += offset;
        attack.swing_at += offset;
        attack.natural_ready_at += offset;
    }

    /// Go `AutoAttacks.startPull`: the off hand is added first.
    pub(crate) fn start_auto_attacks(&mut self) {
        if !self.autos.melee {
            return;
        }
        let haste = self.melee_haste_multiplier();
        let distance = self.config.distance;
        let in_range = |weapon: &Weapon| {
            (weapon.min_range == 0.0 || weapon.min_range < distance)
                && (weapon.max_range == 0.0 || weapon.max_range >= distance)
        };
        if self.autos.mh.swing_at == NEVER_EXPIRES {
            self.autos.mh.swing_at = 0;
            self.autos.mh.natural_ready_at = 0;
        }
        let hands: &[Hand] = if self.autos.dual_wielding {
            &[Hand::Off, Hand::Main]
        } else {
            &[Hand::Main]
        };
        for &hand in hands {
            let attack = self.autos.attack(hand);
            if !in_range(&attack.weapon) {
                continue;
            }
            attack.enabled = true;
            // Go addWeaponAttack: an empty slot never swings.
            if attack.weapon.swing_speed <= 0.0 {
                attack.enabled = false;
                continue;
            }
            attack.update_swing_duration(haste);
            let swing_at = attack.swing_at;
            self.autos.attacks.push(hand);
            self.autos.min_time = self.autos.min_time.min(swing_at);
        }
    }

    /// Go `Simulation.Step`'s weapon check: whether swings are due before the next action at
    /// `next`.
    pub(crate) fn due_weapon_attack(&self, next: i64) -> bool {
        next >= self.autos.min_time
    }

    /// Go `advanceWeaponAttacks`.
    pub(crate) fn advance_weapon_attacks(&mut self) {
        if self.autos.min_time > self.now {
            let time = self.autos.min_time;
            self.advance_to(time);
        }
        self.autos.min_time = NEVER_EXPIRES;
        for position in 0..self.autos.attacks.len() {
            let hand = self.autos.attacks[position];
            let next = self.try_swing(hand);
            self.autos.min_time = self.autos.min_time.min(next);
        }
    }

    /// Go `WeaponAttack.trySwing` and `swing`.
    fn try_swing(&mut self, hand: Hand) -> i64 {
        let now = self.now;
        let attack = self.autos.attack(hand);
        if now < attack.swing_at {
            return attack.swing_at;
        }
        attack.previous_swing = attack.swing_at;
        attack.swing_at = now + attack.cur_swing_duration;
        attack.natural_ready_at = attack.swing_at;
        let swing_at = attack.swing_at;
        if hand == Hand::Enemy {
            // The target's own reaction runs no rotation in scope.
            self.enemy_swing();
            return swing_at;
        }
        let spell = attack.spell.expect("an enabled weapon attack has a spell");
        self.cast(spell, Side::Target);
        // Go ReactToEvent(false, true) after the swing, unless the player is tanking.
        if self.enemy.is_none() {
            self.react_to_event();
        }
        swing_at
    }

    /// Go `Unit.ReactToEvent(sim, false, true)`.
    pub(crate) fn react_to_event(&mut self) {
        self.do_next_action();
        let evaluation = self.now + self.config.reaction;
        if self.player.rotation_timer > evaluation {
            self.set_rotation_timer(evaluation);
        }
    }

    /// The auto attack spell's `ApplyEffects`: weapon damage on the white hit table.
    pub(crate) fn apply_melee_auto(&mut self, spell: SpellId, target: Side, hand: Hand) {
        let attack_power = self.melee_attack_power();
        let weapon = self.autos.attack(hand).weapon.clone();
        let mut base = self.weapon_damage(&weapon, attack_power);
        if hand == Hand::Off {
            base *= 0.5;
        }
        let result = self.calc_physical_damage(spell, target, base, PhysicalOutcome::MeleeWhite);
        self.deal_damage(spell, result, false);
    }

    /// Go `Unit.MHWeaponDamage`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn mh_weapon_damage(&mut self, attack_power: f64) -> f64 {
        let weapon = self.autos.mh.weapon.clone();
        self.weapon_damage(&weapon, attack_power)
    }

    /// Go `Unit.OHWeaponDamage`: half the off hand's roll.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn oh_weapon_damage(&mut self, attack_power: f64) -> f64 {
        let weapon = self.autos.oh.weapon.clone();
        0.5 * self.weapon_damage(&weapon, attack_power)
    }

    /// Go `Unit.MHNormalizedWeaponDamage`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn mh_normalized_weapon_damage(&mut self, attack_power: f64) -> f64 {
        let weapon = self.autos.mh.weapon.clone();
        self.normalized_weapon_damage(&weapon, attack_power)
    }

    /// Go `Unit.OHNormalizedWeaponDamage`: half the off hand's normalized roll.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn oh_normalized_weapon_damage(&mut self, attack_power: f64) -> f64 {
        let weapon = self.autos.oh.weapon.clone();
        0.5 * self.normalized_weapon_damage(&weapon, attack_power)
    }

    /// Go `Weapon.CalculateNormalizedWeaponDamage`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn normalized_weapon_damage(&mut self, weapon: &Weapon, attack_power: f64) -> f64 {
        let roll = self.random("Weapon Base Damage");
        weapon.base_damage_min
            + (weapon.base_damage_max - weapon.base_damage_min) * roll
            + (weapon.normalized_swing_speed * attack_power) / weapon.attack_power_per_dps
    }

    /// Go `CalcOutcome` with a physical outcome applier: no damage, modifiers or debug line.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn calc_physical_outcome(
        &mut self,
        spell: SpellId,
        target: Side,
        outcome: PhysicalOutcome,
    ) -> SpellResult {
        let mut result = SpellResult {
            target,
            outcome: 0,
            damage: 0.0,
            threat: 0.0,
        };
        self.apply_physical_outcome(spell, &mut result, outcome);
        result.threat = if result.landed() {
            let state = &self.spells[spell];
            (result.damage * state.threat_multiplier + state.flat_threat_bonus)
                * self.config.threat_multiplier
        } else {
            0.0
        };
        result
    }

    /// Go `Weapon.CalculateWeaponDamage`.
    pub(crate) fn weapon_damage(&mut self, weapon: &Weapon, attack_power: f64) -> f64 {
        let roll = self.random("Weapon Base Damage");
        weapon.base_damage_min
            + (weapon.base_damage_max - weapon.base_damage_min) * roll
            + (weapon.swing_speed * attack_power) / weapon.attack_power_per_dps
    }

    /// Go `getAttackPowerValueImpl` with no mob type bonus.
    pub(crate) fn melee_attack_power(&self) -> f64 {
        self.player.powers.attack_power + self.config.melee.defender_bonus_attack_power
    }

    /// Go `GetArmorDamageModifier`.
    fn armor_modifier(&self) -> f64 {
        let melee = &self.config.melee;
        if melee.ignore_armor {
            return 1.0;
        }
        let ignore = melee.armor_ignore_factor.clamp(0.0, 1.0);
        let constant = 400.0 + 85.0 * f64::from(self.config.player_level);
        let armor = self.target_armor - self.target_armor * ignore;
        let armor = (armor - self.config.armor_penetration).max(0.0);
        (1.0 - armor / (armor + constant)).max(0.25)
    }

    /// Go `CalcDamage` for a physical spell with a physical outcome applier.
    pub(crate) fn calc_physical_damage(
        &mut self,
        spell: SpellId,
        target: Side,
        base_damage: f64,
        outcome: PhysicalOutcome,
    ) -> SpellResult {
        let attacker = self.attacker_multiplier(spell, false);
        let mut base = base_damage;
        let coefficient = self.spells[spell].bonus_coefficient;
        if coefficient > 0.0 {
            base += coefficient * self.physical_bonus_damage(spell);
        } else {
            base += self.physical_bonus_damage(spell);
        }
        let mut result = SpellResult {
            target,
            outcome: 0,
            damage: base * attacker,
            threat: 0.0,
        };
        let after_attacker = result.damage;
        if !self.spells[spell].flags.ignore_resists {
            result.damage *= self.armor_modifier();
        }
        let after_resistances = result.damage;
        if !self.spells[spell].flags.ignore_target_modifiers {
            result.damage += self.config.melee.defender_bonus_physical_damage_taken;
            result.damage *= self.target_multiplier(spell);
        }
        let after_target = result.damage;
        self.apply_physical_outcome(spell, &mut result, outcome);
        let after_outcome = result.damage;
        result.damage = result.damage.max(0.0);
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
                * self.config.threat_multiplier
        } else {
            0.0
        };
        result
    }

    /// Go `Spell.BonusDamage` for a physical spell.
    fn physical_bonus_damage(&self, spell: SpellId) -> f64 {
        self.spells[spell].bonus_base_damage + self.config.physical_damage
    }

    /// Go `PhysicalHitChance` for melee.
    fn physical_hit_chance(&self, spell: SpellId) -> f64 {
        let hit = self.config.physical_hit_percent + self.spells[spell].bonus_hit_percent
            - self.config.melee.defender_reduced_physical_hit_taken;
        (hit / 100.0 - self.config.melee.hit_suppression).max(0.0)
    }

    /// Go `PhysicalCritChance` for melee.
    fn physical_crit_chance(&self, spell: SpellId) -> f64 {
        let crit = self.player.powers.physical_crit_percent + self.spells[spell].bonus_crit_percent
            - self.config.target_reduced_crit_taken_percent;
        (crit / 100.0 - self.config.melee.melee_crit_suppression).max(0.0)
    }

    /// Go `DodgeParrySuppression`.
    fn dodge_parry_suppression(&self, spell: SpellId) -> f64 {
        (self.config.expertise_percent + self.spells[spell].bonus_expertise_percent) / 100.0
    }

    /// Go `applyAttackTableMiss` and `applyAttackTableMissNoDWPenalty`.
    fn table_miss(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
        dw_penalty: bool,
    ) -> bool {
        let mut miss = self.config.melee.base_miss_chance - self.physical_hit_chance(spell);
        if dw_penalty && self.autos.dual_wielding && !self.config.melee.disable_dw_miss_penalty {
            miss += 0.19;
        }
        *chance = miss.max(0.0);
        if roll < *chance {
            result.outcome = OUTCOME_MISS;
            self.spells[spell].metrics[result.target.index()].misses += 1;
            result.damage = 0.0;
            return true;
        }
        false
    }

    /// Go `applyAttackTableDodge`.
    fn table_dodge(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
    ) -> bool {
        if self.spells[spell].flags.cannot_be_dodged {
            return false;
        }
        let melee = &self.config.melee;
        *chance +=
            (melee.defender_dodge - self.dodge_parry_suppression(spell) - melee.dodge_reduction)
                .max(0.0);
        if roll < *chance {
            result.outcome = OUTCOME_DODGE;
            self.spells[spell].metrics[result.target.index()].dodges += 1;
            result.damage = 0.0;
            return true;
        }
        false
    }

    /// Go `applyAttackTableParry`.
    fn table_parry(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
    ) -> bool {
        *chance +=
            (self.config.melee.defender_parry - self.dodge_parry_suppression(spell)).max(0.0);
        if roll < *chance {
            result.outcome = OUTCOME_PARRY;
            self.spells[spell].metrics[result.target.index()].parries += 1;
            result.damage = 0.0;
            return true;
        }
        false
    }

    /// Go `applyAttackTableGlance`.
    fn table_glance(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
    ) -> bool {
        *chance += self.config.melee.base_glance_chance;
        if roll < *chance {
            result.outcome = OUTCOME_GLANCE;
            self.spells[spell].metrics[result.target.index()].glances += 1;
            let spread = 2.0 * self.random("Glance Damage") - 1.0;
            let melee = &self.config.melee;
            result.damage *= melee.glance_multiplier + melee.glance_spread * spread;
            return true;
        }
        false
    }

    /// Go `applyAttackTableBlock`: a blocked crit moves from crits to blocked crits.
    fn table_block(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
    ) -> bool {
        *chance += self.config.melee.defender_block.max(0.0);
        if roll < *chance {
            let partial = result.outcome & OUTCOME_PARTIAL != 0;
            result.outcome |= OUTCOME_BLOCK;
            let metrics = &mut self.spells[spell].metrics[result.target.index()];
            if result.outcome & OUTCOME_CRIT != 0 {
                metrics.crits -= 1;
                metrics.blocked_crits += 1;
                if partial {
                    metrics.resisted_crits -= 1;
                }
            } else {
                metrics.blocks += 1;
            }
            result.damage = if self.spells[spell].flags.binary {
                0.0
            } else {
                (result.damage - self.config.melee.defender_block_reduction).max(0.0)
            };
            return true;
        }
        false
    }

    /// Go `applyAttackTableCrit` on the shared roll.
    fn table_crit(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        roll: f64,
        chance: &mut f64,
        count: bool,
    ) -> bool {
        *chance += self.physical_crit_chance(spell);
        if roll < *chance {
            self.land_crit(spell, result, count);
            return true;
        }
        false
    }

    /// Go `applyAttackTableCritSeparateRoll`.
    fn table_crit_separate(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        count: bool,
    ) -> bool {
        if self.random("Physical Crit Roll") < self.physical_crit_chance(spell) {
            self.land_crit(spell, result, count);
            return true;
        }
        false
    }

    fn land_crit(&mut self, spell: SpellId, result: &mut SpellResult, count: bool) {
        let partial = result.outcome & OUTCOME_PARTIAL != 0;
        result.outcome = OUTCOME_CRIT;
        if count {
            let metrics = &mut self.spells[spell].metrics[result.target.index()];
            metrics.crits += 1;
            if partial {
                metrics.resisted_crits += 1;
            }
        }
        result.damage *= self.crit_multiplier(spell);
    }

    /// Go `applyAttackTableHit`.
    fn table_hit(&mut self, spell: SpellId, result: &mut SpellResult, count: bool) {
        let partial = result.outcome & OUTCOME_PARTIAL != 0;
        result.outcome = OUTCOME_HIT;
        if count {
            let metrics = &mut self.spells[spell].metrics[result.target.index()];
            metrics.hits += 1;
            if partial {
                metrics.resisted_hits += 1;
            }
        }
    }

    /// Apply a physical outcome, composed as Go composes each applier.
    pub(crate) fn apply_physical_outcome(
        &mut self,
        spell: SpellId,
        result: &mut SpellResult,
        outcome: PhysicalOutcome,
    ) {
        let front = self.config.melee.in_front_of_target;
        let mut chance = 0.0;
        match outcome {
            PhysicalOutcome::MeleeWhite => {
                let roll = self.random("White Hit Table");
                let _ = self.table_miss(spell, result, roll, &mut chance, true)
                    || self.table_dodge(spell, result, roll, &mut chance)
                    || (front && self.table_parry(spell, result, roll, &mut chance))
                    || self.table_glance(spell, result, roll, &mut chance)
                    || (front && self.table_block(spell, result, roll, &mut chance))
                    || self.table_crit(spell, result, roll, &mut chance, true)
                    || {
                        self.table_hit(spell, result, true);
                        true
                    };
            }
            PhysicalOutcome::MeleeSpecialHit { count } => {
                let roll = self.random("White Hit Table");
                let _ = self.table_miss(spell, result, roll, &mut chance, false)
                    || self.table_dodge(spell, result, roll, &mut chance)
                    || (front && self.table_parry(spell, result, roll, &mut chance))
                    || {
                        self.table_hit(spell, result, count);
                        true
                    };
            }
            PhysicalOutcome::MeleeSpecialHitAndCrit { count } => {
                let roll = self.random("White Hit Table");
                if self.table_miss(spell, result, roll, &mut chance, false)
                    || self.table_dodge(spell, result, roll, &mut chance)
                {
                    return;
                }
                if front {
                    if self.table_parry(spell, result, roll, &mut chance) {
                        return;
                    }
                    if self.table_crit_separate(spell, result, count) {
                        self.table_block(spell, result, roll, &mut chance);
                    } else if !self.table_block(spell, result, roll, &mut chance) {
                        self.table_hit(spell, result, count);
                    }
                } else if !self.table_crit_separate(spell, result, count) {
                    self.table_hit(spell, result, count);
                }
            }
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count } => {
                if !front {
                    return self.apply_physical_outcome(
                        spell,
                        result,
                        PhysicalOutcome::MeleeSpecialHitAndCrit { count },
                    );
                }
                let roll = self.random("White Hit Table");
                let _ = self.table_miss(spell, result, roll, &mut chance, false)
                    || self.table_dodge(spell, result, roll, &mut chance)
                    || self.table_parry(spell, result, roll, &mut chance)
                    || self.table_block(spell, result, roll, &mut chance)
                    || self.table_crit_separate(spell, result, count)
                    || {
                        self.table_hit(spell, result, count);
                        true
                    };
            }
            PhysicalOutcome::MeleeSpecialBlockAndCrit { count } => {
                if front {
                    let roll = self.random("White Hit Table");
                    let _ = self.table_crit_separate(spell, result, count)
                        || self.table_block(spell, result, roll, &mut chance)
                        || {
                            self.table_hit(spell, result, count);
                            true
                        };
                } else if !self.table_crit_separate(spell, result, count) {
                    self.table_hit(spell, result, count);
                }
            }
            PhysicalOutcome::MeleeSpecialNoBlockDodgeParry { count } => {
                let roll = self.random("White Hit Table");
                let _ = self.table_miss(spell, result, roll, &mut chance, false)
                    || self.table_crit_separate(spell, result, count)
                    || {
                        self.table_hit(spell, result, count);
                        true
                    };
            }
            PhysicalOutcome::MeleeSpecialCritOnly { count } => {
                if !self.table_crit_separate(spell, result, count) {
                    self.table_hit(spell, result, count);
                }
            }
        }
    }
}

/// Go physical outcome appliers the runtime implements; `count` is false for the
/// `NoHitCounter` variants. The names follow Go's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names, dead_code)] // Shared with the class domains in progress.
pub(crate) enum PhysicalOutcome {
    MeleeWhite,
    MeleeSpecialHit { count: bool },
    MeleeSpecialHitAndCrit { count: bool },
    MeleeWeaponSpecialHitAndCrit { count: bool },
    MeleeSpecialBlockAndCrit { count: bool },
    MeleeSpecialNoBlockDodgeParry { count: bool },
    MeleeSpecialCritOnly { count: bool },
}
