//! Arcane Explosion (10202), Cone of Cold (10161), Frost Nova (10230) and Blast Wave
//! (13021), from Go sim/mage/arcane_explosion.go, cone_of_cold.go, frost_nova.go and
//! blast_wave.go: `CalcAndDealAoeDamageWithVariance` with `OutcomeMagicHitAndCrit`, a
//! rolled hit calculated and dealt on each target in turn. Cone of Cold, Frost Nova and
//! Blast Wave are binary, which the spell's flags carry.

use crate::core::fight::{Agent, Fight, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    fight.calc_and_deal_aoe_damage_with_variance(
        spell,
        |fight| fight.roll_damage_effect(spell),
        Fight::calc_damage,
    );
}
