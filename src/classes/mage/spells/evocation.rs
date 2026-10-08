//! Evocation (12051), from Go sim/mage/evocation.go: an eight-second self channel. While
//! it runs, the regeneration aura raises the spirit regeneration multiplier by the row's
//! percentage and lets spirit regeneration run in full.

use crate::core::fight::{Agent, Fight, SpellId};

/// The channel cast: apply the self-only dot.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    let dot = fight.spells[spell].dot.expect("Evocation has a channel");
    fight.apply_dot(dot);
}

/// Evocation Regen OnGain: Go `AddSpiritRegenMultiplier`, `SetForceFullSpiritRegen` and
/// `UpdateManaRegenRates`. Both helpers move the baseline an Innervate's regeneration is
/// credited against as well, when one is up.
pub(crate) fn regen_gain<A: Agent>(fight: &mut Fight<A>, multiplier: f64) {
    fight.add_spirit_regen_multiplier(multiplier);
    fight.set_force_full_spirit_regen(true);
    fight.update_mana_regen_rates();
}

pub(crate) fn regen_expire<A: Agent>(fight: &mut Fight<A>, multiplier: f64) {
    fight.add_spirit_regen_multiplier(-multiplier);
    fight.set_force_full_spirit_regen(false);
    fight.update_mana_regen_rates();
}
