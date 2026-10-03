//! Ice Lance (1240047), from Go sim/mage/ice_lance.go: an instant binary Frost spell.
//! Against a target the mage counts as frozen, the rolled hit is multiplied after the
//! outcome, so threat keeps the unmultiplied damage, as in Go.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    frozen_multiplier: Option<f64>,
) {
    let base = fight.roll_damage_effect(spell);
    let mut result = fight.calc_damage(spell, target, base);
    if let Some(multiplier) = frozen_multiplier {
        result.damage *= multiplier;
    }
    fight.deal_damage_after_travel(spell, result);
}
