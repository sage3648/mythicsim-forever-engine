//! Execute (20662), from Go sim/warrior/execute.go: after its cost, it spends the rage left,
//! up to what the bar could hold beyond the cost, for damage per point. The extra spend is
//! credited to the cost's metrics without counting an event.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Execute {
    pub(crate) base_damage: f64,
    pub(crate) damage_per_rage: f64,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, params: Execute) {
    let max = fight.maximum_rage() - fight.current_cost(spell);
    let extra = fight.current_rage().min(max);
    let metrics = fight.spells[spell]
        .mana_metrics
        .expect("Execute has rage metrics");
    fight.spend_rage(extra, metrics);
    fight.resources[metrics].events -= 1;
    // Go's arm64 build fuses this multiply and add.
    let base = params.damage_per_rage.mul_add(extra, params.base_damage);
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
