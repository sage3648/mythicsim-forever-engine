//! Explosive Trap (13813, 14316, 14317), from Go sim/hunter/traps.go
//! `registerExplosiveTrapSpell`: one magic hit for each active target, from the cast target
//! on, each on its own roll of the Go literal range times the AoE cap, then the area dot on
//! the hunter. Every tick deals the snapshot to each target that Immolation Trap is not
//! burning (Go `HasActiveAuraWithTag("ImmolationTrap")`).

use crate::core::fight::{Agent, DotId, Fight, Outcome, Side, SpellId};

/// Whether a trap's tick rolls the magic crit, which its fire burn does where the effect row
/// states Periodic Can Crit; a trap that is not a magic spell is unsupported.
pub(crate) fn trap_tick_crit(outcome: Outcome) -> Result<bool, String> {
    match outcome {
        Outcome::TickMagicCrit => Ok(true),
        Outcome::Tick => Ok(false),
        _ => Err("trap ticks that are not magic are unsupported".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExplosiveTrap {
    hit_min: f64,
    hit_max: f64,
    hits: i32,
    aoe_cap_multiplier: f64,
    /// Immolation Trap's dot, whose aura on a target keeps the burn off it.
    immolation: Option<DotId>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    hit_min: f64,
    hit_max: f64,
    hits: i32,
    aoe_cap_multiplier: f64,
    tick_base: f64,
    tick_outcome: Outcome,
    immolation: Option<SpellId>,
) -> Result<ExplosiveTrap, String> {
    let dot = fight.spells[spell].dot.ok_or("Explosive Trap has no dot")?;
    fight.dots[dot].tick_base = Some(tick_base);
    fight.dots[dot].tick_can_crit = trap_tick_crit(tick_outcome)?;
    Ok(ExplosiveTrap {
        hit_min,
        hit_max,
        hits,
        aoe_cap_multiplier,
        immolation: immolation.and_then(|spell| fight.spells[spell].dot),
    })
}

impl ExplosiveTrap {
    /// The cast's `ApplyEffects`: the hits, each calculated and dealt in turn, then
    /// `AOEDot().Apply`.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let mut current = target;
        for _ in 0..self.hits {
            let base = fight.go_roll(self.hit_min, self.hit_max) * self.aoe_cap_multiplier;
            let result = fight.calc_damage(spell, current, base);
            fight.deal_damage(spell, result, false);
            current = fight.next_target(current);
        }
        let dot = fight.spells[spell].dot.expect("Explosive Trap has a dot");
        fight.apply_dot(dot);
    }

    /// The area dot's `OnTick`: `CalcAndDealPeriodicSnapshotDamage` with the effect row's tick
    /// outcome on each target Immolation Trap is not burning, in unit index order.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let spell = fight.dots[dot].spell;
        for position in 0..fight.targets.len() {
            let target = Side::target(position);
            if let Some(immolation) = self.immolation {
                let burning = fight.dots[fight.dot_on(immolation, target)].aura;
                if fight.aura(burning).active {
                    continue;
                }
            }
            let result = fight.snapshot_dot_tick_calc_on(dot, target);
            fight.deal_damage(spell, result, true);
        }
    }
}
