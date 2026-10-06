//! The warrior's own Battle Shout (25289), from Go sim/warrior/battle_shout.go. The shouts are
//! a single aura category: with nothing in it the cast puts the buff up, the warrior's own copy
//! is only worth refreshing as it runs out, and the party's has to be matched first. The cast
//! always hits for no damage, then the warrior's aura activates.

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult};

/// Go `OutcomeHit`.
const OUTCOME_HIT: u16 = 1 << 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BattleShout {
    pub(crate) aura: AuraRef,
    /// The exclusive category the shouts share.
    pub(crate) category: usize,
    pub(crate) value: f64,
    pub(crate) refresh_threshold: i64,
}

/// The cast's `ExtraCastCondition`.
pub(crate) fn condition<A: Agent>(fight: &Fight<A>, params: BattleShout) -> bool {
    match fight.exclusive_active(params.category) {
        None => true,
        Some((aura, _)) if aura == params.aura => {
            fight.aura(aura).remaining(fight.now) <= params.refresh_threshold
        }
        Some((_, priority)) => params.value >= priority,
    }
}

/// The cast's `ApplyEffects`: Go `CalcAndDealOutcome` with `OutcomeAlwaysHit`, then the aura.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: BattleShout,
) {
    let threat = fight.spells[spell].flat_threat_bonus * fight.player.threat_multiplier;
    fight.spells[spell].metrics[target.index()].hits += 1;
    let result = SpellResult {
        armor_multiplier: 0.0,
        target,
        attacker: fight.spells[spell].caster,
        outcome: OUTCOME_HIT,
        damage: 0.0,
        threat,
    };
    fight.deal_damage(spell, result, false);
    fight.activate_aura(params.aura);
}
