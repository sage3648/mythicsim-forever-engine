//! Shadowform (15473), from Go sim/priest/talents_shadow.go `applyShadowform`: its aura raises
//! Shadow damage, halves Shadow costs and doubles the crit damage bonus of the spells the
//! client's mask names. A helpful Holy cast ends it. Its Physical damage taken cut has no
//! effect in scope, where the player takes no damage.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct Shadowform {
    pub(crate) aura: AuraRef,
    /// Damage, cost and crit damage, in Go's attachment order.
    mods: [ModId; 3],
    cancels: Vec<bool>,
}

fn positions(spells: &[usize], len: usize) -> Vec<SpellId> {
    spells
        .iter()
        .copied()
        .filter(|&spell| spell < len)
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    damage_percent: f64,
    cost_percent: f64,
    crit_multiplier: f64,
    school_spells: &[usize],
    crit_spells: &[usize],
    cancel_spells: &[usize],
) -> Result<Shadowform, String> {
    let aura = fight.player_aura(aura)?;
    let len = fight.spells.len();
    let damage = fight.register_mod(
        ModKind::DamageDonePercent,
        damage_percent,
        0,
        positions(school_spells, len),
    );
    let cost = fight.register_mod(
        ModKind::PowerCostPercent,
        cost_percent,
        0,
        positions(school_spells, len),
    );
    let crit = fight.register_mod(
        ModKind::CritMultiplierFlat,
        crit_multiplier,
        0,
        positions(crit_spells, len),
    );
    let mut cancels = vec![false; len];
    for spell in positions(cancel_spells, len) {
        cancels[spell] = true;
    }
    Ok(Shadowform {
        aura,
        mods: [damage, cost, crit],
        cancels,
    })
}

impl Shadowform {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        for modifier in self.mods {
            fight.activate_mod(modifier);
        }
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        for modifier in self.mods {
            fight.deactivate_mod(modifier);
        }
    }

    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.cancels[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
