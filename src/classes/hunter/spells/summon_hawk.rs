//! Summon Hawk (1293241 to 1293527), from Go sim/hunter/summon_hawk.go: a dive bomb on the
//! rank's base plus a share of ranged attack power, a Go literal, on the melee special table,
//! which cannot miss when the client marks the rank always-hit. A landed dive bomb leaves a
//! hawk, in a free slot or else the one with the least time left: a physical dot hasted by the
//! hunter's real ranged haste, which swings once on arrival and then on every tick for a share
//! of the rank's base, on the melee special hit table without a crit roll (community #703).
//!
//! Under Forever's rules (the effect's `guardian`) a hawk swing is a guardian's, from Forever's
//! combat logs (docs/mechanics.md): a roll on a fixed average plus a share of the
//! owner's ranged attack power, taken through the target's armor, on the melee special table with
//! a crit roll at Ferocity's crit plus half the owner's.

use crate::{
    contracts::prepared_v2::HawkGuardian,
    core::fight::{melee::PhysicalOutcome, Agent, DotId, Fight, Outcome, Side, SpellId},
};

/// The bound Summon Hawk.
#[derive(Clone, Debug)]
pub(crate) struct SummonHawk {
    base_damage: f64,
    attack_power_share: f64,
    always_hits: bool,
    /// Forever's hawk swing, if the rules give it.
    guardian: Option<HawkGuardian>,
    /// Each hawk slot's dot, in Go's slot order.
    hawks: Vec<DotId>,
}

impl SummonHawk {
    pub(crate) fn bind<A: Agent>(
        fight: &mut Fight<A>,
        base_damage: f64,
        attack_power_share: f64,
        swing_share: f64,
        always_hits: bool,
        guardian: Option<HawkGuardian>,
        hawk_spells: &[usize],
    ) -> Result<Self, String> {
        let mut hawks = Vec::new();
        for &spell in hawk_spells {
            let dot = fight
                .spells
                .get(spell)
                .and_then(|state| state.dot)
                .ok_or("a hawk spell has no dot")?;
            // Go OnSnapshot: the swing's share of the rank's base, without a spell power share.
            fight.dots[dot].tick_base = Some(base_damage * swing_share);
            hawks.push(dot);
        }
        Ok(SummonHawk {
            base_damage,
            attack_power_share,
            always_hits,
            guardian,
            hawks,
        })
    }

    /// The cast's `ApplyEffects`.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        // Go fuses this multiply and add on the reference arm64 build.
        let damage = self
            .attack_power_share
            .mul_add(fight.ranged_attack_power(), self.base_damage);
        let outcome = if self.always_hits {
            PhysicalOutcome::MeleeSpecialCritOnly { count: true }
        } else {
            PhysicalOutcome::MeleeSpecialHitAndCrit { count: true }
        };
        let result = fight.calc_physical_damage(spell, target, damage, outcome);
        fight.deal_damage(spell, result, false);
        if !result.landed() {
            return;
        }
        let now = fight.now;
        let active = |fight: &Fight<A>, dot: DotId| fight.aura(fight.dots[dot].aura).active;
        let remaining =
            |fight: &Fight<A>, dot: DotId| fight.aura(fight.dots[dot].aura).remaining(now);
        let mut hawk = self.hawks[0];
        for &dot in &self.hawks[1..] {
            if active(fight, hawk)
                && (!active(fight, dot) || remaining(fight, dot) < remaining(fight, hawk))
            {
                hawk = dot;
            }
        }
        fight.apply_dot(hawk);
        // Go TickOnce: the swing on arrival, beside the dot's own ticks.
        fight.tick_once(hawk);
    }

    /// A hawk's swing. Under Forever's rules a guardian's, [`Self::guardian_swing`]; otherwise
    /// Go `CalcAndDealPeriodicSnapshotDamage` with `OutcomeMeleeSpecialHit`, the melee special
    /// table of beta reports 2695 and 2701 without a crit roll: a hawk's attacks can miss, be
    /// dodged and be parried.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        match &self.guardian {
            Some(guardian) => Self::guardian_swing(fight, dot, guardian),
            None => Self::snapshot_swing(fight, dot),
        }
    }

    /// A guardian hawk's swing, read off Forever's combat logs: the roll's average plus
    /// `attack_power_coefficient` for each point of the owner's ranged attack power, before the
    /// target's armor and the owner's damage multipliers, and a crit roll at the spell's flat crit
    /// (Ferocity's) plus `crit_share` of the owner's crit and offset. The owner's crit is read
    /// when the hawk swings; its ranged attack power is the stat's, not Hunter's Mark's, which is
    /// the target's.
    fn guardian_swing<A: Agent>(fight: &mut Fight<A>, dot: DotId, guardian: &HawkGuardian) {
        let (spell, side) = (fight.dots[dot].spell, fight.dots[dot].side);
        let (low, high) = guardian.swing_bounds();
        let roll = fight.go_roll(low, high);
        let powers = fight.unit(Side::Player).powers;
        let base = guardian
            .attack_power_coefficient
            .mul_add(powers.ranged_attack_power, roll);
        // The spell's crit is the owner's crit plus its flat bonus, so a hawk's share is the
        // difference, taken off for the roll and put back after.
        let spell_crit = fight.spells[spell].bonus_crit_percent;
        fight.spells[spell].bonus_crit_percent =
            spell_crit + guardian.crit_adjustment(powers.physical_crit_percent);
        let result = fight.calc_physical_damage(
            spell,
            side,
            base,
            PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
        );
        fight.spells[spell].bonus_crit_percent = spell_crit;
        fight.deal_damage(spell, result, true);
    }

    fn snapshot_swing<A: Agent>(fight: &mut Fight<A>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side) = (state.spell, state.side);
        let mut base = state.snapshot_base;
        if state.reads_spell_power {
            base += state.bonus_coefficient * fight.bonus_damage(spell, side)
                - state.snapshot_spell_power;
        }
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let outcome = Outcome::Table(PhysicalOutcome::MeleeSpecialHit { count: true });
        let result = fight.calc_tick_damage(spell, side, base, attacker, outcome);
        fight.deal_damage(spell, result, true);
    }
}

impl HawkGuardian {
    /// The roll's bounds, a swing's average a spread either side.
    pub(crate) fn swing_bounds(&self) -> (f64, f64) {
        (
            self.swing_average * (1.0 - self.swing_spread),
            self.swing_average * (1.0 + self.swing_spread),
        )
    }

    /// What a hawk's crit chance differs from its owner's by, in percent: `crit_share` of the
    /// owner's crit and offset, less the owner's crit the spell reads.
    pub(crate) fn crit_adjustment(&self, owner_crit_percent: f64) -> f64 {
        self.crit_share * (owner_crit_percent + self.crit_offset) - owner_crit_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guardian() -> HawkGuardian {
        HawkGuardian {
            swing_average: 12.0,
            swing_spread: 1.0 / 3.0,
            attack_power_coefficient: 0.015,
            crit_share: 0.5,
            crit_offset: 2.0,
        }
    }

    #[test]
    fn a_swing_rolls_a_third_either_side_of_its_average() {
        let (low, high) = guardian().swing_bounds();
        assert!((low - 8.0).abs() < 1e-12 && (high - 16.0).abs() < 1e-12);
    }

    /// The hawk reads half of what its owner's crit and offset make, less the owner's crit the
    /// spell reads: 0.5 * (20 + 2) - 20 = -9, so the spell's 20 plus -9 is 11.
    #[test]
    fn a_hawks_crit_is_half_its_owners() {
        let adjustment = guardian().crit_adjustment(20.0);
        assert!((adjustment - -9.0).abs() < 1e-12);
        assert!((20.0 + adjustment - 0.5 * (20.0 + 2.0)).abs() < 1e-12);
        // Without an owner crit it keeps half the offset.
        assert!((guardian().crit_adjustment(0.0) - 1.0).abs() < 1e-12);
    }
}
