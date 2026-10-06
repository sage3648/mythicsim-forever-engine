//! Rend (11574), from Go sim/warrior/rend.go: a special hit roll without damage; a landed hit
//! applies the bleed, whose ticks deal the client base plus a share of attack power read at
//! each tick, a Go literal. A miss refunds rage.

use crate::core::fight::{melee::PhysicalOutcome, Agent, DotId, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rend {
    pub(crate) dot: DotId,
    pub(crate) tick_base: f64,
    pub(crate) attack_power_per_tick: f64,
    /// spelldata `TickOutcome`: the client row lets its ticks roll the physical crit.
    pub(crate) tick_can_crit: bool,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, params: Rend) {
    let outcome = PhysicalOutcome::MeleeSpecialHit { count: true };
    let result = fight.calc_physical_outcome(spell, target, outcome);
    fight.deal_damage(spell, result, false);
    if result.landed() {
        let dot = fight.dot_on(params.dot, target);
        fight.apply_dot(dot);
    } else {
        fight.issue_refund(spell);
    }
}

/// A tick on the physical periodic path, rolling the physical crit when the row says it can.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, params: Rend) {
    let (spell, side) = (fight.dots[dot].spell, fight.dots[dot].side);
    // Go's arm64 build fuses this multiply and add.
    let base = params
        .attack_power_per_tick
        .mul_add(fight.melee_attack_power(), params.tick_base);
    let result = fight.calc_physical_periodic_damage(spell, side, base, params.tick_can_crit);
    fight.deal_damage(spell, result, true);
}
