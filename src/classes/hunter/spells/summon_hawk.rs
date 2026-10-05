//! Summon Hawk (1293241 to 1293527), from Go sim/hunter/summon_hawk.go: a dive bomb on the
//! rank's base plus a share of ranged attack power, a Go literal, on the melee special table,
//! which cannot miss when the client marks the rank always-hit. A landed dive bomb leaves a
//! hawk: a physical dot whose ticks roll the physical crit, in a free slot or else the one with
//! the least time left.

use crate::core::fight::{melee::PhysicalOutcome, Agent, DotId, Fight, Outcome, Side, SpellId};

/// The bound Summon Hawk.
#[derive(Clone, Debug)]
pub(crate) struct SummonHawk {
    base_damage: f64,
    attack_power_share: f64,
    always_hits: bool,
    /// Each hawk slot's dot, in Go's slot order.
    hawks: Vec<DotId>,
}

impl SummonHawk {
    pub(crate) fn bind<A: Agent>(
        fight: &mut Fight<A>,
        base_damage: f64,
        attack_power_share: f64,
        always_hits: bool,
        hawk_spells: &[usize],
    ) -> Result<Self, String> {
        let mut hawks = Vec::new();
        for &spell in hawk_spells {
            let dot = fight
                .spells
                .get(spell)
                .and_then(|state| state.dot)
                .ok_or("a hawk spell has no dot")?;
            // Go OnSnapshot: the rank's base, without a spell power share.
            fight.dots[dot].tick_base = Some(base_damage);
            hawks.push(dot);
        }
        Ok(SummonHawk {
            base_damage,
            attack_power_share,
            always_hits,
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
    }

    /// A hawk's tick: Go `CalcAndDealPeriodicSnapshotDamage` with `OutcomeTickPhysicalCrit`.
    pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side) = (state.spell, state.side);
        let mut base = state.snapshot_base;
        if state.reads_spell_power {
            base += state.bonus_coefficient * fight.bonus_damage(spell, side)
                - state.snapshot_spell_power;
        }
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let result = fight.calc_tick_damage(spell, side, base, attacker, Outcome::TickPhysicalCrit);
        fight.deal_damage(spell, result, true);
    }
}
