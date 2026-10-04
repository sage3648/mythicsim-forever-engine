//! Lightning Bolt, every rank, from Go sim/shaman/lightning_bolt.go: the damage roll and the
//! hit table resolve at cast completion, and when the bolt lands Lightning Overload may cast
//! the rank's overload, a free copy at half damage, before the bolt deals its damage.

use crate::core::fight::{Agent, Fight, Side, SpellId, SpellResult};

/// Lightning Overload's roll for Lightning Bolt.
#[derive(Clone, Debug)]
pub(crate) struct Overload {
    pub(crate) chance: f64,
    pub(crate) label: String,
}

/// Go `ApplyEffects`, for the bolt and its overload alike.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.class_after_travel(spell, result);
}

/// The `WaitTravelTime` callback. An overload passes no `overload` and only deals its damage.
pub(crate) fn arrive<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    result: SpellResult,
    overload: Option<(SpellId, &Overload)>,
) {
    if let Some((overload_spell, roll)) = overload {
        if result.landed() && fight.proc(roll.chance, &roll.label) {
            fight.cast(overload_spell, result.target);
        }
    }
    fight.deal_damage(spell, result, false);
}
