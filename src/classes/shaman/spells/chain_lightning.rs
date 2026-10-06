//! Chain Lightning, every rank, from Go sim/shaman/chain_lightning.go. The bolt hits up to
//! three targets, each from the next after the one before, rolling the rank's damage row for
//! each; the spell's damage multiplier takes the bounce reduction after every hit so the next
//! is weaker. Every hit is resolved before any is dealt, so a proc on the first cannot reach
//! the second. Dealing then runs in turn: Lightning Overload rolls a third of its chance for a
//! landed hit and casts that hit's own overload on its target, the hit is dealt, and the
//! reduction is undone. The multiply and divide stay in Go's order because the spell's
//! multiplier carries between casts and iterations.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Go `maxHits`: the most targets a Chain Lightning hits.
const MAX_HITS: usize = 3;

/// Chain Lightning's bounce and Lightning Overload's roll.
#[derive(Clone, Debug)]
pub(crate) struct ChainLightning {
    pub(crate) overload_chance: f64,
    pub(crate) label: String,
    pub(crate) bounce_reduction: f64,
    pub(crate) bounce_bonus: f64,
}

/// Go `ApplyEffects`. `overloads` holds the overload spell of each hit, in hit order; an
/// overload passes none.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    state: &ChainLightning,
    overloads: Option<&[SpellId]>,
) {
    let bounce = state.bounce_reduction + state.bounce_bonus;
    let mut results = Vec::with_capacity(MAX_HITS);
    let mut current = target;
    for _ in 0..MAX_HITS.min(fight.targets.len()) {
        let base = fight.roll_damage_effect(spell);
        results.push(fight.calc_damage(spell, current, base));
        current = fight.next_target(current);
        fight.spells[spell].damage_multiplier *= bounce;
    }

    for (hit, result) in results.into_iter().enumerate() {
        if let Some(overloads) = overloads {
            if result.landed() && fight.proc(state.overload_chance / 3.0, &state.label) {
                fight.cast(overloads[hit], result.target);
            }
        }
        fight.deal_damage(spell, result, false);
        fight.spells[spell].damage_multiplier /= bounce;
    }
}
