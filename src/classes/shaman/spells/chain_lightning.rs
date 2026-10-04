//! Chain Lightning, every rank, from Go sim/shaman/chain_lightning.go. Against the one target
//! in scope it hits once: the hit resolves, the spell's damage multiplier takes the bounce
//! reduction for a next target, Lightning Overload rolls a third of its chance, and the hit
//! is dealt at once before the reduction is undone. The multiply and divide stay in Go's
//! order because the spell's multiplier carries between casts and iterations.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Chain Lightning's bounce and Lightning Overload's roll.
#[derive(Clone, Debug)]
pub(crate) struct ChainLightning {
    pub(crate) overload_chance: f64,
    pub(crate) label: String,
    pub(crate) bounce_reduction: f64,
    pub(crate) bounce_bonus: f64,
}

/// Go `ApplyEffects` with one target. An overload passes no `overload`.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    state: &ChainLightning,
    overload: Option<SpellId>,
) {
    let bounce = state.bounce_reduction + state.bounce_bonus;
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.spells[spell].damage_multiplier *= bounce;

    if let Some(overload) = overload {
        if result.landed() && fight.proc(state.overload_chance / 3.0, &state.label) {
            fight.cast(overload, result.target);
        }
    }
    fight.deal_damage(spell, result, false);
    fight.spells[spell].damage_multiplier /= bounce;
}
