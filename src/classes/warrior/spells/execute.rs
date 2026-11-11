//! Execute (20662), from Go sim/warrior/execute.go: after its cost, it spends the rage left,
//! up to what the bar could hold beyond the cost, for damage per point. The extra spend is
//! credited to the cost's metrics without counting an event.
//!
//! The Go reference spends the extra rage before the roll and refunds its share of the cost
//! when the Execute is avoided. Under Forever's rules an Execute that misses, is dodged or
//! parried costs its base rage alone: the extra rage stays with the warrior and none of the
//! cost comes back. In raw combat logs of client 1.60.1.70862, 36 avoided Executes from 14
//! warriors all keep the pool, for example 46.9 rage to 37.9 after a parry and one Bloodrage
//! tick at a cost of 10.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Execute {
    pub(crate) base_damage: f64,
    pub(crate) damage_per_rage: f64,
    pub(crate) extra_rage_on_landed: bool,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, params: Execute) {
    let max = fight.maximum_rage() - fight.current_cost(spell);
    let extra = fight.current_rage().min(max);
    let metrics = fight.spells[spell]
        .mana_metrics
        .expect("Execute has rage metrics");
    if !params.extra_rage_on_landed {
        fight.spend_rage(extra, metrics);
        fight.resources[metrics].events -= 1;
    }
    // Go's arm64 build fuses this multiply and add.
    let base = params.damage_per_rage.mul_add(extra, params.base_damage);
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    if params.extra_rage_on_landed {
        if result.landed() {
            fight.spend_rage(extra, metrics);
            fight.resources[metrics].events -= 1;
        }
        fight.deal_damage(spell, result, false);
        return;
    }
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
