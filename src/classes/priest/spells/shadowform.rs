//! Shadowform (15473), from Go sim/priest/talents_shadow.go `applyShadowform`: its aura raises
//! Shadow damage, halves Shadow costs and doubles the crit damage bonus of the spells the
//! client's mask names. A helpful Holy cast no longer ends it (community #719); its form refuses
//! Holy Nova and Chastise through their cast requirements (#686). Its Physical damage taken cut
//! has no effect in scope, where the player takes no damage.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct Shadowform {
    pub(crate) aura: AuraRef,
    /// Damage, cost and crit damage, in Go's attachment order.
    mods: [ModId; 3],
}

fn positions(spells: &[usize], len: usize) -> Vec<SpellId> {
    spells
        .iter()
        .copied()
        .filter(|&spell| spell < len)
        .collect()
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    damage_percent: f64,
    cost_percent: f64,
    crit_multiplier: f64,
    school_spells: &[usize],
    crit_spells: &[usize],
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
    Ok(Shadowform {
        aura,
        mods: [damage, cost, crit],
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
}
