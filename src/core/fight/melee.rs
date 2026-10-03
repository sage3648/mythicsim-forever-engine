//! Go attack.go and the physical half of spell_outcome.go and spell_resistances.go: each
//! acting unit's main and off hand swings, weapon damage, armor and the physical attack table.
//! The player's and its pet's swings share Go's one simulation weapon attack list.
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
    /// The ranged slot's auto attack.
    Ranged,
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
    /// Go `extraAttacks`: extra attacks still owed after the one pulled to now.
    pub(crate) extra_attacks: i32,
    /// Go `pendingSwingDelay`: how late the last swing fired against `natural_ready_at`.
    pub(crate) pending_swing_delay: i64,
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
    /// Go `AutoSwingRanged`.
    pub(crate) ranged_auto: bool,
    pub(crate) dual_wielding: bool,
    pub(crate) mh: WeaponAttack,
    pub(crate) oh: WeaponAttack,
    pub(crate) ranged: WeaponAttack,
    pub(crate) enemy: WeaponAttack,
    /// Go `sim.weaponAttacks`, in the order swings were added, by unit. The player's copy
    /// holds the simulation's list; a pet's stays empty.
    attacks: WeaponAttackList,
    /// Go `sim.minWeaponAttackTime`, likewise kept in the player's copy.
    pub(crate) min_time: i64,
}

impl AutoAttacks {
    pub(crate) fn attack(&mut self, hand: Hand) -> &mut WeaponAttack {
        match hand {
            Hand::Main => &mut self.mh,
            Hand::Off => &mut self.oh,
            Hand::Ranged => &mut self.ranged,
            Hand::Enemy => &mut self.enemy,
        }
    }

    /// Go `AutoAttacks.anyEnabled`.
    fn any_enabled(&self) -> bool {
        self.mh.enabled || self.oh.enabled || self.ranged.enabled
    }
}

/// Go `sim.weaponAttacks`: a slice whose removal swaps the last entry into the gap. Go's range
/// loop over it captures the length and reads the backing array, so a removal during a swing
/// leaves the old last entry visible to the loop; the list keeps the backing array to match.
#[derive(Clone, Debug, Default)]
pub(crate) struct WeaponAttackList {
    backing: Vec<(Side, Hand)>,
    len: usize,
}

impl WeaponAttackList {
    fn clear(&mut self) {
        self.len = 0;
    }

    /// Go `append`, which writes into the backing array while it has room.
    fn push(&mut self, entry: (Side, Hand)) {
        if self.len < self.backing.len() {
            self.backing[self.len] = entry;
        } else {
            self.backing.push(entry);
        }
        self.len += 1;
    }

    /// Go `removeWeaponAttack`.
    fn remove(&mut self, entry: (Side, Hand)) {
        if let Some(index) = self.backing[..self.len].iter().position(|&e| e == entry) {
            self.backing[index] = self.backing[self.len - 1];
            self.len -= 1;
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl std::ops::Index<usize> for WeaponAttackList {
    type Output = (Side, Hand);

    fn index(&self, index: usize) -> &(Side, Hand) {
        &self.backing[index]
    }
}

/// Go `WeaponAttack.IsInRange` for a weapon at the given distance.
fn in_range(weapon: &Weapon, distance: f64) -> bool {
    (weapon.min_range == 0.0 || weapon.min_range < distance)
        && (weapon.max_range == 0.0 || weapon.max_range >= distance)
}

impl<A: Agent> Fight<A> {
    /// The weapon attacks of an acting unit.
    pub(crate) fn autos_of(&self, side: Side) -> &AutoAttacks {
        match side {
            Side::Pet => &self.pet.as_ref().expect("the pet is simulated").autos,
            _ => &self.autos,
        }
    }

    /// [`Self::autos_of`], mutable.
    pub(crate) fn autos_of_mut(&mut self, side: Side) -> &mut AutoAttacks {
        match side {
            Side::Pet => &mut self.pet.as_mut().expect("the pet is simulated").autos,
            _ => &mut self.autos,
        }
    }

    /// Go `TotalMeleeHasteMultiplier`.
    pub(crate) fn melee_haste_multiplier(&self) -> f64 {
        self.melee_haste_multiplier_of(Side::Player)
    }

    /// Go `TotalMeleeHasteMultiplier` of an acting unit.
    pub(crate) fn melee_haste_multiplier_of(&self, side: Side) -> f64 {
        let unit = self.unit(side);
        unit.attack_speed_multiplier
            * unit.melee_speed_multiplier
            * (1.0
                + self.unit_config(side).melee_haste_rating
                    / (PHYSICAL_HASTE_RATING_PER_PERCENT * 100.0))
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

    /// Go `TotalRangedHasteMultiplier`.
    pub(crate) fn ranged_haste_multiplier(&self) -> f64 {
        self.player.attack_speed_multiplier
            * self.player.ranged_speed_multiplier
            * (1.0 + self.config.melee_haste_rating / (PHYSICAL_HASTE_RATING_PER_PERCENT * 100.0))
    }

    /// Go `Unit.MultiplyRangedSpeed`.
    pub(crate) fn multiply_ranged_speed(&mut self, amount: f64) {
        self.player.ranged_speed_multiplier *= amount;
        self.update_swing_timers();
    }

    /// Go `AutoAttacks.UpdateSwingTimers`: the remaining part of each pending melee swing
    /// scales with the change in speed, the off hand by the main hand's factor. A ranged
    /// speed change waits for the next shot.
    pub(crate) fn update_swing_timers(&mut self) {
        if !self.autos.any_enabled() {
            return;
        }
        if self.autos.ranged_auto && self.autos.ranged.enabled {
            let haste = self.ranged_haste_multiplier();
            self.autos.ranged.update_swing_duration(haste);
        }
        if !self.autos.melee || !self.autos.mh.enabled {
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
        self.reset_auto_attacks_of(Side::Player);
    }

    /// Go `AutoAttacks.reset` for one acting unit.
    pub(crate) fn reset_auto_attacks_of(&mut self, side: Side) {
        if !self.autos_of(side).melee && !self.autos_of(side).ranged_auto {
            return;
        }
        let haste = self.melee_haste_multiplier_of(side);
        let ranged_haste = self.ranged_haste_multiplier();
        let autos = self.autos_of_mut(side);
        for attack in [&mut autos.mh, &mut autos.oh, &mut autos.ranged] {
            attack.enabled = false;
        }
        for attack in [&mut autos.mh, &mut autos.oh] {
            attack.previous_swing = -NEVER_EXPIRES;
            attack.swing_at = NEVER_EXPIRES;
        }
        autos.mh.extra_attacks = 0;
        if autos.melee {
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
        autos.ranged.previous_swing = -NEVER_EXPIRES;
        autos.ranged.swing_at = NEVER_EXPIRES;
        if autos.ranged_auto {
            autos.ranged.update_swing_duration(ranged_haste);
            autos.ranged.previous_swing = -autos.ranged.cur_swing_duration;
            autos.ranged.swing_at = 0;
            autos.ranged.natural_ready_at = 0;
        }
    }

    /// Go `AutoAttacks.StopRangedUntil`: the ranged timer restarts as if a shot fired at
    /// `ready`.
    pub(crate) fn stop_ranged_until(&mut self, ready: i64) {
        if !self.autos.ranged_auto {
            return;
        }
        self.autos.ranged.swing_at = ready + self.autos.ranged.cur_swing_duration;
        self.autos.min_time = self.autos.min_time.min(self.autos.ranged.swing_at);
    }

    /// Go `AutoAttacks.DelayRangedUntil`.
    #[allow(dead_code)] // Shared with the class domains in progress.
    pub(crate) fn delay_ranged_until(&mut self, ready: i64) {
        if ready <= self.autos.ranged.swing_at {
            return;
        }
        self.autos.ranged.swing_at = ready;
        self.autos.min_time = self.autos.min_time.min(ready);
    }

    /// When the rotation's `autoTimeToNext` reads the next auto attack of a kind: Go
    /// `NextAttackAt`, `MainhandSwingAt`, `OffhandSwingAt`, `NextRangedAttackAt` and
    /// `NextAnyAttackAt`.
    pub(crate) fn next_auto_attack_at(&self, kind: crate::rotation::AutoAttackType) -> i64 {
        use crate::rotation::AutoAttackType;
        let autos = &self.autos;
        match kind {
            AutoAttackType::Melee => autos.mh.swing_at.min(autos.oh.swing_at),
            AutoAttackType::MainHand => autos.mh.swing_at,
            AutoAttackType::OffHand => autos.oh.swing_at,
            AutoAttackType::Ranged => autos.ranged.swing_at,
            AutoAttackType::Any => autos
                .mh
                .swing_at
                .min(autos.oh.swing_at)
                .min(autos.ranged.swing_at),
        }
    }

    /// Go `RandomizeMeleeTiming` at encounter start: delay the first swings by a random whole
    /// number of milliseconds below the reaction time, unless out of melee range.
    pub(crate) fn randomize_melee_timing(&mut self, side: Side) {
        let config = self.unit_config(side);
        if !self.autos_of(side).melee || config.distance > MAX_MELEE_RANGE {
            return;
        }
        let reaction_ms = config.reaction / crate::core::time::NS_PER_MILLISECOND;
        let roll = self.random("Melee Timing");
        let delay = (roll * reaction_ms as f64) as i64 * crate::core::time::NS_PER_MILLISECOND;
        // Go DelayMeleeBy.
        if delay <= 0 {
            return;
        }
        let autos = self.autos_of_mut(side);
        autos.mh.swing_at += delay;
        let mut earliest = autos.mh.swing_at;
        if autos.dual_wielding {
            autos.oh.swing_at += delay;
            earliest = earliest.min(autos.oh.swing_at);
        }
        self.autos.min_time = self.autos.min_time.min(earliest);
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
        self.autos.attacks.push((Side::Target, Hand::Enemy));
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
    pub(crate) fn start_auto_attacks(&mut self, side: Side) {
        let autos = self.autos_of(side);
        if (!autos.melee && !autos.ranged_auto) || autos.any_enabled() {
            return;
        }
        if self.autos_of(side).melee {
            self.start_melee_attacks(side);
        }
        if self.autos_of(side).ranged_auto {
            let distance = self.config.distance;
            let haste = self.ranged_haste_multiplier();
            let ranged = &mut self.autos.ranged;
            if ranged.swing_at == NEVER_EXPIRES {
                ranged.swing_at = 0;
                ranged.natural_ready_at = 0;
            }
            if in_range(&ranged.weapon, distance) {
                // Go addWeaponAttack: an empty slot never swings.
                if ranged.weapon.swing_speed <= 0.0 {
                    return;
                }
                ranged.enabled = true;
                ranged.update_swing_duration(haste);
                let swing_at = ranged.swing_at;
                self.autos.attacks.push((side, Hand::Ranged));
                self.autos.min_time = self.autos.min_time.min(swing_at);
            }
        }
    }

    /// The melee half of Go `AutoAttacks.startPull`.
    fn start_melee_attacks(&mut self, side: Side) {
        let haste = self.melee_haste_multiplier_of(side);
        let distance = self.unit_config(side).distance;
        let in_range = |weapon: &Weapon| in_range(weapon, distance);
        let autos = self.autos_of_mut(side);
        if autos.mh.swing_at == NEVER_EXPIRES {
            autos.mh.swing_at = 0;
            autos.mh.natural_ready_at = 0;
        }
        let hands: &[Hand] = if autos.dual_wielding {
            &[Hand::Off, Hand::Main]
        } else {
            &[Hand::Main]
        };
        for &hand in hands {
            let attack = self.autos_of_mut(side).attack(hand);
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
            self.autos.attacks.push((side, hand));
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
            let (side, hand) = self.autos.attacks[position];
            let next = self.try_swing(side, hand);
            self.autos.min_time = self.autos.min_time.min(next);
        }
    }

    /// Go `WeaponAttack.trySwing` and `swing`. The next swing time is read after the swing,
    /// which a melee speed change during it can move.
    fn try_swing(&mut self, side: Side, hand: Hand) -> i64 {
        let now = self.now;
        if now < self.autos_of_mut(side).attack(hand).swing_at {
            return self.autos_of_mut(side).attack(hand).swing_at;
        }
        if hand == Hand::Enemy {
            let attack = self.autos.attack(hand);
            attack.previous_swing = attack.swing_at;
            attack.swing_at = now + attack.cur_swing_duration;
            attack.natural_ready_at = attack.swing_at;
            // The target's own reaction runs no rotation in scope.
            self.enemy_swing();
            return self.autos.attack(hand).swing_at;
        }
        let mut spell = self
            .autos_of_mut(side)
            .attack(hand)
            .spell
            .expect("an enabled weapon attack has a spell");
        // Go: with a replacer set, the rotation runs first, then the class may replace the
        // main hand swing, as Heroic Strike does.
        if side == Side::Player && hand == Hand::Main && self.config.melee.replace_main_hand_swing {
            self.react_to_event_now();
            spell = A::replace_mh_swing(self, spell);
        }
        let attack = self.autos_of_mut(side).attack(hand);
        attack.previous_swing = attack.swing_at;
        attack.swing_at = now + attack.cur_swing_duration;
        if attack.extra_attacks > 0 {
            attack.extra_attacks -= 1;
            attack.swing_at = now;
        }
        attack.pending_swing_delay = (now - attack.natural_ready_at).max(0);
        attack.natural_ready_at = attack.swing_at;
        // A melee swing resets the ranged auto timer, as if the shot had just fired.
        if side == Side::Player && hand != Hand::Ranged && self.autos.ranged_auto {
            self.stop_ranged_until(now);
        }
        self.cast(spell, Side::Target);
        // Go ReactToEvent(false, true) after the swing, unless the player is tanking.
        if side != Side::Player || self.enemy.is_none() {
            self.react_to_event(side);
        }
        self.autos_of_mut(side).attack(hand).swing_at
    }

    /// Go `AutoAttacks.CancelMeleeSwing`: the swings leave the simulation's list. The next
    /// weapon attack time stays, as Go only lowers it.
    pub(crate) fn cancel_melee_swing(&mut self) {
        if !self.autos.melee {
            return;
        }
        if self.autos.mh.enabled {
            self.autos.attacks.remove((Side::Player, Hand::Main));
            self.autos.mh.enabled = false;
        }
        if self.autos.dual_wielding && self.autos.oh.enabled {
            self.autos.oh.enabled = false;
            self.autos.attacks.remove((Side::Player, Hand::Off));
        }
    }

    /// Go `AutoAttacks.EnableMeleeSwing`: each hand resumes no earlier than now, and a
    /// stopped hand rejoins the simulation's list.
    pub(crate) fn enable_melee_swing(&mut self) {
        if !self.autos.melee || self.now < 0 {
            return;
        }
        let now = self.now;
        let distance = self.config.distance;
        self.autos.mh.swing_at = self.autos.mh.swing_at.max(now).max(0);
        if in_range(&self.autos.mh.weapon, distance) && !self.autos.mh.enabled {
            self.autos.mh.enabled = true;
            self.add_weapon_attack(Hand::Main);
        }
        if self.autos.dual_wielding && !self.autos.oh.enabled {
            // No ranged swing in scope, so the off hand resumes without an offset.
            self.autos.oh.swing_at = self.autos.oh.swing_at.max(now).max(0);
            if in_range(&self.autos.oh.weapon, distance) {
                self.autos.oh.enabled = true;
                self.add_weapon_attack(Hand::Off);
            }
        }
    }

    /// Go `WeaponAttack.addWeaponAttack` for a melee hand.
    fn add_weapon_attack(&mut self, hand: Hand) {
        let haste = self.melee_haste_multiplier();
        let attack = self.autos.attack(hand);
        if !attack.enabled {
            return;
        }
        // An empty slot never swings.
        if attack.weapon.swing_speed <= 0.0 {
            attack.enabled = false;
            return;
        }
        attack.update_swing_duration(haste);
        let swing_at = attack.swing_at;
        self.autos.attacks.push((Side::Player, hand));
        self.autos.min_time = self.autos.min_time.min(swing_at);
    }

    /// Go `AutoAttacks.ExtraMHAttacks`: the main hand swings now, then again for each extra
    /// attack still owed.
    pub(crate) fn extra_mh_attacks(&mut self, count: i32) {
        if count <= 0 || !self.autos.melee || !self.autos.mh.enabled {
            return;
        }
        self.autos.mh.extra_attacks += count - 1;
        // Go ExtraMHAttack.
        self.autos.mh.swing_at = self.now;
        self.autos.min_time = self.autos.min_time.min(self.now);
    }

    /// Go `Unit.ReactToEvent(sim, false, false)`: the rotation runs, then evaluates again now.
    pub(crate) fn react_to_event_now(&mut self) {
        self.do_next_action();
        if self.player.rotation_timer > self.now {
            self.set_rotation_timer(self.now);
        }
    }

    /// Go `AutoAttacks.HoldMeleeForCast`: a swing due before the cast ends waits for it; a
    /// later one restarts its timer from the cast's end.
    pub(crate) fn hold_melee_for_cast(&mut self, cast_end: i64) {
        // Go heldSwingLag.
        const HELD_SWING_LAG: i64 = 1;
        if !self.autos.melee {
            return;
        }
        let now = self.now;
        let hands: &[Hand] = if self.autos.dual_wielding {
            &[Hand::Main, Hand::Off]
        } else {
            &[Hand::Main]
        };
        for &hand in hands {
            let attack = self.autos.attack(hand);
            if attack.swing_at <= now + HELD_SWING_LAG {
                continue;
            }
            attack.swing_at = if attack.swing_at <= cast_end {
                cast_end + HELD_SWING_LAG
            } else {
                cast_end + attack.cur_swing_duration
            };
            // Go rescheduleWeaponAttack.
            let swing_at = attack.swing_at;
            self.autos.min_time = self.autos.min_time.min(swing_at);
        }
    }

    /// Go `Unit.ReactToEvent(sim, false, true)`.
    pub(crate) fn react_to_event(&mut self, side: Side) {
        self.do_next_action_of(side);
        let evaluation = self.now + self.unit_config(side).reaction;
        if self.unit(side).rotation_timer > evaluation {
            self.set_rotation_timer_of(side, evaluation);
        }
    }

    /// The auto attack spell's `ApplyEffects`: weapon damage on the white hit table.
    pub(crate) fn apply_melee_auto(&mut self, spell: SpellId, target: Side, hand: Hand) {
        if hand == Hand::Ranged {
            return self.apply_ranged_auto(spell, target);
        }
        A::before_melee_auto(self, spell, hand);
        let caster = self.caster(spell);
        let attack_power = self.melee_attack_power_of(caster);
        let weapon = self.autos_of_mut(caster).attack(hand).weapon.clone();
        let mut base = self.weapon_damage(&weapon, attack_power);
        if hand == Hand::Off {
            base *= 0.5;
        }
        let result = self.calc_physical_damage(spell, target, base, PhysicalOutcome::MeleeWhite);
        self.deal_damage(spell, result, false);
    }

    /// The ranged auto attack spell's `ApplyEffects`: the late-shot line, weapon damage on the
    /// ranged table dealt after travel, and a reaction for movement actions.
    fn apply_ranged_auto(&mut self, spell: SpellId, target: Side) {
        let delay = self.autos.ranged.pending_swing_delay;
        let ready_at = self.now - delay;
        if self.log.is_some() && delay > crate::core::time::NS_PER_MILLISECOND && ready_at > 0 {
            let line = format!(
                "{} delayed by {}, was ready at {}",
                super::log::action_string(&self.spells[spell].id),
                crate::core::time::go_string(delay),
                crate::core::time::go_string(ready_at)
            );
            self.player_log(&line);
        }
        let attack_power = self.ranged_attack_power();
        let weapon = self.autos.ranged.weapon.clone();
        let base = self.weapon_damage(&weapon, attack_power);
        let result = self.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::RangedHitAndCrit { count: true },
        );
        self.deal_damage_after_travel(spell, result);
        self.react_to_event(Side::Player);
    }

    /// Go `Spell.RangedAttackPower` with no mob type bonus.
    pub(crate) fn ranged_attack_power(&self) -> f64 {
        self.player.powers.ranged_attack_power + self.config.defender_bonus_ranged_attack_power
    }

    /// Go `Weapon.CalculateNormalizedWeaponDamage` for the ranged weapon.
    pub(crate) fn ranged_normalized_weapon_damage(&mut self, attack_power: f64) -> f64 {
        let weapon = self.autos.ranged.weapon.clone();
        self.normalized_weapon_damage(&weapon, attack_power)
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
        result.threat = self.threat_of(spell, &result);
        result
    }

    /// Go `Weapon.CalculateWeaponDamage`.
    pub(crate) fn weapon_damage(&mut self, weapon: &Weapon, attack_power: f64) -> f64 {
        let roll = self.random("Weapon Base Damage");
        weapon.base_damage_min
            + (weapon.base_damage_max - weapon.base_damage_min) * roll
            + (weapon.swing_speed * attack_power) / weapon.attack_power_per_dps
    }

    /// Go `getAttackPowerValueImpl` for the player, with no mob type bonus.
    pub(crate) fn melee_attack_power(&self) -> f64 {
        self.melee_attack_power_of(Side::Player)
    }

    /// Go `getAttackPowerValueImpl` of an acting unit, with no mob type bonus.
    pub(crate) fn melee_attack_power_of(&self, side: Side) -> f64 {
        self.unit(side).powers.attack_power
            + self.unit_config(side).melee.defender_bonus_attack_power
    }

    /// Go `GetArmorDamageModifier` for the attacker.
    pub(crate) fn armor_modifier(&self, side: Side) -> f64 {
        let config = self.unit_config(side);
        let melee = &config.melee;
        if melee.ignore_armor {
            return 1.0;
        }
        let ignore = melee.armor_ignore_factor.clamp(0.0, 1.0);
        let constant = 400.0 + 85.0 * f64::from(config.player_level);
        let armor = self.target_armor - self.target_armor * ignore;
        let armor = (armor - config.armor_penetration).max(0.0);
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
        let caster = self.caster(spell);
        if !self.spells[spell].flags.ignore_resists {
            result.damage *= self.armor_modifier(caster);
        }
        let after_resistances = result.damage;
        if !self.spells[spell].flags.ignore_target_modifiers {
            result.damage += self
                .unit_config(caster)
                .melee
                .defender_bonus_physical_damage_taken;
            result.damage *= self.target_multiplier(spell);
        }
        let after_target = result.damage;
        self.apply_physical_outcome(spell, &mut result, outcome);
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
        result.threat = self.threat_of(spell, &result);
        result
    }

    /// Go `CalcPeriodicDamage` for a physical dot with `Dot.OutcomeTick`: the periodic attacker
    /// multiplier, no armor for a bleed, the target's physical modifiers and a tick counter.
    pub(crate) fn calc_physical_periodic_damage(
        &mut self,
        spell: SpellId,
        target: Side,
        base_damage: f64,
    ) -> SpellResult {
        let dot = self.spells[spell].dot.expect("a periodic spell has a dot");
        let coefficient = self.dots[dot].bonus_coefficient;
        let mut base = base_damage;
        if coefficient > 0.0 {
            base += coefficient * self.physical_bonus_damage(spell);
        }
        let attacker =
            self.attacker_multiplier(spell, true) * self.dots[dot].periodic_damage_multiplier;
        let mut result = SpellResult {
            target,
            outcome: 0,
            damage: base * attacker,
            threat: 0.0,
        };
        let after_attacker = result.damage;
        // Go ResistanceMultiplier: physical dots ignore armor.
        let after_resistances = result.damage;
        if !self.spells[spell].flags.ignore_target_modifiers {
            result.damage += self.config.melee.defender_bonus_physical_damage_taken;
            result.damage *= self.target_multiplier(spell);
        }
        let after_target = result.damage;
        // Go Dot.OutcomeTick.
        result.outcome = OUTCOME_HIT;
        self.spells[spell].metrics[target.index()].ticks += 1;
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
        let state = &self.spells[spell];
        result.threat = (result.damage * state.threat_multiplier + state.flat_threat_bonus)
            * self.config.threat_multiplier;
        result
    }

    /// Go `Spell.BonusDamage` for a physical spell.
    fn physical_bonus_damage(&self, spell: SpellId) -> f64 {
        self.spells[spell].bonus_base_damage + self.unit_config(self.caster(spell)).physical_damage
    }

    /// Go `PhysicalHitChance`: ranged attacks add ranged hit.
    fn physical_hit_chance(&self, spell: SpellId) -> f64 {
        let config = self.unit_config(self.caster(spell));
        let mut hit = config.physical_hit_percent + self.spells[spell].bonus_hit_percent
            - config.melee.defender_reduced_physical_hit_taken;
        if self.spells[spell].ranged_proc {
            hit += config.ranged_hit_percent;
        }
        (hit / 100.0 - config.melee.hit_suppression).max(0.0)
    }

    /// Go `PhysicalCritChance`: ranged attacks add ranged crit.
    pub(crate) fn physical_crit_chance(&self, spell: SpellId) -> f64 {
        let caster = self.caster(spell);
        let config = self.unit_config(caster);
        let mut crit = self.unit(caster).powers.physical_crit_percent
            + self.spells[spell].bonus_crit_percent
            - config.target_reduced_crit_taken_percent;
        if self.spells[spell].ranged_proc {
            crit += config.ranged_crit_percent;
        }
        (crit / 100.0 - config.melee.melee_crit_suppression).max(0.0)
    }

    /// Go `AutoAttacks.StopMeleeUntil`: the swings restart a full swing after `ready_at`.
    pub(crate) fn stop_melee_until(&mut self, ready_at: i64) {
        if !self.autos.melee {
            return;
        }
        self.autos.mh.swing_at = ready_at + self.autos.mh.cur_swing_duration;
        self.autos.min_time = self.autos.min_time.min(self.autos.mh.swing_at);
        if self.autos.dual_wielding {
            self.autos.oh.swing_at = ready_at + self.autos.oh.cur_swing_duration;
            self.autos.min_time = self.autos.min_time.min(self.autos.oh.swing_at);
        }
    }

    /// Go `DodgeParrySuppression`.
    fn dodge_parry_suppression(&self, spell: SpellId) -> f64 {
        (self.unit_config(self.caster(spell)).expertise_percent
            + self.spells[spell].bonus_expertise_percent)
            / 100.0
    }

    /// The physical attack table of a spell's caster against the target.
    fn melee_table(&self, spell: SpellId) -> &crate::contracts::prepared_v2::Melee {
        &self.unit_config(self.caster(spell)).melee
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
        let mut miss = self.melee_table(spell).base_miss_chance - self.physical_hit_chance(spell);
        if dw_penalty
            && self.autos_of(self.caster(spell)).dual_wielding
            && !self.melee_table(spell).disable_dw_miss_penalty
        {
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
        let melee = self.melee_table(spell);
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
            (self.melee_table(spell).defender_parry - self.dodge_parry_suppression(spell)).max(0.0);
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
        *chance += self.melee_table(spell).base_glance_chance;
        if roll < *chance {
            result.outcome = OUTCOME_GLANCE;
            self.spells[spell].metrics[result.target.index()].glances += 1;
            let spread = 2.0 * self.random("Glance Damage") - 1.0;
            let melee = self.melee_table(spell);
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
        *chance += self.melee_table(spell).defender_block.max(0.0);
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
                (result.damage - self.melee_table(spell).defender_block_reduction).max(0.0)
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
        let front = self.melee_table(spell).in_front_of_target;
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
            PhysicalOutcome::RangedHit { count } => {
                let roll = self.random("White Hit Table");
                if !self.table_miss(spell, result, roll, &mut chance, false) {
                    self.table_hit(spell, result, count);
                }
            }
            PhysicalOutcome::RangedHitAndCrit { count } => {
                let roll = self.random("White Hit Table");
                if self.table_miss(spell, result, roll, &mut chance, false) {
                    return;
                }
                if front {
                    if self.table_crit_separate(spell, result, count) {
                        self.table_block(spell, result, roll, &mut chance);
                    } else if !self.table_block(spell, result, roll, &mut chance) {
                        self.table_hit(spell, result, count);
                    }
                } else if !self.table_crit_separate(spell, result, count) {
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
    RangedHit { count: bool },
    RangedHitAndCrit { count: bool },
}
