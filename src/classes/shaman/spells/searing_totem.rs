//! Searing Totem, from Go sim/shaman/fire_totems.go `registerSearingTotemSpell`: the cast
//! takes down the other fire totems and applies a dot on the target whose every tick casts
//! the totem's attack, a fixed base that resolves at once and is dealt after travel.

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct SearingTotem {
    pub(crate) attack: SpellId,
    pub(crate) attack_damage: f64,
    /// Go `cancelFireTotems` order: Magma Totem's dot, then this totem's, then Flametongue
    /// Totem's aura.
    pub(crate) magma_totem: Option<AuraRef>,
    pub(crate) flametongue_totem: Option<AuraRef>,
}

/// The totem's ApplyEffects.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, state: &SearingTotem) {
    let dot = fight.spells[spell].dot.expect("the totem has a dot");
    if let Some(aura) = state.magma_totem {
        fight.deactivate_aura(aura);
    }
    let own = fight.dots[dot].aura;
    fight.deactivate_aura(own);
    if let Some(aura) = state.flametongue_totem {
        fight.deactivate_aura(aura);
    }
    fight.apply_dot(dot);
}

/// A dot tick: the totem casts its attack at the dot's unit.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, side: Side, state: &SearingTotem) {
    fight.cast(state.attack, side);
}

/// The attack's ApplyEffects.
pub(crate) fn attack<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, base: f64) {
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
}
